use anyhow::{Context, Result, ensure};
use beat_authoring::{Prediction, SourceHints, SynthesisRequest, synthesize};
use effect_model::beat::Capabilities;
use milk_import::corpus::sha256;
use serde_json::{Value, json};
use std::{io::Cursor, path::Path};

fn resolve(root: &Path, path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.into()
    } else {
        root.join(path)
    }
}

fn checked_file(path: &Path, expected: &str, limit: u64) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "input size budget exceeded: {}",
        path.display()
    );
    ensure!(
        sha256(&bytes) == expected,
        "audited input changed: {}",
        path.display()
    );
    Ok(bytes)
}

fn author_row(root: &Path, directory: &Path, row: &Value, run: &str) -> Result<Value> {
    ensure!(
        row["errors"].as_array().is_some_and(Vec::is_empty),
        "prediction audit row has errors"
    );
    let source_sha = row["preset"]["sha256"]
        .as_str()
        .context("missing source hash")?;
    let result_sha = row["file_sha256"]
        .as_str()
        .context("missing prediction file hash")?;
    let source_path = Path::new(row["source_path"].as_str().context("missing source path")?);
    let result_path = Path::new(row["file"].as_str().context("missing result path")?);
    let source = SourceHints::from_bytes(&checked_file(source_path, source_sha, 4 * 1024 * 1024)?)?;
    let prediction =
        Prediction::from_bytes(&checked_file(result_path, result_sha, 4 * 1024 * 1024)?)?;
    ensure!(
        prediction.envelope()["run_identity"] == run,
        "prediction run changed"
    );
    ensure!(
        prediction.envelope()["preset"] == row["preset"],
        "prediction source identity changed"
    );
    ensure!(
        prediction.status == row["status"],
        "prediction status changed"
    );
    prediction.associate(&source)?;
    if prediction.status != "computed" {
        return Ok(
            json!({"source_sha256":source_sha,"prediction_status":prediction.status,
            "authoring":"pending","reason":"no computed behavioral evidence"}),
        );
    }
    ensure!(
        row["identity_checks"]
            .as_object()
            .is_some_and(|checks| !checks.is_empty() && checks.values().all(|v| *v == true)),
        "prediction identity checks did not pass"
    );
    let name = row["preset"]["name"]
        .as_str()
        .context("missing source name")?;
    let scene = synthesize(&SynthesisRequest {
        name,
        source: &source,
        prediction: Some(&prediction),
        capabilities: &Capabilities::baseline([3840, 2160]),
    })?;
    let mut archive = Cursor::new(Vec::new());
    scene.package.write(&mut archive)?;
    let bytes = archive.into_inner();
    let filename = format!("{source_sha}.beat");
    crate::write_bytes(root, &directory.join(&filename), &bytes)?;
    Ok(
        json!({"source_name":name,"source_sha256":source_sha,"prediction_status":prediction.status,
        "prediction_file_sha256":result_sha,"authoring":"generated",
        "package":filename,"package_sha256":sha256(&bytes),"components":scene.metadata.components,
        "unknown_features":scene.metadata.unknown_features,"runtime_verified":false,
        "visual_acceptance":"awaiting user review","native_4k30":"unmeasured"}),
    )
}

