use anyhow::{Context, Result};
use milk_import::{corpus, parser};
use std::path::Path;

pub(crate) fn report(root: &Path, artifacts: bool, smoke_frames: u32) -> Result<serde_json::Value> {
    anyhow::ensure!(
        (1..=480).contains(&smoke_frames),
        "smoke frames must be 1..480"
    );
    let corpus = corpus::verify(root)?;
    let producer_hash = corpus::sha256(&std::fs::read(std::env::current_exe()?)?);
    let mut outcomes = vec![];
    for entry in &corpus.presets {
        let bytes = corpus::verify_entry(root, entry, "presets")?;
        let parsed = parser::parse(&bytes)?;
        let mut result = serde_json::json!({"id":entry.id,"name":entry.path,"source_sha256":entry.sha256,"split":entry.split,"equations":"failed","equation_smoke":"pending","shader":"pending","conversion":"pending","render":"pending","visual_acceptance":"unverified","pcm_response":"unverified","native_4k30":"unmeasured"});
        match milk_compile::bundle::compile_equations(&parsed) {
            Err(e) => result["error"] = e.to_string().into(),
            Ok(bundle) => {
                result["equations"] = "compiled".into();
                result["sections"] = bundle.sections.len().into();
                result["instruction_count"] =
                    bundle.sections.values().map(Vec::len).sum::<usize>().into();
                result["symbols"] = bundle.symbols.len().into();
                let artifact = serde_json::json!({"artifact_kind":"equation-bundle-v1","playable":false,"pending":["shader-compilation","effect-graph"],"source_id":entry.id,"source_name":entry.path,"source_sha256":entry.sha256,"compiler":"milk-compile-equations-v1","producer_artifact_sha256":producer_hash,"equation_abi":1,"rng":"mt19937-continuous-u32max-scaled-floor-bound-v1","seed":12345,"corpus_attribution":corpus.attribution,"dependency_lock_sha256":corpus::sha256(&std::fs::read(root.join("Cargo.lock"))?),"bundle":bundle});
                let json = serde_json::to_vec(&artifact)?;
                let hash = corpus::sha256(&json);
                result["equation_artifact_sha256"] = hash.clone().into();
                if artifacts {
                    let source_id = entry.id.as_ref().context("source id missing")?;
                    let path = root
                        .join("effects")
                        .join(source_id)
                        .join(&hash)
                        .join("equations.json");
                    if path.exists() {
                        anyhow::ensure!(
                            std::fs::read(&path)? == json,
                            "equation artifact identity collision"
                        );
                    } else {
                        super::write_report(root, &path, std::str::from_utf8(&json)?)?;
                    }
                    result["equation_artifact"] = path
                        .strip_prefix(root)?
                        .to_string_lossy()
                        .into_owned()
                        .into();
                }
                let begin = std::time::Instant::now();
                match smoke(bundle, smoke_frames) {
                    Ok((instructions, memory, cleanups)) => {
                        result["equation_smoke"] = "passed".into();
                        result["max_frame_instructions"] = instructions.into();
                        result["memory_bytes"] = memory.into();
                        result["nonfinite_assignment_cleanups"] = cleanups.into();
                    }
                    Err(e) => {
                        result["equation_smoke"] = "failed".into();
                        result["runtime_error"] = e.to_string().into();
                    }
                }
                result["host_smoke_elapsed_ms"] = (begin.elapsed().as_secs_f64() * 1000.0).into();
            }
        }
        outcomes.push(result);
    }
    Ok(
        serde_json::json!({"schema_version":1,"producer":"preset-equation-compiler-smoke-v1","sample_denominator":100,"equations_compiled":outcomes.iter().filter(|r|r["equations"]=="compiled").count(),"equation_smoke_passed":outcomes.iter().filter(|r|r["equation_smoke"]=="passed").count(),"converted":0,"rendered":0,"qualified_4k30":0,"smoke_protocol":{"frames":smoke_frames,"fps":30,"seed":12345,"mesh_vertices":"49x33 (48x32 cells)","wave_samples":"authored count, capped at 512","shape_instances":"authored count, capped at 1024","audio":"synthetic static legacy bands at 1.0; waveform controls sin(8*pi*sample), cos(8*pi*sample)","reference_compared":false,"performance_meaning":"host equation execution only, not TV rendering or deadline certification"},"presets":outcomes}),
    )
}

fn smoke(
    bundle: effect_model::equations::EquationBundle,
    frames: u32,
) -> Result<(u64, usize, u64)> {
    use effect_runtime::state::{FrameInput, PresetState};
    let mut state = PresetState::new(bundle, 12345)?;
    let mut high = 0;
    for frame in 0..frames {
        state.frame(FrameInput {
            frame: frame as u64,
            time: frame as f64 / 30.0,
            progress: frame as f64 / 480.0,
            ..FrameInput::default()
        })?;
        for y in 0..=32 {
            for x in 0..=48 {
                state.pixel(x as f64 / 48.0, y as f64 / 32.0)?;
            }
        }
        for index in 0..4 {
            if state.static_value(&format!("wavecode_{index}_enabled"), 0.0) != 0.0 {
                state.begin_wave(index)?;
                let count = state.wave_frame_value(index, "samples").clamp(0.0, 512.0) as usize;
                for point in 0..count {
                    let sample = point as f64 / count.saturating_sub(1).max(1) as f64;
                    let phase = sample * std::f64::consts::TAU * 4.0;
                    state.wave_point(index, sample, phase.sin() * 0.2, phase.cos() * 0.2)?;
                }
            }
            if state.static_value(&format!("shapecode_{index}_enabled"), 0.0) != 0.0 {
                let count = state
                    .static_value(&format!("shapecode_{index}_num_inst"), 1.0)
                    .clamp(0.0, 1024.0) as usize;
                for instance in 0..count {
                    state.shape(index, instance)?;
                }
            }
        }
        high = high.max(state.instruction_count());
    }
    Ok((high, state.memory_bytes(), state.nonfinite_assignments()))
}
