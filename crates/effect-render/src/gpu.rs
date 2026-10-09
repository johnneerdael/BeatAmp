use crate::geometry::Vertex;
use anyhow::{Context, Result, ensure};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 2],
    pub original: [f32; 2],
    pub warped: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Params {
    pub data: [[f32; 4]; 64],
}
impl Default for Params {
    fn default() -> Self {
        Self {
            data: [[0.0; 4]; 64],
        }
    }
}

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
}
impl Gpu {
    pub async fn headless() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("preset-native-4k"),
                ..Default::default()
            })
            .await?;
        Ok(Self {
            device,
            queue,
            adapter,
        })
    }
}
struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}
fn target(device: &wgpu::Device, width: u32, height: u32, label: &str) -> Target {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    Target { texture, view }
}

/// Native primary ping-pong histories and final target. Readback is opt-in.
pub struct Renderer {
    pub gpu: Gpu,
    pub width: u32,
    pub height: u32,
    histories: [Target; 2],
    pub output: wgpu::Texture,
    output_view: wgpu::TextureView,
    uniform: wgpu::Buffer,
    mesh: wgpu::Buffer,
    indices: wgpu::Buffer,
    geometry: wgpu::Buffer,
    groups: [[wgpu::BindGroup; 2]; 2],
    main_wrap: bool,
    warp: wgpu::RenderPipeline,
    comp: wgpu::RenderPipeline,
    alpha: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
    current: usize,
    composite_mesh: wgpu::Buffer,
    blurs: [Target; 3],
    blur_pipeline: wgpu::RenderPipeline,
    blur_groups: [wgpu::BindGroup; 4],
    authored_warp: Option<crate::authored::Authored>,
    authored_comp: Option<crate::authored::Authored>,
    resources: Option<crate::resources::Resources>,
}
impl Renderer {
    pub fn resource_substitutions(&self) -> &[String] {
        self.resources
            .as_ref()
            .map(|r| r.substitutions.as_slice())
            .unwrap_or(&[])
    }
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.output_view
    }
    pub fn set_shaders(
        &mut self,
        warp: Option<effect_model::shader::Shader>,
        comp: Option<effect_model::shader::Shader>,
        assets: std::collections::BTreeMap<String, Vec<u8>>,
        main_wrap: bool,
    ) -> Result<()> {
        let mut resources = crate::resources::Resources::new(&self.gpu.device, assets);
        let build = |shader: effect_model::shader::Shader,
                     resources: &mut crate::resources::Resources| {
            crate::authored::Authored::new(
                &self.gpu.device,
                &self.gpu.queue,
                shader,
                resources,
                [&self.histories[0].view, &self.histories[1].view],
                [
                    &self.blurs[0].view,
                    &self.blurs[1].view,
                    &self.blurs[2].view,
                ],
                [self.width, self.height],
                main_wrap,
            )
        };
        let authored_warp = warp.map(|s| build(s, &mut resources)).transpose()?;
        let authored_comp = comp.map(|s| build(s, &mut resources)).transpose()?;
        self.authored_warp = authored_warp;
        self.authored_comp = authored_comp;
        self.resources = Some(resources);
        self.main_wrap = main_wrap;
        self.clear_history();
        Ok(())
    }
    pub fn clear_history(&self) {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        for view in self
            .histories
            .iter()
            .map(|t| &t.view)
            .chain(self.blurs.iter().map(|t| &t.view))
        {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("preset-state-reset"),
                color_attachments: &colors,
                ..Default::default()
            });
        }
        self.gpu.queue.submit([encoder.finish()]);
    }
    pub fn allocation_dimensions(&self) -> [[u32; 2]; 3] {
        [
            [
                self.histories[0].texture.width(),
                self.histories[0].texture.height(),
            ],
            [
                self.histories[1].texture.width(),
                self.histories[1].texture.height(),
            ],
            [self.output.width(), self.output.height()],
        ]
    }
    pub fn new(gpu: Gpu, width: u32, height: u32) -> Result<Self> {
        ensure!(
            width > 0
                && height > 0
                && width <= gpu.device.limits().max_texture_dimension_2d
                && height <= gpu.device.limits().max_texture_dimension_2d,
            "native canvas exceeds adapter capability"
        );
        let device = &gpu.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("milkdrop-fixed-function"),
            source: wgpu::ShaderSource::Wgsl(include_str!("default.wgsl").into()),
        });
        let histories = [
            target(device, width, height, "native-history-a"),
            target(device, width, height, "native-history-b"),
        ];
        let output = target(device, width, height, "native-final");
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frame-params"),
            contents: bytemuck::bytes_of(&Params::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let samplers = [wgpu::AddressMode::ClampToEdge, wgpu::AddressMode::Repeat].map(|mode| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("feedback-linear-declared-addressing"),
                address_mode_u: mode,
                address_mode_v: mode,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            })
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("feedback-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let groups = std::array::from_fn(|mode| {
            std::array::from_fn(|i| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("feedback-group"),
                    layout: &bind_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&histories[i].view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&samplers[mode]),
                        },
                    ],
                })
            })
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("feedback-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_layout)],
            immediate_size: 0,
        });
        let mesh_attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2];
        let mesh_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<MeshVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh_attributes,
        };
        let warp = pipeline(
            device,
            &shader,
            Some(&layout),
            "mesh_vertex",
            "default_warp",
            mesh_layout.clone(),
            None,
        );
        let comp = pipeline(
            device,
            &shader,
            Some(&layout),
            "mesh_vertex",
            "default_comp",
            mesh_layout,
            None,
        );
        let geometry_attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x4,2=>Float32x3];
        let geometry_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &geometry_attributes,
        };
        let alpha = pipeline(
            device,
            &shader,
            Some(&layout),
            "geometry_vertex",
            "geometry_fragment",
            geometry_layout.clone(),
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );
        let additive = pipeline(
            device,
            &shader,
            Some(&layout),
            "geometry_vertex",
            "geometry_fragment",
            geometry_layout,
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::OVER,
            }),
        );
        let mesh = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("warp-mesh"),
            size: 49 * 33 * std::mem::size_of::<MeshVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut triangle_indices = vec![];
        for y in 0..32u32 {
            for x in 0..48u32 {
                let at = y * 49 + x;
                triangle_indices.extend_from_slice(&[
                    at,
                    at + 1,
                    at + 49,
                    at + 1,
                    at + 50,
                    at + 49,
                ]);
            }
        }
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-topology"),
            contents: bytemuck::cast_slice(&triangle_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let geometry = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("native-stroke-geometry"),
            size: 16 * 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let static_vertices: Vec<MeshVertex> = (0..=32)
            .flat_map(|y| {
                (0..=48).map(move |x| {
                    let uv = [x as f32 / 48.0, y as f32 / 32.0];
                    MeshVertex {
                        position: uv,
                        original: uv,
                        warped: uv,
                    }
                })
            })
            .collect();
        let composite_mesh = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("native-composite-mesh"),
            contents: bytemuck::cast_slice(&static_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let blurs = std::array::from_fn(|i| {
            target(
                device,
                (width >> (i + 1)).max(1),
                (height >> (i + 1)).max(1),
                "declared-auxiliary-blur",
            )
        });
        let blur_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("auxiliary-blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });
        let blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur-chain"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &blur_module,
                entry_point: Some("vertex"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blur_module,
                entry_point: Some("fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let blur_layout = blur_pipeline.get_bind_group_layout(0);
        let blur_groups = std::array::from_fn(|i| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("blur-input"),
                layout: &blur_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(match i {
                            0 => &histories[0].view,
                            1 => &histories[1].view,
                            2 => &blurs[0].view,
                            _ => &blurs[1].view,
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&samplers[1]),
                    },
                ],
            })
        });
        Ok(Self {
            gpu,
            width,
            height,
            histories,
            output: output.texture,
            output_view: output.view,
            uniform,
            mesh,
            indices,
            geometry,
            groups,
            main_wrap: true,
            warp,
            comp,
            alpha,
            additive,
            current: 0,
            composite_mesh,
            blurs,
            blur_pipeline,
            blur_groups,
            authored_warp: None,
            authored_comp: None,
            resources: None,
        })
    }
    pub fn render(
        &mut self,
        params: &Params,
        mesh: &[MeshVertex],
        vertices: &[Vertex],
        batches: &[(std::ops::Range<u32>, bool)],
    ) -> Result<()> {
        ensure!(mesh.len() == 49 * 33, "expected 48x32-cell warp mesh");
        ensure!(
            std::mem::size_of_val(vertices) <= 16 * 1024 * 1024,
            "geometry allocation budget exceeded"
        );
        let device = &self.gpu.device;
        let queue = &self.gpu.queue;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(params));
        queue.write_buffer(&self.mesh, 0, bytemuck::cast_slice(mesh));
        if !vertices.is_empty() {
            queue.write_buffer(&self.geometry, 0, bytemuck::cast_slice(vertices));
        }
        if let Some(pass) = &self.authored_warp {
            pass.update(queue, params);
        }
        if let Some(pass) = &self.authored_comp {
            pass.update(queue, params);
        }
        let next = 1 - self.current;
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view: &self.histories[next].view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native-feedback-warp"),
                color_attachments: &colors,
                ..Default::default()
            });
            if let Some(shader) = &self.authored_warp {
                pass.set_pipeline(&shader.pipeline);
                pass.set_bind_group(0, &shader.groups[self.current], &[]);
            } else {
                pass.set_pipeline(&self.warp);
                pass.set_bind_group(
                    0,
                    &self.groups[usize::from(self.main_wrap)][self.current],
                    &[],
                );
            }
            pass.set_vertex_buffer(0, self.mesh.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..48 * 32 * 6, 0, 0..1);
            if !vertices.is_empty() {
                pass.set_bind_group(
                    0,
                    &self.groups[usize::from(self.main_wrap)][self.current],
                    &[],
                );
                pass.set_vertex_buffer(0, self.geometry.slice(..));
                for (range, add) in batches {
                    pass.set_pipeline(if *add { &self.additive } else { &self.alpha });
                    pass.draw(range.clone(), 0..1);
                }
            }
        }
        if self.authored_warp.is_some() || self.authored_comp.is_some() {
            for level in 0..3 {
                let colors = [Some(wgpu::RenderPassColorAttachment {
                    view: &self.blurs[level].view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })];
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("auxiliary-blur-level"),
                    color_attachments: &colors,
                    ..Default::default()
                });
                pass.set_pipeline(&self.blur_pipeline);
                pass.set_bind_group(
                    0,
                    &self.blur_groups[if level == 0 { next } else { level + 1 }],
                    &[],
                );
                pass.draw(0..3, 0..1);
            }
        }
        {
            let colors = [Some(wgpu::RenderPassColorAttachment {
                view: &self.output_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native-final-composite"),
                color_attachments: &colors,
                ..Default::default()
            });
            if let Some(shader) = &self.authored_comp {
                pass.set_pipeline(&shader.pipeline);
                pass.set_bind_group(0, &shader.groups[next], &[]);
            } else {
                pass.set_pipeline(&self.comp);
                pass.set_bind_group(0, &self.groups[usize::from(self.main_wrap)][next], &[]);
            }
            pass.set_vertex_buffer(0, self.composite_mesh.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..48 * 32 * 6, 0, 0..1);
        }
        queue.submit([encoder.finish()]);
        self.current = next;
        Ok(())
    }
    pub fn capture(&self) -> Result<Vec<u8>> {
        let stride = (self.width * 4).div_ceil(256) * 256;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("comparison-only-readback"),
            size: stride as u64 * self.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv().context("readback callback missing")??;
        let mapped = buffer.get_mapped_range(..)?;
        let mut pixels = Vec::with_capacity((self.width * self.height * 4) as usize);
        for row in mapped.chunks(stride as usize) {
            pixels.extend_from_slice(&row[..(self.width * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }
}
fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: Option<&wgpu::PipelineLayout>,
    vertex: &str,
    fragment: &str,
    buffer: wgpu::VertexBufferLayout,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout,
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some(vertex),
            compilation_options: Default::default(),
            buffers: &[Some(buffer)],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
