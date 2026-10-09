//! Prepared native `.beat` graphs. Asset decoding and pipeline creation happen on load.
use crate::gpu::Gpu;
use anyhow::{Context, Result, bail, ensure};
use effect_model::beat::{
    AssetKind, BindingSource, Capabilities, Manifest, Pass, PassKind, Resource,
};
use effect_package::Package;
use image::ImageDecoder;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};
use wgpu::util::DeviceExt;

#[derive(Default)]
pub struct BeatInput {
    pub parameters: BTreeMap<String, Vec<f32>>,
    pub buffers: BTreeMap<String, Vec<u8>>,
    pub audio: Option<BeatAudioDescriptor>,
}
/// Actual provider metadata travels beside named features; no sample format is inferred.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeatAudioDescriptor {
    pub generation: u64,
    pub end_frame: u64,
    pub capture_end_ns: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub timestamp_estimated: bool,
    pub encoding: Option<String>,
}
#[derive(Clone, Debug)]
pub struct AllocationReport {
    pub dimensions: BTreeMap<String, Vec<[u32; 2]>>,
    pub bytes: u64,
}
struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    attachment: wgpu::TextureView,
}
enum Allocated {
    Texture {
        copies: Vec<Texture>,
        initial: Option<Vec<Vec<u8>>>,
    },
    Buffer {
        copies: Vec<wgpu::Buffer>,
        initial: Option<Vec<u8>>,
    },
}
impl Allocated {
    fn index(&self, frame: usize, previous: bool) -> usize {
        let len = match self {
            Self::Texture { copies, .. } => copies.len(),
            Self::Buffer { copies, .. } => copies.len(),
        };
        if len == 1 {
            0
        } else {
            frame ^ usize::from(previous)
        }
    }
    fn texture(&self, frame: usize, previous: bool) -> &Texture {
        let Self::Texture { copies, .. } = self else {
            unreachable!()
        };
        &copies[self.index(frame, previous)]
    }
    fn binding(&self, frame: usize, previous: bool) -> wgpu::BindingResource<'_> {
        match self {
            Self::Texture { .. } => {
                wgpu::BindingResource::TextureView(&self.texture(frame, previous).view)
            }
            Self::Buffer { copies, .. } => copies[self.index(frame, previous)].as_entire_binding(),
        }
    }
}
enum Pipeline {
    Render(wgpu::RenderPipeline),
    Compute(wgpu::ComputePipeline),
}
struct PreparedPass {
    description: Pass,
    pipeline: Pipeline,
    groups: [Vec<wgpu::BindGroup>; 2],
    uniform: Option<wgpu::Buffer>,
    parameter_data: Vec<f32>,
    colors: Vec<String>,
    depth: Option<String>,
}
pub struct BeatRenderer {
    pub gpu: Gpu,
    manifest: Manifest,
    resources: BTreeMap<String, Allocated>,
    passes: Vec<PreparedPass>,
    parameters: BTreeMap<String, Vec<f32>>,
    frame: usize,
    presented: usize,
    report: AllocationReport,
    audio: Option<BeatAudioDescriptor>,
}
impl BeatRenderer {
    pub fn new(gpu: Gpu, package: Package, extent: [u32; 2]) -> Result<Self> {
        let mut caps = Capabilities::baseline(extent);
        caps.features = gpu.device.features();
        caps.limits = gpu.device.limits();
        Self::with_capabilities(gpu, package, caps)
    }
    /// Caller budgets are authoritative; feature/limit availability comes from the device.
    pub fn with_capabilities(gpu: Gpu, package: Package, mut caps: Capabilities) -> Result<Self> {
        package.validate_contents()?;
        caps.features = gpu.device.features();
        caps.limits = gpu.device.limits();
        let plan = package.manifest.validate(&caps)?;
        let mut draw_work = 0u64;
        let mut dispatch_work = 0u64;
        for pass in &package.manifest.passes {
            match pass.kind {
                PassKind::Render => {
                    draw_work = draw_work
                        .checked_add(pass.draw.vertices as u64 * pass.draw.instances as u64)
                        .context("frame draw work overflow")?;
                    ensure!(
                        draw_work <= caps.max_draw_work,
                        "frame draw work budget exceeded"
                    );
                }
                PassKind::Compute => {
                    let dispatch = pass.dispatch.context("compute dispatch must be explicit")?;
                    let work = dispatch
                        .iter()
                        .try_fold(1u64, |a, b| a.checked_mul(*b as u64))
                        .context("frame dispatch work overflow")?;
                    dispatch_work = dispatch_work
                        .checked_add(work)
                        .context("frame dispatch work overflow")?;
                    ensure!(
                        dispatch_work <= caps.max_dispatch_workgroups,
                        "frame dispatch work budget exceeded"
                    );
                }
            }
        }
        let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let result = Self::prepare(gpu, package, plan, &caps);
        let error = pollster::block_on(scope.pop());
        if let Some(error) = error {
            bail!("beat GPU validation: {error}");
        }
        result
    }
    fn prepare(
        gpu: Gpu,
        package: Package,
        plan: effect_model::beat::ExecutionPlan,
        caps: &Capabilities,
    ) -> Result<Self> {
        let manifest = package.manifest;
        let mut programs = BTreeMap::new();
        let mut texture_usages = plan.texture_usages.clone();
        for pass in &manifest.passes {
            if !programs.contains_key(&pass.program) {
                let text = std::str::from_utf8(&package.assets[&pass.program])?;
                let module = naga::front::wgsl::parse_str(text)
                    .map_err(|e| anyhow::anyhow!("{}: {}", pass.id, e.emit_to_string(text)))?;
                let info = naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::all(),
                )
                .validate(&module)
                .with_context(|| format!("invalid WGSL: {}", pass.id))?;
                let shader = gpu
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(&pass.program),
                        source: wgpu::ShaderSource::Wgsl(text.into()),
                    });
                programs.insert(pass.program.clone(), (module, info, shader));
            }
            let (module, info, _) = &programs[&pass.program];
            validate_bindings(module, info, pass, &manifest)?;
            for (_, var) in module.global_variables.iter() {
                let Some(slot) = &var.binding else { continue };
                if matches!(
                    module.types[var.ty].inner,
                    naga::TypeInner::Image {
                        class: naga::ImageClass::Storage { .. },
                        ..
                    }
                ) {
                    let binding = pass
                        .bindings
                        .iter()
                        .find(|b| b.group == slot.group && b.binding == slot.binding)
                        .unwrap();
                    let BindingSource::Resource { resource, .. } = &binding.source else {
                        unreachable!()
                    };
                    *texture_usages.get_mut(resource).unwrap() |=
                        wgpu::TextureUsages::STORAGE_BINDING;
                }
            }
        }
        let mut resources = BTreeMap::new();
        let mut dimensions = BTreeMap::new();
        for (name, description) in &manifest.resources {
            let resource = match description {
                Resource::Texture {
                    format,
                    history,
                    mips,
                    asset,
                    ..
                } => {
                    ensure!(
                        *mips == 1 || asset.is_some(),
                        "dynamic mip generation is not declared/supported: {name}"
                    );
                    let [width, height] = plan.dimensions[name];
                    let initial = asset
                        .as_ref()
                        .map(|id| match manifest.assets[id].kind {
                            AssetKind::Image => {
                                ensure!(
                                    *mips == 1,
                                    "image assets need explicitly cooked binary mip chains: {name}"
                                );
                                Ok(vec![decode_image(
                                    &package.assets[id],
                                    *format,
                                    [width, height],
                                    caps.max_resource_bytes,
                                )?])
                            }
                            AssetKind::Binary => {
                                cooked_mips(&package.assets[id], *format, [width, height], *mips)
                            }
                            _ => bail!("unsupported texture asset kind: {name}"),
                        })
                        .transpose()?;
                    let usage = texture_usages[name]
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC;
                    ensure!(
                        gpu.adapter
                            .get_texture_format_features(*format)
                            .allowed_usages
                            .contains(usage),
                        "unsupported actual texture usage: {name}"
                    );
                    let copies = (0..if *history { 2 } else { 1 })
                        .map(|_| {
                            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                                label: Some(name),
                                size: wgpu::Extent3d {
                                    width,
                                    height,
                                    depth_or_array_layers: 1,
                                },
                                mip_level_count: *mips,
                                sample_count: 1,
                                dimension: wgpu::TextureDimension::D2,
                                format: *format,
                                usage,
                                view_formats: &[],
                            });
                            let view = texture.create_view(&Default::default());
                            let attachment = texture.create_view(&wgpu::TextureViewDescriptor {
                                mip_level_count: Some(1),
                                ..Default::default()
                            });
                            Texture {
                                texture,
                                view,
                                attachment,
                            }
                        })
                        .collect::<Vec<_>>();
                    dimensions.insert(
                        name.clone(),
                        copies
                            .iter()
                            .map(|t| [t.texture.width(), t.texture.height()])
                            .collect(),
                    );
                    Allocated::Texture { copies, initial }
                }
                Resource::Buffer {
                    bytes,
                    history,
                    asset,
                    ..
                } => {
                    ensure!(
                        bytes.is_multiple_of(4),
                        "buffer size must be a multiple of four: {name}"
                    );
                    let initial = asset.as_ref().map(|id| package.assets[id].clone());
                    if let Some(initial) = &initial {
                        ensure!(
                            initial.len() as u64 == *bytes,
                            "buffer asset size mismatch: {name}"
                        );
                    }
                    let copies = (0..if *history { 2 } else { 1 })
                        .map(|_| {
                            gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some(name),
                                size: *bytes,
                                usage: wgpu::BufferUsages::STORAGE
                                    | wgpu::BufferUsages::UNIFORM
                                    | wgpu::BufferUsages::COPY_DST
                                    | wgpu::BufferUsages::COPY_SRC,
                                mapped_at_creation: false,
                            })
                        })
                        .collect();
                    Allocated::Buffer { copies, initial }
                }
            };
            resources.insert(name.clone(), resource);
        }
        let mut passes = Vec::new();
        let mut parameter_allocation = 0u64;
        for index in plan.pass_order {
            let pass = &manifest.passes[index];
            let (module, _, shader) = &programs[&pass.program];
            let mut colors = Vec::new();
            let mut depth = None;
            let mut targets = Vec::new();
            for name in &pass.writes {
                if let Resource::Texture { format, .. } = &manifest.resources[name] {
                    if format.is_depth_stencil_format() {
                        depth = Some(name.clone());
                    } else {
                        colors.push(name.clone());
                        targets.push(Some(wgpu::ColorTargetState {
                            format: *format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        }));
                    }
                }
            }
            let pipeline = match pass.kind {
                PassKind::Render => {
                    let depth_stencil = depth.as_ref().map(|name| {
                        let Resource::Texture { format, .. } = manifest.resources[name] else {
                            unreachable!()
                        };
                        wgpu::DepthStencilState {
                            format,
                            depth_write_enabled: Some(true),
                            depth_compare: Some(wgpu::CompareFunction::LessEqual),
                            stencil: Default::default(),
                            bias: Default::default(),
                        }
                    });
                    let vertex = entry(module, naga::ShaderStage::Vertex)?;
                    let fragment = if targets.is_empty() {
                        None
                    } else {
                        Some(entry(module, naga::ShaderStage::Fragment)?)
                    };
                    Pipeline::Render(gpu.device.create_render_pipeline(
                        &wgpu::RenderPipelineDescriptor {
                            label: Some(&pass.id),
                            layout: None,
                            vertex: wgpu::VertexState {
                                module: shader,
                                entry_point: Some(vertex),
                                compilation_options: Default::default(),
                                buffers: &[],
                            },
                            primitive: wgpu::PrimitiveState {
                                topology: pass.draw.topology,
                                ..Default::default()
                            },
                            depth_stencil,
                            multisample: Default::default(),
                            fragment: fragment.map(|name| wgpu::FragmentState {
                                module: shader,
                                entry_point: Some(name),
                                compilation_options: Default::default(),
                                targets: &targets,
                            }),
                            multiview_mask: None,
                            cache: None,
                        },
                    ))
                }
                PassKind::Compute => {
                    ensure!(
                        pass.dispatch.is_some(),
                        "compute dispatch must be explicit: {}",
                        pass.id
                    );
                    Pipeline::Compute(gpu.device.create_compute_pipeline(
                        &wgpu::ComputePipelineDescriptor {
                            label: Some(&pass.id),
                            layout: None,
                            module: shader,
                            entry_point: Some(entry(module, naga::ShaderStage::Compute)?),
                            compilation_options: Default::default(),
                            cache: None,
                        },
                    ))
                }
            };
            let mut parameter_data = Vec::new();
            for name in &pass.parameters {
                let value = &manifest.parameters[name].initial;
                parameter_data.extend_from_slice(value);
                parameter_data.resize(parameter_data.len().next_multiple_of(4), 0.);
            }
            ensure!(
                parameter_data.len() as u64 * 4
                    <= gpu.device.limits().max_uniform_buffer_binding_size,
                "parameter uniform exceeds device binding limit: {}",
                pass.id
            );
            let uniform = pass
                .bindings
                .iter()
                .any(|b| matches!(b.source, BindingSource::Parameters))
                .then(|| {
                    gpu.device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some(&pass.id),
                            contents: bytemuck::cast_slice(&parameter_data),
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        })
                });
            if uniform.is_some() {
                parameter_allocation += parameter_data.len() as u64 * 4;
            }
            ensure!(
                parameter_allocation <= caps.max_parameter_bytes,
                "prepared parameter memory budget exceeded"
            );
            let samplers: BTreeMap<_, _> = pass
                .bindings
                .iter()
                .filter_map(|b| {
                    let BindingSource::Sampler { u, v, filter } = b.source else {
                        return None;
                    };
                    Some((
                        (b.group, b.binding),
                        gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                            address_mode_u: u,
                            address_mode_v: v,
                            mag_filter: filter,
                            min_filter: filter,
                            mipmap_filter: match filter {
                                wgpu::FilterMode::Nearest => wgpu::MipmapFilterMode::Nearest,
                                wgpu::FilterMode::Linear => wgpu::MipmapFilterMode::Linear,
                            },
                            ..Default::default()
                        }),
                    ))
                })
                .collect();
            let group_count = pass.bindings.iter().map(|b| b.group + 1).max().unwrap_or(0);
            let make_groups = |frame| {
                (0..group_count)
                    .map(|group| {
                        let entries: Vec<_> = pass
                            .bindings
                            .iter()
                            .filter(|b| b.group == group)
                            .map(|b| wgpu::BindGroupEntry {
                                binding: b.binding,
                                resource: match &b.source {
                                    BindingSource::Parameters => {
                                        uniform.as_ref().unwrap().as_entire_binding()
                                    }
                                    BindingSource::Sampler { .. } => {
                                        wgpu::BindingResource::Sampler(
                                            &samplers[&(group, b.binding)],
                                        )
                                    }
                                    BindingSource::Resource { resource, previous } => {
                                        resources[resource].binding(frame, *previous)
                                    }
                                },
                            })
                            .collect();
                        let layout = match &pipeline {
                            Pipeline::Render(p) => p.get_bind_group_layout(group),
                            Pipeline::Compute(p) => p.get_bind_group_layout(group),
                        };
                        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            label: Some(&pass.id),
                            layout: &layout,
                            entries: &entries,
                        })
                    })
                    .collect()
            };
            let groups = [make_groups(0), make_groups(1)];
            passes.push(PreparedPass {
                description: pass.clone(),
                pipeline,
                groups,
                uniform,
                parameter_data,
                colors,
                depth,
            });
        }
        let parameters = manifest
            .parameters
            .iter()
            .map(|(n, p)| (n.clone(), p.initial.clone()))
            .collect();
        let mut result = Self {
            gpu,
            manifest,
            resources,
            passes,
            parameters,
            frame: 0,
            presented: 0,
            report: AllocationReport {
                dimensions,
                bytes: plan.resource_bytes + parameter_allocation,
            },
            audio: None,
        };
        result.reset()?;
        Ok(result)
    }
    pub fn update(&mut self, input: &BeatInput) -> Result<()> {
        // Validate the whole update before changing state or submitting writes.
        if let Some(audio) = &input.audio {
            ensure!(
                (8000..=192000).contains(&audio.sample_rate) && (1..=2).contains(&audio.channels),
                "invalid audio provider rate/channels"
            );
            ensure!(
                audio.encoding.as_ref().is_none_or(|s| !s.trim().is_empty()),
                "empty audio provider encoding"
            );
            if let Some(previous) = &self.audio {
                ensure!(
                    audio.generation >= previous.generation,
                    "stale audio generation"
                );
                if audio.generation == previous.generation {
                    ensure!(
                        audio.end_frame >= previous.end_frame
                            && audio.capture_end_ns >= previous.capture_end_ns,
                        "stale audio source timestamp"
                    );
                    ensure!(
                        audio.sample_rate == previous.sample_rate
                            && audio.channels == previous.channels
                            && audio.encoding == previous.encoding,
                        "audio provider changed without a new generation"
                    );
                }
            }
        }
        for (name, value) in &input.parameters {
            let prior = self
                .parameters
                .get(name)
                .with_context(|| format!("unknown input parameter: {name}"))?;
            ensure!(
                value.len() == prior.len() && value.iter().all(|v| v.is_finite()),
                "invalid input parameter: {name}"
            );
        }
        for (name, bytes) in &input.buffers {
            let Some(Allocated::Buffer { copies, .. }) = self.resources.get(name) else {
                bail!("unknown input buffer: {name}")
            };
            ensure!(
                bytes.len() as u64 == copies[0].size(),
                "input buffer size mismatch: {name}"
            );
            ensure!(
                !self.manifest.passes.iter().any(|p| p.writes.contains(name)),
                "input cannot overwrite graph-owned buffer: {name}"
            );
            ensure!(
                matches!(
                    self.manifest.resources[name],
                    Resource::Buffer { asset: None, .. }
                ),
                "input cannot overwrite asset buffer: {name}"
            );
        }
        if input.audio.as_ref().is_some_and(|next| {
            self.audio
                .as_ref()
                .is_some_and(|old| next.generation != old.generation)
        }) {
            self.reset()?;
        }
        if let Some(audio) = &input.audio {
            self.audio = Some(audio.clone());
        }
        for (name, value) in &input.parameters {
            self.parameters
                .get_mut(name)
                .unwrap()
                .copy_from_slice(value);
        }
        self.write_parameters();
        for (name, bytes) in &input.buffers {
            let Allocated::Buffer { copies, .. } = &self.resources[name] else {
                unreachable!()
            };
            for buffer in copies {
                self.gpu.queue.write_buffer(buffer, 0, bytes);
            }
        }
        Ok(())
    }
    fn write_parameters(&mut self) {
        for pass in &mut self.passes {
            if let Some(buffer) = &pass.uniform {
                let mut offset = 0;
                for name in &pass.description.parameters {
                    let value = &self.parameters[name];
                    pass.parameter_data[offset..offset + value.len()].copy_from_slice(value);
                    offset += value.len().next_multiple_of(4);
                }
                self.gpu
                    .queue
                    .write_buffer(buffer, 0, bytemuck::cast_slice(&pass.parameter_data));
            }
        }
    }
    pub fn render(&mut self) -> Result<()> {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        for prepared in &self.passes {
            match &prepared.pipeline {
                Pipeline::Render(pipeline) => {
                    let colors: Vec<_> = prepared
                        .colors
                        .iter()
                        .map(|name| {
                            Some(wgpu::RenderPassColorAttachment {
                                view: &self.resources[name].texture(self.frame, false).attachment,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })
                        })
                        .collect();
                    let depth = prepared.depth.as_ref().map(|name| {
                        wgpu::RenderPassDepthStencilAttachment {
                            view: &self.resources[name].texture(self.frame, false).attachment,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }
                    });
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some(&prepared.description.id),
                        color_attachments: &colors,
                        depth_stencil_attachment: depth,
                        ..Default::default()
                    });
                    pass.set_pipeline(pipeline);
                    for (i, group) in prepared.groups[self.frame].iter().enumerate() {
                        pass.set_bind_group(i as u32, group, &[]);
                    }
                    pass.draw(
                        0..prepared.description.draw.vertices,
                        0..prepared.description.draw.instances,
                    );
                }
                Pipeline::Compute(pipeline) => {
                    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some(&prepared.description.id),
                        ..Default::default()
                    });
                    pass.set_pipeline(pipeline);
                    for (i, group) in prepared.groups[self.frame].iter().enumerate() {
                        pass.set_bind_group(i as u32, group, &[]);
                    }
                    let [x, y, z] = prepared.description.dispatch.unwrap();
                    pass.dispatch_workgroups(x, y, z);
                }
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.presented = self.frame;
        self.frame ^= 1;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<()> {
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        for resource in self.resources.values() {
            match resource {
                Allocated::Buffer { copies, initial } => {
                    for buffer in copies {
                        if let Some(bytes) = initial {
                            self.gpu.queue.write_buffer(buffer, 0, bytes)
                        } else {
                            encoder.clear_buffer(buffer, 0, None)
                        }
                    }
                }
                Allocated::Texture { copies, initial } => {
                    for target in copies {
                        if let Some(levels) = initial {
                            for (level, bytes) in levels.iter().enumerate() {
                                upload(&self.gpu.queue, &target.texture, level as u32, bytes);
                            }
                        } else if target
                            .texture
                            .usage()
                            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                        {
                            let is_depth = target.texture.format().is_depth_stencil_format();
                            let colors = [(!is_depth).then_some(wgpu::RenderPassColorAttachment {
                                view: &target.attachment,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })];
                            let depth =
                                is_depth.then_some(wgpu::RenderPassDepthStencilAttachment {
                                    view: &target.attachment,
                                    depth_ops: target
                                        .texture
                                        .format()
                                        .has_depth_aspect()
                                        .then_some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                    stencil_ops: target
                                        .texture
                                        .format()
                                        .has_stencil_aspect()
                                        .then_some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                });
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                color_attachments: if is_depth { &[] } else { &colors },
                                depth_stencil_attachment: depth,
                                ..Default::default()
                            });
                        } else {
                            let size = target.texture.width() as usize
                                * target.texture.height() as usize
                                * target
                                    .texture
                                    .format()
                                    .block_copy_size(None)
                                    .context("texture cannot reset by upload")?
                                    as usize;
                            upload(&self.gpu.queue, &target.texture, 0, &vec![0; size]);
                        }
                    }
                }
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        for (name, value) in &mut self.parameters {
            value.copy_from_slice(&self.manifest.parameters[name].initial);
        }
        self.write_parameters();
        self.frame = 0;
        self.presented = 0;
        self.audio = None;
        Ok(())
    }
    pub fn input_descriptor(&self) -> Option<&BeatAudioDescriptor> {
        self.audio.as_ref()
    }
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.resources[&self.manifest.output]
            .texture(self.presented, false)
            .view
    }
    pub fn output_format(&self) -> wgpu::TextureFormat {
        self.resources[&self.manifest.output]
            .texture(self.presented, false)
            .texture
            .format()
    }
    /// Opt-in tightly packed native-format readback; never runs during render().
    pub fn capture(&self) -> Result<Vec<u8>> {
        let texture = &self.resources[&self.manifest.output]
            .texture(self.presented, false)
            .texture;
        let format = texture.format();
        ensure!(
            format.block_dimensions() == (1, 1),
            "capture of compressed output is unsupported"
        );
        let row = texture.width()
            * format
                .block_copy_size(None)
                .context("output capture format unsupported")?;
        let stride = row.div_ceil(256) * 256;
        let buffer = self.gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("beat-opt-in-capture"),
            size: stride as u64 * texture.height() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(texture.height()),
                },
            },
            texture.size(),
        );
        self.gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        self.gpu.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv().context("capture callback missing")??;
        let mapped = buffer.get_mapped_range(..)?;
        let mut result = Vec::with_capacity(row as usize * texture.height() as usize);
        for bytes in mapped.chunks(stride as usize) {
            result.extend_from_slice(&bytes[..row as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(result)
    }
    pub fn allocation_report(&self) -> AllocationReport {
        self.report.clone()
    }
}
fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, level: u32, bytes: &[u8]) {
    let (bw, bh) = texture.format().block_dimensions();
    let width = (texture.width() >> level).max(1);
    let height = (texture.height() >> level).max(1);
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: level,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(
                width.div_ceil(bw) * texture.format().block_copy_size(None).unwrap(),
            ),
            rows_per_image: Some(height.div_ceil(bh)),
        },
        wgpu::Extent3d {
            width: width.div_ceil(bw) * bw,
            height: height.div_ceil(bh) * bh,
            depth_or_array_layers: 1,
        },
    );
}
fn cooked_mips(
    bytes: &[u8],
    format: wgpu::TextureFormat,
    extent: [u32; 2],
    mips: u32,
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        !format.is_depth_stencil_format(),
        "binary depth assets are unsupported"
    );
    let (bw, bh) = format.block_dimensions();
    let stride = format
        .block_copy_size(None)
        .context("unsupported cooked texture format")? as usize;
    let mut offset = 0usize;
    let mut levels = Vec::new();
    for level in 0..mips {
        let count = ((extent[0] >> level).max(1).div_ceil(bw) as usize)
            .checked_mul((extent[1] >> level).max(1).div_ceil(bh) as usize)
            .and_then(|v| v.checked_mul(stride))
            .context("cooked texture size overflow")?;
        let end = offset
            .checked_add(count)
            .context("cooked texture size overflow")?;
        levels.push(
            bytes
                .get(offset..end)
                .context("cooked texture byte size mismatch")?
                .to_vec(),
        );
        offset = end;
    }
    ensure!(offset == bytes.len(), "cooked texture byte size mismatch");
    Ok(levels)
}
fn decode_image(
    bytes: &[u8],
    format: wgpu::TextureFormat,
    extent: [u32; 2],
    budget: u64,
) -> Result<Vec<u8>> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(extent[0]);
    limits.max_image_height = Some(extent[1]);
    limits.max_alloc = Some(budget);
    reader.limits(limits);
    let decoder = reader.into_decoder()?;
    ensure!(
        decoder.dimensions() == (extent[0], extent[1]),
        "image asset extent mismatch"
    );
    ensure!(
        decoder.total_bytes() <= budget,
        "decoded image exceeds memory budget"
    );
    let image = image::DynamicImage::from_decoder(decoder)?;
    match (format, image) {
        (
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb,
            image::DynamicImage::ImageRgba8(im),
        ) => Ok(im.into_raw()),
        (
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb,
            image::DynamicImage::ImageRgb8(im),
        ) => Ok(image::DynamicImage::ImageRgb8(im).into_rgba8().into_raw()),
        (wgpu::TextureFormat::R16Float, image::DynamicImage::ImageLuma16(im)) => Ok(im
            .into_raw()
            .into_iter()
            .flat_map(|v| {
                half::f16::from_f32(v as f32 / 65535.)
                    .to_bits()
                    .to_le_bytes()
            })
            .collect()),
        (wgpu::TextureFormat::R16Unorm, image::DynamicImage::ImageLuma16(im)) => Ok(im
            .into_raw()
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()),
        (wgpu::TextureFormat::R8Unorm, image::DynamicImage::ImageLuma8(im)) => Ok(im.into_raw()),
        _ => bail!(
            "unsupported image encoding/declared texture format; no implicit RGBA8 conversion"
        ),
    }
}
fn entry(module: &naga::Module, stage: naga::ShaderStage) -> Result<&str> {
    let entries: Vec<_> = module
        .entry_points
        .iter()
        .filter(|e| e.stage == stage)
        .collect();
    ensure!(
        entries.len() == 1,
        "exactly one {stage:?} entry point is required"
    );
    Ok(&entries[0].name)
}
fn validate_bindings(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
    pass: &Pass,
    manifest: &Manifest,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (handle, var) in module.global_variables.iter() {
        let Some(slot) = &var.binding else { continue };
        let binding = pass
            .bindings
            .iter()
            .find(|b| b.group == slot.group && b.binding == slot.binding)
            .with_context(|| {
                format!(
                    "{}: undeclared shader binding {}:{}",
                    pass.id, slot.group, slot.binding
                )
            })?;
        seen.insert((slot.group, slot.binding));
        match &binding.source {
            BindingSource::Parameters => ensure!(
                var.space == naga::AddressSpace::Uniform,
                "parameters must bind a uniform"
            ),
            BindingSource::Sampler { .. } => ensure!(
                matches!(
                    module.types[var.ty].inner,
                    naga::TypeInner::Sampler { comparison: false }
                ),
                "sampler binding type mismatch"
            ),
            BindingSource::Resource { resource, previous } => {
                let usage = module
                    .entry_points
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| match pass.kind {
                        PassKind::Render => matches!(
                            e.stage,
                            naga::ShaderStage::Vertex | naga::ShaderStage::Fragment
                        ),
                        PassKind::Compute => e.stage == naga::ShaderStage::Compute,
                    })
                    .fold(naga::valid::GlobalUse::empty(), |usage, (index, _)| {
                        usage | info.get_entry_point(index)[handle]
                    });
                ensure!(
                    !usage
                        .intersects(naga::valid::GlobalUse::READ | naga::valid::GlobalUse::ATOMIC)
                        || pass
                            .reads
                            .iter()
                            .any(|r| r.resource == *resource && r.previous == *previous),
                    "undeclared shader read: {resource}"
                );
                ensure!(
                    !usage
                        .intersects(naga::valid::GlobalUse::WRITE | naga::valid::GlobalUse::ATOMIC)
                        || (!previous && pass.writes.contains(resource)),
                    "undeclared shader write: {resource}"
                );
                match &manifest.resources[resource] {
                    Resource::Buffer { .. } => {
                        ensure!(
                            matches!(
                                var.space,
                                naga::AddressSpace::Storage { .. } | naga::AddressSpace::Uniform
                            ),
                            "resource buffer binding type mismatch"
                        );
                        if let naga::AddressSpace::Storage { access } = var.space {
                            ensure!(
                                !access.contains(naga::StorageAccess::STORE)
                                    || (!previous && pass.writes.contains(resource)),
                                "shader writes bypass declared buffer dependency: {resource}"
                            );
                        }
                    }
                    Resource::Texture { format, .. } => {
                        let naga::TypeInner::Image {
                            dim: naga::ImageDimension::D2,
                            arrayed: false,
                            class,
                        } = module.types[var.ty].inner
                        else {
                            bail!("texture binding must be 2D: {resource}")
                        };
                        match class {
                            naga::ImageClass::Depth { multi: false } => {
                                ensure!(format.has_depth_aspect(), "depth binding type mismatch")
                            }
                            naga::ImageClass::Sampled { multi: false, .. } => ensure!(
                                !format.is_depth_stencil_format(),
                                "color binding type mismatch"
                            ),
                            naga::ImageClass::Storage { access, .. } => ensure!(
                                !access.contains(naga::StorageAccess::STORE)
                                    || (!previous && pass.writes.contains(resource)),
                                "shader writes bypass declared texture dependency: {resource}"
                            ),
                            _ => bail!("unsupported multisampled texture binding"),
                        }
                    }
                }
            }
        }
    }
    ensure!(
        seen.len() == pass.bindings.len(),
        "{}: manifest binding has no shader declaration",
        pass.id
    );
    Ok(())
}
