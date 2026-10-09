use anyhow::{Context, Result, ensure};
use effect_model::shader::{PresetPackage, Shader};
use milk_import::{corpus, parser};
use std::{collections::BTreeSet, path::Path};

/// Promote verified shader-route artifacts and equation IR into a native package.
/// The source hash/rank must match the frozen corpus; shader loops are bounded
/// in Naga IR. Compiler orchestration/source normalization is a separate route.
pub fn from_artifacts(root: &Path, entry: &corpus::Entry) -> Result<PresetPackage> {
    let bytes = corpus::verify_entry(root, entry, "presets")?;
    let source = parser::parse(&bytes)?;
    let rank = entry.selection_rank.context("preset rank missing")?;
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(
        root.join("reports/shader-portability.json"),
    )?)?;
    let load = |stage: &str| -> Result<Option<Shader>> {
        let Some(lines) = source.sections.get(stage) else {
            return Ok(None);
        };
        let path = root
            .join("tools/shader-spike/output")
            .join(format!("{rank:03}-{stage}.shader.json"));
        let raw = std::fs::read(&path)
            .with_context(|| format!("authored {stage} shader artifact unavailable"))?;
        let evidence = report["shaders"]
            .as_array()
            .context("shader identity evidence missing")?
            .iter()
            .find(|r| r["rank"].as_u64() == Some(rank as u64) && r["stage"].as_str() == Some(stage))
            .context("shader identity row missing")?;
        let digest = corpus::sha256(&raw);
        ensure!(
            evidence["name"].as_str() == Some(entry.path.as_str())
                && evidence["artifact_sha256"].as_str() == Some(digest.as_str()),
            "shader artifact/source identity mismatch"
        );
        let original: Shader = serde_json::from_slice(&raw)?;
        let mut shader = crate::shader::bounded_wgsl(&original.wgsl, 64)?;
        // Modern pixel-center rasterization never reaches exactly UV (0,0).
        // Preserve explicitly authored origin seeds as 3-native-pixel cells in
        // adapted mode; keep their original predicate in comparison mode.
        let compact: String = lines
            .iter()
            .map(|line| line.text.split_whitespace().collect::<String>())
            .collect();
        if compact.contains("uv_orig.x==0") && compact.contains("uv_orig.y==0") {
            let assignment = "= coord_0_;";
            ensure!(
                shader.wgsl.matches(assignment).count() == 1,
                "origin seed adapter requires identified compiler wrapper"
            );
            let uniform_access = shader
                .wgsl
                .lines()
                .find_map(|line| {
                    let (_, access) = line.split_once("= MilkParams_0_.")?;
                    let (member, _) = access.split_once("[3]")?;
                    Some(format!("MilkParams_0_.{member}[20]"))
                })
                .context("origin seed uniform ABI not found")?;
            let replacement = format!(
                "= select(coord_0_, floor(coord_0_ * {uniform_access}.xy / 3.0) * 3.0 / {uniform_access}.xy, {uniform_access}.w > 0.5);"
            );
            shader.wgsl = shader.wgsl.replace(assignment, &replacement);
            // Revalidate transformed source with the declared shader frontend.
            crate::shader::from_wgsl(&shader.wgsl)?;
        }
        let mut sizes = BTreeSet::new();
        for line in lines {
            for word in line
                .text
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            {
                if let Some(name) = word.strip_prefix("texsize_") {
                    sizes.insert(name.to_owned());
                }
            }
        }
        ensure!(
            sizes.len() <= 32,
            "shader texture-size ABI capacity exceeded"
        );
        shader.texture_sizes = sizes.into_iter().collect();
        Ok(Some(shader))
    };
    Ok(PresetPackage {
        schema: 1,
        source_id: entry.id.clone().context("source id missing")?,
        source_name: entry.path.clone(),
        source_sha256: entry.sha256.clone(),
        split: entry.split.clone().context("corpus split missing")?,
        equations: crate::bundle::compile_equations(&source)?,
        warp: load("warp")?,
        comp: load("comp")?,
        adaptations: vec![
            "native-3840x2160-primary-feedback-v1".into(),
            "native-coverage-density-v1-minimum-3px".into(),
            "dense-shape-geometry-uniform-index-emission-cap-64-v2-state-evaluated-all".into(),
            "shader-total-loop-budget-64-per-fragment-v1".into(),
            "declared-auxiliary-blur-half-quarter-eighth-v1".into(),
            "seeded-procedural-noise-v1".into(),
            "stable-reflected-resource-identities-v2".into(),
            "shader-volume-mean-of-immediate-and-attenuated-bands-v1".into(),
            "source-textured-shapes-sample-native-feedback-v1".into(),
            "source-native-feedback-borders-v1".into(),
            "source-wave-mode-family-procedural-geometry-v1".into(),
            "authored-pixel-space-footprint-3-native-pixels-v1-native-canvas-unchanged".into(),
            "aggregated-64-band-spectrum-wave-bindings-v1".into(),
            "exact-origin-predicate-native-3px-seed-v1-wrapper-limited".into(),
        ],
    })
}
