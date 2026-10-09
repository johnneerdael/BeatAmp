use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceLine {
    pub number: u32,
    pub source_line: u32,
    pub text: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub source_line: u32,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Preset {
    pub encoding: String,
    pub numeric: BTreeMap<String, f64>,
    pub text: BTreeMap<String, String>,
    pub sections: BTreeMap<String, Vec<SourceLine>>,
    pub diagnostics: Vec<Diagnostic>,
}
pub fn parse(bytes: &[u8]) -> Result<Preset> {
    ensure!(
        bytes.len() <= 512 * 1024,
        "preset exceeds 512 KiB source limit"
    );
    let (decoded, encoding) = match std::str::from_utf8(bytes) {
        Ok(s) => (std::borrow::Cow::Borrowed(s), "utf-8"),
        Err(_) => {
            let (s, _, had_errors) = encoding_rs::WINDOWS_1252.decode(bytes);
            ensure!(!had_errors, "undecodable Windows-1252 source");
            (s, "windows-1252")
        }
    };
    let mut preset = Preset {
        encoding: encoding.to_owned(),
        ..Preset::default()
    };
    let mut found_header = false;
    let mut sections: BTreeMap<String, BTreeMap<u32, SourceLine>> = BTreeMap::new();
    for (index, line) in decoded.lines().enumerate() {
        let source_line = index as u32 + 1;
        let line = line.trim_start_matches('\u{feff}').trim();
        if line == "[preset00]" {
            found_header = true;
            continue;
        }
        if line.is_empty() || line.starts_with("//") || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            preset.diagnostics.push(Diagnostic {
                source_line,
                message: "unrecognized source line".into(),
            });
            continue;
        };
        let key = key.trim();
        let value = value.trim_end();
        if let Some((scope, number)) = section_key(key) {
            let text = if scope == "warp" || scope == "comp" {
                value.strip_prefix('`').unwrap_or(value)
            } else {
                value
            };
            let previous = sections.entry(scope).or_default().insert(
                number,
                SourceLine {
                    number,
                    source_line,
                    text: text.to_owned(),
                },
            );
            if previous.is_some() {
                preset.diagnostics.push(Diagnostic {
                    source_line,
                    message: format!("duplicate code key {key}; last value retained"),
                });
            }
        } else if let Ok(number) = value.trim().parse::<f64>() {
            ensure!(
                number.is_finite(),
                "nonfinite static value at line {source_line}"
            );
            if preset.numeric.insert(key.to_owned(), number).is_some() {
                preset.diagnostics.push(Diagnostic {
                    source_line,
                    message: format!("duplicate key {key}; last value retained"),
                });
            }
        } else {
            if preset
                .text
                .insert(key.to_owned(), value.to_owned())
                .is_some()
            {
                preset.diagnostics.push(Diagnostic {
                    source_line,
                    message: format!("duplicate text key {key}; last value retained"),
                });
            }
        }
    }
    ensure!(found_header, "missing [preset00] section");
    preset.sections = sections
        .into_iter()
        .map(|(k, v)| (k, v.into_values().collect()))
        .collect();
    Ok(preset)
}

fn section_key(key: &str) -> Option<(String, u32)> {
    for prefix in [
        "per_frame_init_",
        "per_frame_",
        "per_pixel_",
        "warp_",
        "comp_",
    ] {
        if let Some(suffix) = key.strip_prefix(prefix)
            && let Ok(number) = suffix.parse::<u32>()
        {
            return Some((prefix.trim_end_matches('_').into(), number));
        }
    }
    for kind in ["wave", "shape"] {
        let Some(rest) = key.strip_prefix(&format!("{kind}_")) else {
            continue;
        };
        let Some((index, phase)) = rest.split_once('_') else {
            continue;
        };
        if index.parse::<u32>().is_err() {
            continue;
        }
        for prefix in ["init", "per_frame", "per_point"] {
            if let Some(suffix) = phase.strip_prefix(prefix)
                && let Ok(number) = suffix.parse::<u32>()
            {
                return Some((format!("{kind}_{index}_{prefix}"), number));
            }
        }
    }
    None
}
