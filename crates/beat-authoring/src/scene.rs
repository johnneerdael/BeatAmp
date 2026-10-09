use crate::{Prediction, SourceHints, prediction::digest};
use anyhow::{Result, ensure};
use effect_model::beat::*;
use effect_package::Package;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use wgpu_types::{AddressMode, FilterMode, TextureFormat};

const COMPONENTS: [&str; 5] = ["silk", "crown", "lattice", "interference", "folding"];
const PARAMETERS: [&str; 11] = [
    "scene_time",
    "audio_energy",
    "audio_regions",
    "audio_chroma",
    "audio_space",
    "audio_windows",
    "audio_frequency",
    "scene_style",
    "palette_a",
    "palette_b",
    "palette_c",
];

pub struct SynthesisRequest<'a> {
    pub name: &'a str,
    pub source: &'a SourceHints,
    pub prediction: Option<&'a Prediction>,
    pub capabilities: &'a Capabilities,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceTarget {
    pub feature: String,
    pub original_f64: f64,
    pub target: String,
    pub interpretation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptationMetadata {
    pub authoring_version: String,
    pub source_sha256: String,
    pub source_name: String,
    pub source_hints: SourceHints,
    pub prediction_status: String,
    pub prediction_file_sha256: Option<String>,
    pub prediction_context: Option<Value>,
    pub prediction_run_identity: Option<Value>,
    pub unknown_features: Vec<String>,
    pub evidence_targets: Vec<EvidenceTarget>,
    pub components: Vec<String>,
    pub choices: Vec<String>,
    pub limitations: Vec<String>,
    pub audio_contract: Value,
}
pub struct SynthesisResult {
    pub package: Package,
    pub metadata: AdaptationMetadata,
}
fn texture(role: TextureRole, history: bool) -> Resource {
    Resource::Texture {
        format: if role == TextureRole::Output {
            TextureFormat::Rgba8UnormSrgb
        } else {
            TextureFormat::Rgba16Float
        },
        extent: Extent::Native,
        history,
        role,
        mips: 1,
        color_space: if role == TextureRole::Output {
            ColorSpace::Srgb
        } else {
            ColorSpace::Linear
        },
        alpha: Alpha::Opaque,
        asset: None,
    }
}
fn resource_binding(binding: u32, name: &str, previous: bool) -> Binding {
    Binding {
        group: 0,
        binding,
        source: BindingSource::Resource {
            resource: name.into(),
            previous,
        },
    }
}
fn sampler(binding: u32) -> Binding {
    Binding {
        group: 0,
        binding,
        source: BindingSource::Sampler {
            u: AddressMode::ClampToEdge,
            v: AddressMode::ClampToEdge,
            filter: FilterMode::Linear,
        },
    }
}
fn read(resource: &str, previous: bool, kind: ReadKind) -> Read {
    Read {
        resource: resource.into(),
        previous,
        kind,
    }
}
fn pass(id: &str, program: &str, reads: Vec<Read>, writes: &str, bindings: Vec<Binding>) -> Pass {
    Pass {
        id: id.into(),
        kind: PassKind::Render,
        program: program.into(),
        reads,
        writes: vec![writes.into()],
        parameters: PARAMETERS.into_iter().map(str::to_owned).collect(),
        bindings,
        draw: Draw::default(),
        dispatch: None,
    }
}
fn asset(
    manifest: &mut Manifest,
    assets: &mut BTreeMap<String, Vec<u8>>,
    id: &str,
    path: &str,
    kind: AssetKind,
    bytes: Vec<u8>,
) {
    manifest.assets.insert(id.into(),Asset {path:path.into(),sha256:digest(&bytes),kind,license:Some("Original Beatamp authoring code; source and evidence attribution in adaptation.json".into())});
    assets.insert(id.into(), bytes);
}
fn colour(seed: u8, offset: f32) -> [f32; 3] {
    let h = seed as f32 / 255.0 * std::f32::consts::TAU + offset;
    [
        0.12 + 0.88 * (0.5 + 0.5 * h.cos()),
        0.12 + 0.88 * (0.5 + 0.5 * (h + 2.094).cos()),
        0.12 + 0.88 * (0.5 + 0.5 * (h + 4.188).cos()),
    ]
}
fn target(
    prediction: Option<&Prediction>,
    targets: &mut Vec<EvidenceTarget>,
    feature: &str,
    name: &str,
    default: f64,
    min: f64,
    max: f64,
) -> f64 {
    if let Some(value) = prediction.and_then(|p| p.scalar(feature)) {
        targets.push(EvidenceTarget {feature:feature.into(),original_f64:value,target:name.into(),interpretation:"Artistic bounded mapping of source-field statistic; no geometry/fidelity or live-causality claim".into()});
        value.clamp(min, max)
    } else {
        default
    }
}
/// Composes a bounded independently authored native2D vocabulary. No source
/// shader/evaluator executes in this package; exact source fidelity is optional.
pub fn synthesize(request: &SynthesisRequest<'_>) -> Result<SynthesisResult> {
    ensure!(!request.name.trim().is_empty(), "empty source name");
    ensure!(
        request.source.sha256.len() == 64
            && request
                .source
                .sha256
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)),
        "invalid source identity"
    );
    ensure!(
        request.source.numeric.values().all(|v| v.is_finite()),
        "nonfinite source hints"
    );
    if let Some(prediction) = request.prediction {
        prediction.associate(request.source)?;
    }
    let hash: Vec<u8> = (0..32)
        .map(|i| u8::from_str_radix(&request.source.sha256[i * 2..i * 2 + 2], 16).unwrap())
        .collect();
    let count = 2 + usize::from(hash[0].is_multiple_of(3));
    let mut selected = vec![hash[1] as usize % COMPONENTS.len()];
    for offset in 1..count {
        selected.push(
            (selected[0] + 1 + (hash[offset + 1] as usize % (COMPONENTS.len() - 1)))
                % COMPONENTS.len(),
        );
    }
    selected.sort_unstable();
    selected.dedup();
    if selected.len() < count {
        for candidate in 0..COMPONENTS.len() {
            if !selected.contains(&candidate) {
                selected.push(candidate);
                if selected.len() == count {
                    break;
                }
            }
        }
    }
    let feedback = hash[5] & 1 == 1;
    let mut targets = Vec::new();
    let saturation = target(
        request.prediction,
        &mut targets,
        "colour.mean_saturation",
        "palette saturation",
        0.8,
        0.65,
        1.0,
    ) as f32;
    let luma = target(
        request.prediction,
        &mut targets,
        "colour.mean_luma",
        "display exposure",
        0.24,
        0.08,
        0.65,
    ) as f32;
    let movement = target(
        request.prediction,
        &mut targets,
        "motion.median_speed_viewports_per_second",
        "continuous motion rate",
        0.15,
        0.02,
        0.75,
    ) as f32;
    let warm_cool = target(
        request.prediction,
        &mut targets,
        "colour.warm_cool",
        "palette warm/cool offset",
        0.0,
        -1.0,
        1.0,
    ) as f32;
    let speed = (0.10 + hash[6] as f32 / 255.0 * 0.18 + movement * 0.15).min(0.5);
    let thickness = 8.0 + hash[7] as f32 / 255.0 * 6.0; //4–7px half width in normalized height coordinates.
    let decay = request
        .source
        .numeric
        .get("fDecay")
        .copied()
        .unwrap_or(0.89)
        .clamp(0.55, 0.965) as f32;
    let mut palette_a = colour(hash[8], warm_cool * 0.5);
    for (index, key) in ["fWaveR", "fWaveG", "fWaveB"].into_iter().enumerate() {
        if let Some(value) = request.source.numeric.get(key) {
            palette_a[index] = (*value as f32).clamp(0.12, 1.0);
        }
    }
    let palettes = [palette_a, colour(hash[8], 2.0), colour(hash[9], 4.0)].map(|color| {
        let grey = (color[0] + color[1] + color[2]) / 3.0;
        color.map(|v| grey + (v - grey) * saturation)
    });
    let mut manifest=Manifest {format:"beat".into(),version:1,name:request.name.into(),author:request.source.author.clone().unwrap_or_else(||"Beatamp procedural authoring; original source authors identified by source name".into()),notices:vec![format!("Source attribution and original name: {}",request.name),"Generated independently from source hints and optional offline evidence; artistic adaptation, not reconstructed executable geometry".into()],required_features:vec![],quality:Quality::Native,output:"display".into(),assets:BTreeMap::new(),parameters:BTreeMap::new(),resources:BTreeMap::new(),passes:vec![]};
    let values = [
        vec![
            0.0,
            1.0 / 30.0,
            request.capabilities.native_extent[0] as f32,
            request.capabilities.native_extent[1] as f32,
        ],
        vec![0.0; 4],
        vec![0.0; 8],
        vec![0.0; 12],
        vec![0.0; 4],
        vec![0.0, 0.0, -1.0, -1.0],
        vec![0.0; 4],
        vec![hash[10] as f32 / 255.0, speed, thickness, decay],
        vec![
            palettes[0][0],
            palettes[0][1],
            palettes[0][2],
            1.2 + luma * 2.0,
        ],
        vec![palettes[1][0], palettes[1][1], palettes[1][2], 0.0],
        vec![palettes[2][0], palettes[2][1], palettes[2][2], 0.0],
    ];
    manifest.parameters = PARAMETERS
        .into_iter()
        .zip(values)
        .map(|(name, initial)| (name.into(), Parameter { initial }))
        .collect();
    manifest
        .resources
        .insert("field".into(), texture(TextureRole::Intermediate, false));
    manifest
        .resources
        .insert("display".into(), texture(TextureRole::Output, false));
    manifest.resources.insert(
        "audio_fast".into(),
        Resource::Buffer {
            bytes: 513 * 16,
            history: false,
            initialized: true,
            asset: None,
        },
    );
    manifest.resources.insert(
        "audio_detail".into(),
        Resource::Buffer {
            bytes: 2049 * 16,
            history: false,
            initialized: true,
            asset: None,
        },
    );
    let params = Binding {
        group: 0,
        binding: 0,
        source: BindingSource::Parameters,
    };
    let mut assets = BTreeMap::new();
    let common = include_str!("common.wgsl");
    let mut scene = format!(
        "{}\n{}\n@fragment fn fragment_main(@builtin(position) pixel:vec4<f32>)->@location(0) vec4<f32> {{\n",
        common,
        include_str!("components.wgsl")
    );
    scene.push_str("let uv=pixel.xy/control.time.zw;\nlet p=(pixel.xy*2.0-control.time.zw)/control.time.w;\nlet t=control.time.x*control.style.y;\nlet radial=length(p);\nlet angular=atan2(p.y,p.x)/6.2831853+0.5;\nlet tonal=dot(control.chroma_a,vec4<f32>(0.11,0.23,0.37,0.41))+dot(control.chroma_b,vec4<f32>(0.47,0.53,0.67,0.71))+dot(control.chroma_c,vec4<f32>(0.79,0.83,0.89,0.97));\nlet full=spectrum(uv.x,p.x);\nlet spectral=spectrum(angular,p.x);\nlet regions=dot(control.regions_lo,vec4<f32>(0.1,0.3,0.25,0.25))+dot(control.regions_hi,vec4<f32>(0.3,0.35,0.0,0.0));\nvar color=palette(radial+t*0.2+tonal*0.6)*(0.012+0.035*regions)*exp(-radial*0.7);\n");
    for (layer, index) in selected.iter().enumerate() {
        let phase = hash[12 + layer] as f32 / 255.0 * std::f32::consts::TAU;
        scene.push_str(&format!(
            "color += {}(rotation(p,{:.6}+t*{:.6}),t,{:.6}+tonal*0.7,{}+regions*0.24)*{:.6};\n",
            COMPONENTS[*index],
            phase,
            if layer == 0 { 0.035 } else { -0.025 },
            phase,
            if layer % 2 == 0 { "full" } else { "spectral" },
            if layer == 0 { 0.9 } else { 0.38 }
        ));
    }
    scene.push_str("color *= 0.8+control.energy.x*0.7+control.energy.y*0.35;\nreturn vec4<f32>(color,1.0);\n}\n");
    asset(
        &mut manifest,
        &mut assets,
        "scene_program",
        "programs/scene.wgsl",
        AssetKind::Wgsl,
        scene.into_bytes(),
    );
    manifest.passes.push(pass(
        "native_scene",
        "scene_program",
        vec![
            read("audio_fast", false, ReadKind::Buffer),
            read("audio_detail", false, ReadKind::Buffer),
        ],
        "field",
        vec![
            params.clone(),
            resource_binding(1, "audio_fast", false),
            resource_binding(2, "audio_detail", false),
        ],
    ));
    let present_input = if feedback {
        manifest.resources.insert(
            "transport".into(),
            texture(TextureRole::PrimaryFeedback, true),
        );
        asset(
            &mut manifest,
            &mut assets,
            "feedback_program",
            "programs/feedback.wgsl",
            AssetKind::Wgsl,
            format!("{common}\n{}", include_str!("feedback.wgsl")).into_bytes(),
        );
        manifest.passes.push(pass(
            "native_feedback",
            "feedback_program",
            vec![
                read("field", false, ReadKind::ColorTexture),
                read("transport", true, ReadKind::ColorTexture),
            ],
            "transport",
            vec![
                params.clone(),
                resource_binding(1, "field", false),
                resource_binding(2, "transport", true),
                sampler(3),
            ],
        ));
        "transport"
    } else {
        "field"
    };
    asset(
        &mut manifest,
        &mut assets,
        "present_program",
        "programs/present.wgsl",
        AssetKind::Wgsl,
        format!("{common}\n{}", include_str!("present.wgsl")).into_bytes(),
    );
    manifest.passes.push(pass(
        "native_display",
        "present_program",
        vec![read(present_input, false, ReadKind::ColorTexture)],
        "display",
        vec![
            params,
            resource_binding(1, present_input, false),
            sampler(2),
        ],
    ));
    let unknown_features = request
        .prediction
        .and_then(|p| p.envelope()["feature_record"]["features"].as_object())
        .map(|f| {
            f.iter()
                .filter(|(_, v)| v["status"] == "unknown")
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    let status = request
        .prediction
        .map_or("pending", |p| p.status.as_str())
        .to_owned();
    let mut choices=vec!["Source SHA256 deterministically selects 2–3 distinct components, geometric phases, palettes, native thickness and optional history transport".into(),"Native independently evaluated analytic geometry; full-screen topology and lighting are explicit original artistic choices".into(),"Fast full-spectrum spatial sampling plus independent full-frequency region energy; detail spectrum enriches rather than gates fast response".into()];
    choices.push("Deliberate artistic palette saturation floor 0.65 preserves colour contrast when source-field saturation is low; original f64 evidence remains unchanged".into());
    if status != "computed" {
        choices.push("Prediction absent/noncomputed: source-guided original adaptation; prediction-derived support is not claimed".into());
    }
    if request
        .source
        .code_sections
        .keys()
        .any(|k| k.starts_with("wave") || k.starts_with("shape"))
    {
        choices.push("Custom source wave/shape code is an unproven hint; no inferred tunnel/fractal/stationary geometry claim".into());
    }
    let metadata=AdaptationMetadata {authoring_version:"beat-authoring-native2d-v2".into(),source_sha256:request.source.sha256.clone(),source_name:request.name.into(),source_hints:request.source.clone(),prediction_status:status,prediction_file_sha256:request.prediction.map(|p|p.file_sha256.clone()),prediction_context:request.prediction.and_then(|p|p.envelope()["feature_record"]["context"].as_object().map(|_|p.envelope()["feature_record"]["context"].clone())),prediction_run_identity:request.prediction.and_then(|p|p.envelope().get("run_identity").cloned()),unknown_features,evidence_targets:targets,components:selected.iter().map(|i|COMPONENTS[*i].to_owned()).collect(),choices,limitations:vec!["Exact Python reference hashes require the external reference consumer; Rust preserves original bytes and validates structure/source association only".into(),"Source-field statistics do not uniquely identify executable geometry, fidelity, live beat triggers or instruments".into(),"Native allocation and shader validity are not native4K/30 performance or premium visual-quality evidence".into()],audio_contract:serde_json::json!({"version":1,"fast_fft":1024,"fast_bins":513,"detail_fft":4096,"detail_bins":2049,"range":"DC through actual Nyquist","channels":"independent left/right power; mono right unavailable","row_layout":["raw_left_power","raw_right_power","smoothed_left_amplitude","smoothed_right_amplitude"],"row_format":"little endian vec4<f32>","attack_seconds":0.010,"release_seconds":0.180,"spectral_amplitude_curve":"10*sqrt(power)/(1+10*sqrt(power))","parameters_order":PARAMETERS,"detail_gates_fast":false,"recorded_flashing_events_are_live_triggers":false,"stem_separation":false})};
    asset(
        &mut manifest,
        &mut assets,
        "adaptation_metadata",
        "adaptation.json",
        AssetKind::Binary,
        serde_json::to_vec_pretty(&metadata)?,
    );
    if let Some(prediction) = request.prediction {
        asset(
            &mut manifest,
            &mut assets,
            "prediction_evidence",
            "evidence/prediction.json",
            AssetKind::Binary,
            prediction.original_bytes().to_vec(),
        );
    }
    manifest.validate(request.capabilities)?;
    let package = Package { manifest, assets };
    package.validate_contents()?;
    Ok(SynthesisResult { package, metadata })
}
