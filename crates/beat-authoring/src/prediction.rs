use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::Read};

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn hash(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

/// Offline parse hints, explicitly insufficient to reconstruct arbitrary geometry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceHints {
    pub sha256: String,
    pub encoding: String,
    pub numeric: BTreeMap<String, f64>,
    pub code_sections: BTreeMap<String, usize>,
    pub diagnostics: Vec<String>,
    pub author: Option<String>,
}
impl SourceHints {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let source = milk_import::parser::parse(bytes)?;
        Ok(Self {
            sha256: digest(bytes),
            encoding: source.encoding,
            numeric: source.numeric,
            code_sections: source
                .sections
                .into_iter()
                .map(|(k, v)| (k, v.len()))
                .collect(),
            diagnostics: source
                .diagnostics
                .into_iter()
                .map(|d| format!("line {}: {}", d.source_line, d.message))
                .collect(),
            author: source.text.get("author").cloned(),
        })
    }
}

/// A checked envelope with exact original bytes, not a reserialized hash claim.
#[derive(Clone, Debug)]
pub struct Prediction {
    envelope: Value,
    original: Vec<u8>,
    pub file_sha256: String,
    pub status: String,
    pub source_sha256: String,
}
impl Prediction {
    pub fn read(reader: impl Read) -> Result<Self> {
        let mut bytes = Vec::new();
        reader.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        Self::from_bytes(&bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() <= 4 * 1024 * 1024,
            "prediction exceeds admission budget"
        );
        let envelope: Value = serde_json::from_slice(bytes).context("invalid prediction JSON")?;
        ensure!(
            envelope["schema_version"] == 1 && envelope["export_kind"] == "preset-corpus-result",
            "unsupported prediction envelope"
        );
        let status = envelope["status"]
            .as_str()
            .context("missing prediction status")?
            .to_owned();
        ensure!(
            ["computed", "unsupported", "error", "timeout"].contains(&status.as_str()),
            "unsupported prediction status"
        );
        ensure!(
            hash(&envelope["preset"]["sha256"]),
            "invalid prediction source hash"
        );
        let source_sha256 = envelope["preset"]["sha256"].as_str().unwrap().to_owned();
        ensure!(
            envelope["preset"]["name"].is_string()
                && envelope["preset"]["relative_path"].is_string(),
            "missing prediction source identity"
        );
        if status == "computed" {
            let record = &envelope["feature_record"];
            ensure!(record["schema_version"] == 1, "unsupported feature schema");
            ensure!(
                ["source-field-simulation", "strict-source-no-display-frames"]
                    .contains(&record["feature_basis"].as_str().unwrap_or("")),
                "unsupported feature basis"
            );
            ensure!(
                record["uses_rendered_reference"] == false
                    && record["appearance_accuracy_verified"] == false,
                "unsupported fidelity claim in source features"
            );
            ensure!(
                hash(&record["context_sha256"]) && hash(&record["record_sha256"]),
                "invalid reference digest descriptions"
            );
            let context = &record["context"];
            ensure!(
                context["input_hashes"]["preset_sha256"] == source_sha256,
                "record/envelope source association mismatch"
            );
            ensure!(
                context["provenance"].is_object()
                    && context["domain"].is_object()
                    && hash(&context["feature_extractor_sha256"]),
                "invalid opaque feature context"
            );
            let features = record["features"]
                .as_object()
                .context("missing feature map")?;
            let expected: Vec<_> = include_str!("feature-keys.txt").lines().collect();
            ensure!(
                features.len() == expected.len()
                    && expected.iter().all(|k| features.contains_key(*k)),
                "unsupported source feature vocabulary"
            );
            for (key, feature) in features {
                ensure!(
                    feature["unit"].is_string()
                        && feature["evidence_kind"].is_string()
                        && feature["support"].is_object(),
                    "invalid feature metadata: {key}"
                );
                ensure!(
                    feature["dependencies"]
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
                        && feature["unknown_reasons"]
                            .as_array()
                            .is_some_and(|a| a.iter().all(Value::is_string)),
                    "invalid feature reasons/dependencies: {key}"
                );
                match feature["status"].as_str() {
                    Some("unknown") => ensure!(
                        feature["value"].is_null()
                            && !feature["unknown_reasons"].as_array().unwrap().is_empty(),
                        "unknown must remain null with reasons: {key}"
                    ),
                    Some("computed") => ensure!(
                        feature["value"].is_number() || feature["value"].is_array(),
                        "computed feature requires value: {key}"
                    ),
                    _ => anyhow::bail!("invalid feature status: {key}"),
                }
                if let Some(value) = feature["value"].as_f64() {
                    ensure!(value.is_finite(), "nonfinite feature: {key}");
                }
            }
        } else {
            ensure!(
                envelope["feature_record"].is_null(),
                "terminal prediction must not masquerade as computed evidence"
            );
        }
        Ok(Self {
            envelope,
            original: bytes.to_vec(),
            file_sha256: digest(bytes),
            status,
            source_sha256,
        })
    }
    pub fn envelope(&self) -> &Value {
        &self.envelope
    }
    pub fn original_bytes(&self) -> &[u8] {
        &self.original
    }
    pub fn scalar(&self, key: &str) -> Option<f64> {
        let feature = &self.envelope["feature_record"]["features"][key];
        (self.status == "computed" && feature["status"] == "computed")
            .then(|| feature["value"].as_f64())
            .flatten()
    }
    pub fn associate(&self, source: &SourceHints) -> Result<()> {
        ensure!(
            self.source_sha256 == source.sha256,
            "prediction/source SHA256 mismatch"
        );
        Ok(())
    }
}
