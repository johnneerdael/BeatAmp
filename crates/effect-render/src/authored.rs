use crate::{
    gpu::{MeshVertex, Params},
    resources::{Resources, key},
};
use anyhow::{Result, ensure};
use effect_model::shader::Shader;
use wgpu::util::DeviceExt;

pub struct Authored {
    pub pipeline: wgpu::RenderPipeline,
    pub groups: [wgpu::BindGroup; 2],
    uniform: wgpu::Buffer,
    sizes: Vec<[f32; 4]>,
}
impl Authored {
    #[allow(clippy::too_many_arguments)] // Explicit pass resource contract, used only during prewarming.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        shader: Shader,
        resources: &mut Resources,
        history: [&wgpu::TextureView; 2],
        blurs: [&wgpu::TextureView; 3],
        canvas: [u32; 2],
        main_wrap: bool,
    ) -> Result<Self> {
        ensure!(
            shader.loop_budget.is_some(),
            "authored GPU shader must declare bounded loops"
        );
        for binding in &shader.bindings {
            if binding.kind != "uniform" {
                resources.ensure(&binding.resource_name(), device, queue)?;
            }
        }
        let mut sizes = vec![];
        for name in &shader.texture_sizes {
            let alias = format!("texsize_{name}");
            resources.ensure(&alias, device, queue)?;
            let size = if name == "main" {
                [canvas[0], canvas[1], 1]
            } else {
                resources.get(&alias)?.size
            };
            sizes.push([
                size[0] as f32,
                size[1] as f32,
                1.0 / size[0] as f32,
                1.0 / size[1] as f32,
            ]);
        }
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("source-shader-params"),
            contents: bytemuck::bytes_of(&Params::default()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let entries: Vec<_> = shader
            .bindings
            .iter()
            .map(|b| wgpu::BindGroupLayoutEntry {
                binding: b.binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: match b.kind.as_str() {
                    "uniform" => wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    "sampler" => wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    _ => wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: if b.kind == "texture3d" {
                            wgpu::TextureViewDimension::D3
                        } else {
                            wgpu::TextureViewDimension::D2
                        },
                        multisampled: false,
                    },
                },
                count: None,
            })
            .collect();
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("source-reflected-resources"),
            entries: &entries,
        });
        let make_group = |index: usize| -> Result<wgpu::BindGroup> {
            let mut entries = vec![];
            for b in &shader.bindings {
                let name = b.resource_name();
                let (base, _, _) = key(&name, main_wrap);
                let resource = match b.kind.as_str() {
                    "uniform" => uniform.as_entire_binding(),
                    "sampler" => {
                        wgpu::BindingResource::Sampler(resources.sampler(&name, main_wrap))
                    }
                    _ => wgpu::BindingResource::TextureView(match base.as_str() {
                        "main" => history[index],
                        "blur1" => blurs[0],
                        "blur2" => blurs[1],
                        "blur3" => blurs[2],
                        _ => &resources.get(&name)?.view,
                    }),
                };
                entries.push(wgpu::BindGroupEntry {
                    binding: b.binding,
                    resource,
                });
            }
            Ok(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("source-pass-bindings"),
                layout: &layout,
                entries: &entries,
            }))
        };
        let groups = [make_group(0)?, make_group(1)?];
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("native-mesh-vertex"),
            source: wgpu::ShaderSource::Wgsl(include_str!("default.wgsl").into()),
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("authored-source-fragment"),
            source: wgpu::ShaderSource::Wgsl(shader.wgsl.clone().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("authored-pipeline-layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let attributes = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2,2=>Float32x2];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("authored-fragment"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &vertex,
                entry_point: Some("mesh_vertex"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(&shader.entry),
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
        if let Some(error) = pollster::block_on(scope.pop()) {
            return Err(anyhow::anyhow!(
                "authored pipeline validation failed: {error}"
            ));
        }
        Ok(Self {
            pipeline,
            groups,
            uniform,
            sizes,
        })
    }
    pub fn update(&self, queue: &wgpu::Queue, params: &Params) {
        let mut params = *params;
        for (i, size) in self.sizes.iter().enumerate() {
            params.data[32 + i] = *size;
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&params));
    }
}