/// Consume only the audited snapshot; later worker results require a fresh audit.
/// Python-reference digest verification belongs to the auditor, not serde reserialization.
pub fn ready(root: &Path, audit_path: &Path, output: &Path) -> Result<Value> {
    use std::io::Read;
    let mut raw = Vec::new();
    std::fs::File::open(resolve(root, audit_path))?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)?;
    ensure!(raw.len() <= 64 * 1024 * 1024, "audit size budget exceeded");
    let audit: Value = serde_json::from_slice(&raw)?;
    ensure!(
        audit["errors"].as_array().is_some_and(Vec::is_empty),
        "prediction audit did not pass"
    );
    let run = audit["run_identity"]
        .as_str()
        .context("missing run identity")?;
    ensure!(
        run.len() == 64
            && run
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "invalid run identity"
    );
    let rows = audit["records"]
        .as_array()
        .context("missing audited records")?;
    ensure!(
        rows.len() <= 10_000
            && audit["snapshot_completed_files"].as_u64() == Some(rows.len() as u64),
        "invalid audit snapshot size"
    );
    let directory = resolve(root, output);
    // Admission writes through the frozen-corpus guard before producing any packages.
    crate::write_bytes(root, &directory.join("audit-receipt.json"), &raw)?;
    let outcomes: Vec<_> = rows
        .iter()
        .map(|row| match author_row(root, &directory, row, run) {
            Ok(value) => value,
            Err(error) => {
                json!({"source_name":row["preset"]["name"],"source_sha256":row["preset"]["sha256"],
            "prediction_status":row["status"],"authoring":"failed","error":format!("{error:#}")})
            }
        })
        .collect();
    let generated = outcomes
        .iter()
        .filter(|r| r["authoring"] == "generated")
        .count();
    let failed = outcomes
        .iter()
        .filter(|r| r["authoring"] == "failed")
        .count();
    let report = json!({"schema_version":1,"producer":"beatamp-adapt-ready-v1","run_identity":run,
        "audit_file_sha256":sha256(&raw),"snapshot_denominator":rows.len(),"generated":generated,"failed":failed,
        "pending":rows.len()-generated-failed,"frozen100_overlap_completed":audit["frozen100_overlap_completed"],
        "scope":"separate prediction pilot; not frozen100 coverage",
        "render_domain":[3840,2160],"runtime_verified":false,"native_4k30":"unmeasured",
        "limitations":["Native scene reinterpretation with bounded shared components; not an exact source translation",
            "47 source-field statistics guide artistic parameters; scene topology and audio mappings are separately authored",
            "Package generation does not establish visual quality, runtime performance or texture integration"],"outcomes":outcomes});
    crate::write_report(
        root,
        &directory.join("adaptation-report.json"),
        &serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

pub fn frozen(root: &Path, audit_path: Option<&Path>, output: &Path) -> Result<Value> {
    let corpus = milk_import::corpus::verify(root)?;
    let directory = resolve(root, output);
    let mut predictions = std::collections::BTreeMap::new();
    if let Some(path) = audit_path {
        use std::io::Read;
        let mut raw = Vec::new();
        std::fs::File::open(resolve(root, path))?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
        ensure!(raw.len() <= 64 * 1024 * 1024, "audit size budget exceeded");
        let audit: Value = serde_json::from_slice(&raw)?;
        ensure!(
            audit["errors"].as_array().is_some_and(Vec::is_empty),
            "prediction audit did not pass"
        );
        let rows = audit["records"]
            .as_array()
            .context("missing audited records")?;
        ensure!(
            rows.len() <= 10_000
                && audit["snapshot_completed_files"].as_u64() == Some(rows.len() as u64),
            "invalid audit snapshot size"
        );
        for row in rows {
            let sha = row["preset"]["sha256"]
                .as_str()
                .context("missing audited source hash")?;
            if corpus.presets.iter().any(|entry| entry.sha256 == sha) {
                ensure!(
                    row["errors"].as_array().is_some_and(Vec::is_empty),
                    "prediction audit row has errors"
                );
                if row["status"] == "computed" {
                    ensure!(
                        row["identity_checks"].as_object().is_some_and(
                            |checks| !checks.is_empty() && checks.values().all(|v| *v == true)
                        ),
                        "prediction identity checks did not pass"
                    );
                }
                let bytes = checked_file(
                    Path::new(row["file"].as_str().context("missing result path")?),
                    row["file_sha256"].as_str().context("missing result hash")?,
                    4 * 1024 * 1024,
                )?;
                let prediction = Prediction::from_bytes(&bytes)?;
                ensure!(
                    prediction.envelope()["run_identity"] == audit["run_identity"]
                        && prediction.envelope()["preset"] == row["preset"]
                        && prediction.status == row["status"],
                    "audited prediction identity changed"
                );
                ensure!(
                    predictions.insert(sha.to_owned(), prediction).is_none(),
                    "duplicate audited source"
                );
            }
        }
        crate::write_bytes(root, &directory.join("audit-receipt.json"), &raw)?;
    }
    let outcomes: Vec<_> = corpus.presets.iter().map(|entry| {
        let rank = entry.selection_rank.unwrap();
        let result = (|| -> Result<Value> {
            let source = SourceHints::from_bytes(&milk_import::corpus::verify_entry(root,entry,"presets")?)?;
            let prediction = predictions.get(&entry.sha256);
            let mut scene = synthesize(&SynthesisRequest{name:&entry.path,source:&source,prediction,capabilities:&Capabilities::baseline([3840,2160])})?;
            scene.package.manifest.notices.push(corpus.attribution.clone());
            let mut archive = Cursor::new(Vec::new());scene.package.write(&mut archive)?;
            let bytes = archive.into_inner();let name = format!("{rank:03}.beat");
            crate::write_bytes(root,&directory.join(&name),&bytes)?;
            Ok(json!({"rank":rank,"name":entry.path,"split":entry.split,"source_sha256":entry.sha256,
                "path":name,"package_sha256":sha256(&bytes),"authoring":"generated",
                "prediction_status":scene.metadata.prediction_status,"components":scene.metadata.components,
                "runtime_verified":false,"visual_acceptance":"awaiting user review","native_4k30":"unmeasured"}))
        })();
        match result {Ok(v)=>v,Err(error)=>json!({"rank":rank,"name":entry.path,"split":entry.split,"source_sha256":entry.sha256,"authoring":"failed","error":format!("{error:#}")})}
    }).collect();
    let generated = outcomes
        .iter()
        .filter(|r| r["authoring"] == "generated")
        .count();
    let report = json!({"schema_version":1,"producer":"beatamp-frozen-native-pilot-v1","count":100,
        "corpus_manifest_sha256":milk_import::corpus::MANIFEST_SHA256,"generated":generated,"failed":100-generated,
        "matched_predictions":predictions.len(),"predictions_pending":100-predictions.len(),
        "scope":"fixed100 initial artistic native-scene pilot; source translation is a separate diagnostic route",
        "attribution":corpus.attribution,"presets":outcomes,"runtime_verified":false,
        "limitations":["Unpredicted sources remain marked pending; no computed behavior is invented",
            "Bounded shared scene components; generated does not mean accurate source conversion or visual acceptance",
            "Premium texture integration and emulated native4K playback deadlines remain unverified"]});
    crate::write_report(
        root,
        &directory.join("catalog.json"),
        &serde_json::to_string_pretty(&report)?,
    )?;
    Ok(report)
}

pub fn render(
    root: &Path,
    input: &Path,
    pcm: Option<&Path>,
    rate: u32,
    channels: u16,
    frames: u32,
    output: &Path,
) -> Result<()> {
    use audio_analysis::Analyzer;
    use beat_authoring::Controller;
    use effect_package::{Limits, Package};
    use effect_render::{
        beat::{BeatAudioDescriptor, BeatInput, BeatRenderer},
        gpu::Gpu,
    };
    use std::io::Read;
    ensure!(
        (1..=18_000).contains(&frames),
        "preview frame budget exceeded"
    );
    ensure!(
        output.extension().is_some_and(|s| s == "png"),
        "preview output must be PNG"
    );
    let mut analyzer = Analyzer::new(rate, channels, 1)?;
    let package = Package::read(
        std::fs::File::open(resolve(root, input))?,
        &Limits::default(),
    )?;
    let name = package.manifest.name.clone();
    let mut renderer =
        BeatRenderer::new(pollster::block_on(Gpu::headless())?, package, [3840, 2160])?;
    ensure!(
        renderer.output_format() == wgpu_types::TextureFormat::Rgba8UnormSrgb,
        "preview PNG requires an explicitly sRGB RGBA8 output"
    );
    let mut controller = Controller::new([3840, 2160])?;
    let mut stream = pcm
        .map(|p| std::fs::File::open(resolve(root, p)))
        .transpose()?;
    let mut snapshot = None;
    let mut start_frame = 0u64;
    for frame in 0..frames {
        let end_frame = (u64::from(frame) + 1) * u64::from(rate) / 30;
        let now_ns = end_frame * 1_000_000_000 / u64::from(rate);
        let mut bytes = vec![0; ((end_frame - start_frame) * u64::from(channels) * 2) as usize];
        if let Some(file) = &mut stream {
            file.read_exact(&mut bytes)
                .context("PCM input is shorter than requested preview; no padding applied")?;
        }
        let samples: Vec<_> = bytes
            .chunks_exact(2)
            .map(|v| i16::from_le_bytes([v[0], v[1]]))
            .collect();
        if let Some(latest) = analyzer.push_i16(&samples, start_frame, now_ns, false)? {
            snapshot = Some(latest);
        }
        start_frame = end_frame;
        if let Some(s) = &snapshot {
            let control = controller.update(s, now_ns)?;
            renderer.update(&BeatInput {
                parameters: control.parameters,
                buffers: control.buffers,
                audio: Some(BeatAudioDescriptor {
                    generation: s.generation,
                    end_frame: s.end_frame,
                    capture_end_ns: s.capture_end_ns,
                    sample_rate: s.sample_rate,
                    channels: s.channels,
                    timestamp_estimated: false,
                    encoding: Some("signed-16-bit-LE".into()),
                }),
            })?;
        }
        renderer.render()?;
    }
    let rgba = renderer.capture()?;
    let image = image::RgbaImage::from_raw(3840, 2160, rgba)
        .context("native preview byte count mismatch")?;
    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image).write_to(&mut png, image::ImageFormat::Png)?;
    let path = resolve(root, output);
    crate::write_bytes(root, &path, png.get_ref())?;
    let allocations = renderer.allocation_report();
    let report = json!({"producer":"beatamp-native-preview-v1","package":input,"preset":name,
        "frames":frames,"render_domain":[3840,2160],"native_allocations":allocations.dimensions,
        "logical_allocation_bytes":allocations.bytes,"output_format":format!("{:?}",renderer.output_format()),
        "adapter":renderer.gpu.adapter.get_info().name,"backend":format!("{:?}",renderer.gpu.adapter.get_info().backend),
        "input":if pcm.is_some(){"offline interleaved stereo/mono PCM16LE; unnormalized"}else{"explicit digital silence"},
        "sample_rate":rate,"channels":channels,"source_end_frame":start_frame,
        "fast_bins_per_channel":snapshot.as_ref().and_then(|s|s.fast_spectrum.as_ref()).map(|s|s.left_power.len()),
        "detail_bins_per_channel":snapshot.as_ref().and_then(|s|s.detail_spectrum.as_ref()).map(|s|s.left_power.len()),
        "nyquist_hz":rate as f64 / 2.0,"native_4k30":"unmeasured",
        "timing_scope":"offline renderer and final-frame readback; no presentation/deadline benchmark",
        "visual_acceptance":"awaiting user review"});
    crate::write_report(
        root,
        &path.with_extension("json"),
        &serde_json::to_string_pretty(&report)?,
    )?;
    println!(
        "Rendered standalone .beat native4K preview: {}",
        path.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_audit_never_produces_assets() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        std::fs::write(
            root.path().join("audit.json"),
            br#"{"errors":["changed source"]}"#,
        )
        .unwrap();
        assert!(
            ready(
                root.path(),
                Path::new("audit.json"),
                Path::new("effects/pilot")
            )
            .is_err()
        );
        assert!(!root.path().join("effects").exists());
    }

    #[test]
    fn changed_prediction_cannot_use_an_old_audit_receipt() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        let source = root.path().join("source.milk");
        let result = root.path().join("prediction.json");
        let milk = b"[preset00]\nfDecay=0.96\n";
        std::fs::write(&source, milk).unwrap();
        std::fs::write(&result, b"modified").unwrap();
        let audit = json!({"errors":[],"run_identity":"a".repeat(64),"snapshot_completed_files":1,
            "records":[{"errors":[],"source_path":source,"file":result,"file_sha256":sha256(b"before"),
                "preset":{"sha256":sha256(milk),"name":"test.milk"},"status":"computed"}]});
        std::fs::write(
            root.path().join("audit.json"),
            serde_json::to_vec(&audit).unwrap(),
        )
        .unwrap();
        let report = ready(
            root.path(),
            Path::new("audit.json"),
            Path::new("effects/pilot"),
        )
        .unwrap();
        assert_eq!(report["snapshot_denominator"], 1);
        assert_eq!(report["generated"], 0);
        assert_eq!(report["failed"], 1);
        assert!(
            report["outcomes"][0]["error"]
                .as_str()
                .unwrap()
                .contains("audited input changed")
        );
        assert!(
            !root
                .path()
                .join("effects/pilot")
                .read_dir()
                .unwrap()
                .any(|p| p.unwrap().path().extension().is_some_and(|v| v == "beat"))
        );
    }
}
