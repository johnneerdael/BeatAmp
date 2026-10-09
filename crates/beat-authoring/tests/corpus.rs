#![cfg(feature = "offline")]
use beat_authoring::{SourceHints, SynthesisRequest, synthesize};
use effect_model::beat::{AssetKind, Capabilities};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};

#[test]
fn frozen_hundred_all_generate_without_replacing_failures_or_tuning_holdout() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("corpus/manifest.json")).unwrap()).unwrap();
    let rows = manifest["presets"].as_array().unwrap();
    assert_eq!(rows.len(), 100);
    let caps = Capabilities::baseline([3840, 2160]);
    let mut compositions = BTreeSet::new();
    let mut shader_hashes = BTreeSet::new();
    for row in rows {
        let name = row["path"].as_str().unwrap();
        let source = SourceHints::from_bytes(
            &std::fs::read(root.join(row["copied_path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        assert_eq!(source.sha256, row["sha256"].as_str().unwrap());
        let result = synthesize(&SynthesisRequest {
            name,
            source: &source,
            prediction: None,
            capabilities: &caps,
        })
        .unwrap();
        compositions.insert(result.metadata.components);
        shader_hashes.insert(
            result.package.manifest.assets["scene_program"]
                .sha256
                .clone(),
        );
        for (id, asset) in &result.package.manifest.assets {
            if asset.kind == AssetKind::Wgsl {
                let module = naga::front::wgsl::parse_str(
                    std::str::from_utf8(&result.package.assets[id]).unwrap(),
                )
                .unwrap();
                naga::valid::Validator::new(
                    naga::valid::ValidationFlags::all(),
                    naga::valid::Capabilities::empty(),
                )
                .validate(&module)
                .unwrap();
            }
        }
    }
    assert!(compositions.len() >= 15);
    assert_eq!(shader_hashes.len(), 100);
}
