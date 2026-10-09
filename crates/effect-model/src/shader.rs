use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub group: u32,
    pub binding: u32,
    pub name: String,
    pub kind: String,
    /// Stable source resource identity, independent of WGSL identifier mangling.
    #[serde(default)]
    pub resource: Option<String>,
}
impl Binding {
    pub fn resource_name(&self) -> String {
        self.resource
            .clone()
            .unwrap_or_else(|| resource_alias(&self.name))
    }
}

/// Strip compiler collision suffixes across Slang/Naga round trips. Digits
/// inside source names (rand00, blur2) remain part of the resource identity.
pub fn resource_alias(name: &str) -> String {
    let mut name = name.trim_matches('_');
    while let Some((head, tail)) = name.rsplit_once('_') {
        if tail.is_empty() || !tail.bytes().all(|c| c.is_ascii_digit()) {
            break;
        }
        name = head.trim_end_matches('_');
    }
    name.strip_suffix("_sampler").unwrap_or(name).to_owned()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shader {
    pub wgsl: String,
    pub bindings: Vec<Binding>,
    pub entry: String,
    #[serde(default)]
    pub texture_sizes: Vec<String>,
    #[serde(default)]
    pub loop_budget: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetPackage {
    pub schema: u32,
    pub source_id: String,
    pub source_name: String,
    pub source_sha256: String,
    pub split: String,
    pub equations: crate::equations::EquationBundle,
    pub warp: Option<Shader>,
    pub comp: Option<Shader>,
    pub adaptations: Vec<String>,
}
