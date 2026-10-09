//! Owned, serializable wgpu graph descriptions. No legacy preset semantics.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use wgpu_types::{
    AddressMode, Features, FilterMode, Limits, PrimitiveTopology, TextureFormat, TextureUsages,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub notices: Vec<String>,
    #[serde(default)]
    pub required_features: Vec<Features>,
    pub quality: Quality,
    pub output: String,
    pub assets: BTreeMap<String, Asset>,
    #[serde(default)]
    pub parameters: BTreeMap<String, Parameter>,
    pub resources: BTreeMap<String, Resource>,
    pub passes: Vec<Pass>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Native,
    Reduced,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    pub path: String,
    pub sha256: String,
    pub kind: AssetKind,
    #[serde(default)]
    pub license: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Wgsl,
    Image,
    Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub initial: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Resource {
    Texture {
        format: TextureFormat,
        extent: Extent,
        history: bool,
        role: TextureRole,
        mips: u32,
        color_space: ColorSpace,
        alpha: Alpha,
        #[serde(default)]
        asset: Option<String>,
    },
    Buffer {
        bytes: u64,
        history: bool,
        initialized: bool,
        #[serde(default)]
        asset: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum Extent {
    Native,
    Fixed { width: u32, height: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureRole {
    PrimaryFeedback,
    Output,
    Intermediate,
    Asset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorSpace {
    Linear,
    Srgb,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alpha {
    Opaque,
    Straight,
    Premultiplied,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pass {
    pub id: String,
    pub kind: PassKind,
    pub program: String,
    pub reads: Vec<Read>,
    pub writes: Vec<String>,
    #[serde(default)]
    pub parameters: Vec<String>,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default)]
    pub draw: Draw,
    #[serde(default)]
    pub dispatch: Option<[u32; 3]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    #[serde(default)]
    pub group: u32,
    pub binding: u32,
    pub source: BindingSource,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BindingSource {
    Resource {
        resource: String,
        #[serde(default)]
        previous: bool,
    },
    Parameters,
    Sampler {
        u: AddressMode,
        v: AddressMode,
        filter: FilterMode,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draw {
    pub vertices: u32,
    pub instances: u32,
    pub topology: PrimitiveTopology,
}
impl Default for Draw {
    fn default() -> Self {
        Self {
            vertices: 3,
            instances: 1,
            topology: PrimitiveTopology::TriangleList,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassKind {
    Render,
    Compute,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Read {
    pub resource: String,
    pub previous: bool,
    pub kind: ReadKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadKind {
    ColorTexture,
    DepthTexture,
    Buffer,
}

/// Values come from the requested device, not merely available adapter features.
#[derive(Clone, Debug)]
pub struct Capabilities {
    pub native_extent: [u32; 2],
    pub features: Features,
    pub limits: Limits,
    pub max_resource_bytes: u64,
    pub max_parameter_bytes: u64,
    pub max_passes: usize,
    pub max_draw_work: u64,
    pub max_dispatch_workgroups: u64,
}
impl Capabilities {
    pub fn baseline(native_extent: [u32; 2]) -> Self {
        Self {
            native_extent,
            features: Features::empty(),
            limits: Limits::default(),
            max_resource_bytes: 512 * 1024 * 1024,
            max_parameter_bytes: 16 * 1024 * 1024,
            max_passes: 4096,
            max_draw_work: 16 * 1024 * 1024,
            max_dispatch_workgroups: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExecutionPlan {
    pub pass_order: Vec<usize>,
    pub dimensions: BTreeMap<String, [u32; 2]>,
    pub resource_bytes: u64,
    pub parameter_bytes: u64,
    pub texture_usages: BTreeMap<String, TextureUsages>,
}

impl Resource {
    fn history(&self) -> bool {
        match self {
            Self::Texture { history, .. } | Self::Buffer { history, .. } => *history,
        }
    }
    fn initialized(&self) -> bool {
        match self {
            Self::Texture { history, asset, .. } => *history || asset.is_some(),
            Self::Buffer {
                history,
                initialized,
                asset,
                ..
            } => *history || *initialized || asset.is_some(),
        }
    }
    fn matches_read(&self, kind: ReadKind) -> bool {
        match (self, kind) {
            (Self::Texture { format, .. }, ReadKind::DepthTexture) => format.has_depth_aspect(),
            (Self::Texture { format, .. }, ReadKind::ColorTexture) => {
                !format.is_depth_stencil_format()
            }
            (Self::Buffer { .. }, ReadKind::Buffer) => true,
            _ => false,
        }
    }
}

impl Manifest {
    pub fn validate(&self, caps: &Capabilities) -> Result<ExecutionPlan> {
        ensure!(
            self.format == "beat" && self.version == 1,
            "unsupported beat format/version"
        );
        ensure!(
            !self.name.trim().is_empty() && !self.author.trim().is_empty(),
            "effect identity is empty"
        );
        ensure!(
            caps.native_extent.iter().all(|v| *v > 0),
            "invalid native extent"
        );
        ensure!(
            !self.passes.is_empty() && self.passes.len() <= caps.max_passes,
            "pass budget exceeded"
        );
        for required in &self.required_features {
            ensure!(
                caps.features.contains(*required),
                "required wgpu feature is not enabled"
            );
        }
        let mut paths = BTreeSet::new();
        for (id, asset) in &self.assets {
            ensure!(
                !id.is_empty() && safe_asset_path(&asset.path),
                "invalid asset path/identity: {id}"
            );
            ensure!(
                paths.insert(&asset.path),
                "duplicate asset path: {}",
                asset.path
            );
            ensure!(
                asset.sha256.len() == 64 && asset.sha256.bytes().all(|c| c.is_ascii_hexdigit()),
                "invalid asset hash: {id}"
            );
        }
        let mut parameter_bytes = 0u64;
        for (name, parameter) in &self.parameters {
            ensure!(
                !name.is_empty()
                    && !parameter.initial.is_empty()
                    && parameter.initial.iter().all(|v| v.is_finite()),
                "invalid/nonfinite parameter: {name}"
            );
            parameter_bytes = parameter_bytes
                .checked_add(
                    (parameter.initial.len() as u64)
                        .checked_mul(4)
                        .context("parameter size overflow")?,
                )
                .context("parameter size overflow")?;
        }
        ensure!(
            parameter_bytes <= caps.max_parameter_bytes,
            "parameter budget exceeded"
        );
        let mut dimensions = BTreeMap::new();
        let mut resource_bytes = 0u64;
        let mut usages = BTreeMap::new();
        for (name, resource) in &self.resources {
            ensure!(!name.is_empty(), "empty resource identity");
            let (size, asset, expected_asset) = match resource {
                Resource::Texture {
                    format,
                    extent,
                    history,
                    role,
                    mips,
                    color_space,
                    alpha,
                    asset,
                } => {
                    ensure!(
                        caps.features.contains(format.required_features()),
                        "texture feature is not enabled: {name}"
                    );
                    let [w, h] = match extent {
                        Extent::Native => caps.native_extent,
                        Extent::Fixed { width, height } => [*width, *height],
                    };
                    ensure!(
                        w > 0
                            && h > 0
                            && w <= caps.limits.max_texture_dimension_2d
                            && h <= caps.limits.max_texture_dimension_2d,
                        "texture extent exceeds device limits: {name}"
                    );
                    let (block_width, block_height) = format.block_dimensions();
                    ensure!(
                        w.is_multiple_of(block_width) && h.is_multiple_of(block_height),
                        "texture base extent is not block aligned: {name}"
                    );
                    if self.quality == Quality::Native
                        && matches!(role, TextureRole::PrimaryFeedback | TextureRole::Output)
                    {
                        ensure!(
                            [w, h] == caps.native_extent,
                            "native primary/output extent required: {name}"
                        );
                    }
                    ensure!(
                        *mips > 0 && *mips <= w.max(h).ilog2() + 1,
                        "invalid mip count: {name}"
                    );
                    ensure!(
                        (*color_space == ColorSpace::Srgb) == format.is_srgb(),
                        "texture color-space/format mismatch: {name}"
                    );
                    if format.is_depth_stencil_format() {
                        ensure!(
                            *color_space == ColorSpace::Data
                                && *alpha == Alpha::Opaque
                                && *mips == 1,
                            "invalid depth interpretation: {name}"
                        );
                    }
                    let block_size = format
                        .block_copy_size(None)
                        .context("unsupported texture storage estimate")?
                        as u64;
                    let (bw, bh) = format.block_dimensions();
                    let mut bytes = 0u64;
                    for level in 0..*mips {
                        let lw = (w >> level).max(1).div_ceil(bw) as u64;
                        let lh = (h >> level).max(1).div_ceil(bh) as u64;
                        bytes = bytes
                            .checked_add(
                                lw.checked_mul(lh)
                                    .and_then(|v| v.checked_mul(block_size))
                                    .context("texture size overflow")?,
                            )
                            .context("texture size overflow")?;
                    }
                    if *history {
                        bytes = bytes.checked_mul(2).context("history size overflow")?;
                    }
                    dimensions.insert(name.clone(), [w, h]);
                    usages.insert(
                        name.clone(),
                        if asset.is_some() {
                            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST
                        } else {
                            TextureUsages::empty()
                        },
                    );
                    (bytes, asset, AssetKind::Image)
                }
                Resource::Buffer {
                    bytes,
                    history,
                    asset,
                    ..
                } => {
                    ensure!(
                        *bytes > 0 && *bytes <= caps.limits.max_buffer_size,
                        "buffer exceeds device limits: {name}"
                    );
                    let bytes = bytes
                        .checked_mul(if *history { 2 } else { 1 })
                        .context("buffer size overflow")?;
                    (bytes, asset, AssetKind::Binary)
                }
            };
            if let Some(asset) = asset {
                ensure!(
                    self.assets
                        .get(asset)
                        .is_some_and(|a| a.kind == expected_asset
                            || (matches!(resource, Resource::Texture { .. })
                                && a.kind == AssetKind::Binary)),
                    "missing/wrong resource asset: {name}"
                );
            }
            resource_bytes = resource_bytes
                .checked_add(size)
                .context("resource size overflow")?;
            ensure!(
                resource_bytes <= caps.max_resource_bytes,
                "resource memory budget exceeded"
            );
        }
        let output = self
            .resources
            .get(&self.output)
            .context("unknown output resource")?;
        ensure!(
            matches!(output,Resource::Texture{format,role:TextureRole::Output,..} if !format.is_depth_stencil_format()),
            "output must be a color texture"
        );
        // The public final view is consumed by the platform presentation pass,
        // outside the effect graph. Validate its sampling usage before allocation.
        *usages.get_mut(&self.output).unwrap() |= TextureUsages::TEXTURE_BINDING;
        let mut ids = BTreeSet::new();
        let mut writers = BTreeMap::new();
        for (index, pass) in self.passes.iter().enumerate() {
            ensure!(
                !pass.id.is_empty() && ids.insert(&pass.id),
                "duplicate/empty pass identity"
            );
            ensure!(
                self.assets
                    .get(&pass.program)
                    .is_some_and(|a| a.kind == AssetKind::Wgsl),
                "missing/wrong program: {}",
                pass.id
            );
            ensure!(!pass.writes.is_empty(), "pass has no output: {}", pass.id);
            self.validate_executable_metadata(pass, caps)?;
            if pass.kind == PassKind::Compute {
                ensure!(
                    caps.limits.max_compute_workgroups_per_dimension > 0,
                    "compute is unsupported on this device"
                );
            }
            for parameter in &pass.parameters {
                ensure!(
                    self.parameters.contains_key(parameter),
                    "unknown parameter: {parameter}"
                );
            }
            for write in &pass.writes {
                let resource = self
                    .resources
                    .get(write)
                    .context("unknown resource written by pass")?;
                ensure!(
                    !matches!(
                        resource,
                        Resource::Texture {
                            role: TextureRole::Asset,
                            ..
                        }
                    ),
                    "shared asset texture is immutable: {write}"
                );
                if writers.insert(write, index).is_some() {
                    bail!("multiple writers for resource: {write}");
                }
                match (pass.kind, resource) {
                    (PassKind::Render, Resource::Buffer { .. }) => {
                        bail!("render attachment must be a texture")
                    }
                    (kind, Resource::Texture { .. }) => {
                        *usages.get_mut(write).unwrap() |= if kind == PassKind::Render {
                            TextureUsages::RENDER_ATTACHMENT
                        } else {
                            TextureUsages::STORAGE_BINDING
                        };
                    }
                    _ => {}
                }
            }
            if pass.kind == PassKind::Render {
                self.validate_attachments(pass, &dimensions, caps)?;
            }
        }
        ensure!(writers.contains_key(&self.output), "output has no writer");
        let mut outgoing = vec![BTreeSet::new(); self.passes.len()];
        let mut indegree = vec![0usize; self.passes.len()];
        for (index, pass) in self.passes.iter().enumerate() {
            for read in &pass.reads {
                let resource = self
                    .resources
                    .get(&read.resource)
                    .context("unknown resource read by pass")?;
                ensure!(
                    resource.matches_read(read.kind),
                    "binding type mismatch: {}",
                    read.resource
                );
                if let Some(usage) = usages.get_mut(&read.resource) {
                    *usage |= TextureUsages::TEXTURE_BINDING;
                }
                if read.previous {
                    ensure!(
                        resource.history(),
                        "previous-frame read requires history: {}",
                        read.resource
                    );
                } else if let Some(writer) = writers.get(&read.resource) {
                    if outgoing[*writer].insert(index) {
                        indegree[index] += 1;
                    }
                } else {
                    ensure!(
                        resource.initialized(),
                        "resource read before initialization: {}",
                        read.resource
                    );
                }
            }
        }
        for (name, usage) in &usages {
            ensure!(!usage.is_empty(), "unused texture has no GPU usage: {name}");
            if let Resource::Texture { format, .. } = &self.resources[name] {
                ensure!(
                    format
                        .guaranteed_format_features(caps.features)
                        .allowed_usages
                        .contains(*usage),
                    "texture usages unsupported by device feature contract: {name}"
                );
            }
        }
        let mut ready: VecDeque<_> = indegree
            .iter()
            .enumerate()
            .filter_map(|(i, d)| (*d == 0).then_some(i))
            .collect();
        let mut pass_order = Vec::with_capacity(self.passes.len());
        while let Some(index) = ready.pop_front() {
            pass_order.push(index);
            for target in &outgoing[index] {
                indegree[*target] -= 1;
                if indegree[*target] == 0 {
                    ready.push_back(*target);
                }
            }
        }
        ensure!(
            pass_order.len() == self.passes.len(),
            "same-frame graph cycle"
        );
        Ok(ExecutionPlan {
            pass_order,
            dimensions,
            resource_bytes,
            parameter_bytes,
            texture_usages: usages,
        })
    }

    fn validate_executable_metadata(&self, pass: &Pass, caps: &Capabilities) -> Result<()> {
        let mut slots = BTreeSet::new();
        let mut parameter_slots = 0;
        for binding in &pass.bindings {
            ensure!(
                binding.group < caps.limits.max_bind_groups
                    && binding.binding < caps.limits.max_bindings_per_bind_group
                    && slots.insert((binding.group, binding.binding)),
                "invalid/duplicate binding slot: {}",
                pass.id
            );
            match &binding.source {
                BindingSource::Resource { resource, previous } => {
                    ensure!(
                        self.resources.contains_key(resource),
                        "unknown binding resource: {resource}"
                    );
                    ensure!(
                        pass.reads
                            .iter()
                            .any(|r| r.resource == *resource && r.previous == *previous)
                            || (pass.kind == PassKind::Compute
                                && !previous
                                && pass.writes.contains(resource)),
                        "binding bypasses declared graph access: {resource}"
                    );
                }
                BindingSource::Parameters => {
                    parameter_slots += 1;
                    ensure!(!pass.parameters.is_empty(), "empty parameter binding");
                }
                BindingSource::Sampler { u, v, .. } => {
                    ensure!(
                        *u != AddressMode::ClampToBorder && *v != AddressMode::ClampToBorder,
                        "border samplers require a separately declared capability"
                    );
                }
            }
        }
        ensure!(parameter_slots <= 1, "duplicate parameter binding");
        if pass.kind == PassKind::Render {
            ensure!(
                pass.draw.vertices > 0
                    && pass.draw.instances > 0
                    && pass.draw.vertices as u64 * pass.draw.instances as u64 <= caps.max_draw_work,
                "draw work budget exceeded: {}",
                pass.id
            );
            ensure!(
                pass.dispatch.is_none(),
                "render pass cannot dispatch compute"
            );
        } else if let Some(dispatch) = pass.dispatch {
            ensure!(
                dispatch
                    .iter()
                    .all(|n| *n > 0 && *n <= caps.limits.max_compute_workgroups_per_dimension),
                "dispatch extent exceeds device limits"
            );
            let total = dispatch
                .iter()
                .try_fold(1u64, |a, b| a.checked_mul(*b as u64))
                .context("dispatch overflow")?;
            ensure!(
                total <= caps.max_dispatch_workgroups,
                "dispatch work budget exceeded"
            );
        }
        Ok(())
    }

    fn validate_attachments(
        &self,
        pass: &Pass,
        dimensions: &BTreeMap<String, [u32; 2]>,
        caps: &Capabilities,
    ) -> Result<()> {
        let mut extent = None;
        let mut depth_count = 0;
        let mut colors = Vec::new();
        for name in &pass.writes {
            let Resource::Texture { format, .. } = &self.resources[name] else {
                bail!("render attachment must be a texture");
            };
            let size = dimensions[name];
            if let Some(expected) = extent {
                ensure!(
                    size == expected,
                    "render attachment extent mismatch: {}",
                    pass.id
                );
            } else {
                extent = Some(size);
            }
            if format.is_depth_stencil_format() {
                depth_count += 1;
                ensure!(depth_count <= 1, "multiple depth attachments: {}", pass.id);
            } else {
                colors.push(*format);
            }
        }
        ensure!(
            colors.len() <= caps.limits.max_color_attachments as usize,
            "render color attachment count exceeds device limit: {}",
            pass.id
        );
        let mut cost = 0u32;
        for format in colors {
            let alignment = format
                .target_component_alignment()
                .context("invalid color attachment format")?;
            let bytes = format
                .target_pixel_byte_cost()
                .context("invalid color attachment format")?;
            cost = cost
                .checked_next_multiple_of(alignment)
                .and_then(|value| value.checked_add(bytes))
                .context("color attachment byte cost overflow")?;
        }
        ensure!(
            cost <= caps.limits.max_color_attachment_bytes_per_sample,
            "render attachment byte cost exceeds device limit: {}",
            pass.id
        );
        Ok(())
    }
}

/// Portable archive names; do not treat host path normalization as validation.
pub fn safe_asset_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}
