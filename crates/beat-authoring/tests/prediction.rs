#![cfg(feature = "offline")]
use beat_authoring::{Prediction, SourceHints, SynthesisRequest, synthesize};
use effect_model::beat::Capabilities;
use serde_json::{Value, json};

fn fixture(source: &SourceHints) -> Value {
    let mut features = serde_json::Map::new();
    for key in include_str!("../src/feature-keys.txt").lines() {
        features.insert(key.into(), json!({"value":null,"status":"unknown","unit":"test_unit","evidence_kind":"source_field","interval":null,"interval_kind":null,"support":{},"dependencies":[],"unknown_reasons":["custom geometry unobservable"]}));
    }
    features["colour.mean_saturation"] = json!({"value":0.1234567890123456,"status":"computed","unit":"fraction","evidence_kind":"source_field","interval":null,"interval_kind":null,"support":{"scope":"conditional"},"dependencies":[],"unknown_reasons":[]});
    json!({"schema_version":1,"export_kind":"preset-corpus-result","preset":{"name":"test.milk","relative_path":"test.milk","sha256":source.sha256},"status":"computed","feature_record":{"schema_version":1,"feature_basis":"source-field-simulation","context":{"input_hashes":{"preset_sha256":source.sha256},"provenance":{"producer":"synthetic fixture"},"domain":{"width":854,"height":480,"frames":60,"fps":15},"feature_extractor_sha256":"a".repeat(64)},"context_sha256":"b".repeat(64),"record_sha256":"c".repeat(64),"features":features,"uses_rendered_reference":false,"appearance_accuracy_verified":false,"limitations":["not executable geometry"]}})
}

#[test]
fn admission_preserves_unknowns_f64_opaque_context_and_original_bytes() {
    let source = SourceHints::from_bytes(b"[preset00]\nfDecay=0.98\n").unwrap();
    let data = serde_json::to_vec_pretty(&fixture(&source)).unwrap();
    let prediction = Prediction::from_bytes(&data).unwrap();
    assert_eq!(prediction.original_bytes(), data);
    assert_eq!(
        prediction.scalar("colour.mean_saturation"),
        Some(0.1234567890123456)
    );
    assert_eq!(prediction.scalar("geometry.speed_p95"), None);
    assert!(
        prediction.envelope()["feature_record"]["features"]["geometry.speed_p95"]["value"]
            .is_null()
    );
    let caps = Capabilities::baseline([3840, 2160]);
    let result = synthesize(&SynthesisRequest {
        name: "unknown",
        source: &source,
        prediction: Some(&prediction),
        capabilities: &caps,
    })
    .unwrap();
    assert_eq!(
        result.metadata.prediction_context.unwrap()["domain"]["fps"],
        15
    );
    assert_eq!(result.metadata.source_sha256, source.sha256);
    assert!(
        result
            .metadata
            .unknown_features
            .contains(&"geometry.speed_p95".into())
    );
    assert!(
        result
            .metadata
            .limitations
            .iter()
            .any(|v| v.contains("reference hashes"))
    );
    assert_eq!(result.package.assets["prediction_evidence"], data);
}

#[test]
fn source_mismatch_and_unknown_zero_substitution_are_rejected() {
    let source = SourceHints::from_bytes(b"[preset00]\nfDecay=0.98\n").unwrap();
    let other = SourceHints::from_bytes(b"[preset00]\nfDecay=0.97\n").unwrap();
    let prediction =
        Prediction::from_bytes(&serde_json::to_vec(&fixture(&source)).unwrap()).unwrap();
    let caps = Capabilities::baseline([3840, 2160]);
    assert!(
        synthesize(&SynthesisRequest {
            name: "mismatch",
            source: &other,
            prediction: Some(&prediction),
            capabilities: &caps
        })
        .is_err()
    );
    let mut invalid = fixture(&source);
    invalid["feature_record"]["features"]["geometry.speed_p95"]["value"] = json!(0.0);
    assert!(Prediction::from_bytes(&serde_json::to_vec(&invalid).unwrap()).is_err());
}

#[test]
fn terminal_outcomes_produce_explicit_source_guided_adaptation() {
    let source = SourceHints::from_bytes(b"[preset00]\nfDecay=0.98\n").unwrap();
    let caps = Capabilities::baseline([3840, 2160]);
    for status in ["unsupported", "error", "timeout"] {
        let mut value = fixture(&source);
        value["status"] = json!(status);
        value["feature_record"] = Value::Null;
        value["error"] = json!("original predictor diagnostic");
        let prediction = Prediction::from_bytes(&serde_json::to_vec(&value).unwrap()).unwrap();
        let result = synthesize(&SynthesisRequest {
            name: "failure retained",
            source: &source,
            prediction: Some(&prediction),
            capabilities: &caps,
        })
        .unwrap();
        assert_eq!(result.metadata.prediction_status, status);
        assert!(result.metadata.evidence_targets.is_empty());
        assert!(
            result
                .metadata
                .choices
                .iter()
                .any(|v| v.contains("source-guided original adaptation"))
        );
        assert_eq!(
            prediction.envelope()["error"],
            "original predictor diagnostic"
        );
    }
}

#[test]
fn low_source_saturation_retains_a_deliberate_artistic_colour_contrast_floor() {
    let source = SourceHints::from_bytes(b"[preset00]\nfDecay=0.98\n").unwrap();
    let mut value = fixture(&source);
    value["feature_record"]["features"]["colour.mean_saturation"]["value"] = json!(0.0);
    let prediction = Prediction::from_bytes(&serde_json::to_vec(&value).unwrap()).unwrap();
    let result = synthesize(&SynthesisRequest {
        name: "low saturation original",
        source: &source,
        prediction: Some(&prediction),
        capabilities: &Capabilities::baseline([3840, 2160]),
    })
    .unwrap();
    let colour = &result.package.manifest.parameters["palette_b"].initial[..3];
    let maximum = colour.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let minimum = colour.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(
        maximum - minimum > 0.25,
        "premium palette must retain deliberate colour separation"
    );
    assert_eq!(
        result
            .metadata
            .evidence_targets
            .iter()
            .find(|v| v.feature == "colour.mean_saturation")
            .unwrap()
            .original_f64,
        0.0
    );
    assert!(
        result
            .metadata
            .choices
            .iter()
            .any(|v| v.contains("saturation floor"))
    );
}
