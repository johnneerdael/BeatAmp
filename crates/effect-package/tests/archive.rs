use effect_model::beat::Manifest;
use effect_package::{Limits, Package};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

const SHADER: &[u8] = b"@fragment fn fragment_main() -> @location(0) vec4<f32> { return vec4<f32>(0.0, 0.5, 1.0, 1.0); }\n";

fn fixture() -> Package {
    let manifest: Manifest = serde_json::from_value(json!({
        "format":"beat", "version":1, "name":"Independent spectrum", "author":"Fixture artist",
        "notices":["Original fixture artwork"], "quality":"native", "output":"final",
        "assets":{"shader":{"path":"programs/field.wgsl","sha256":format!("{:x}",Sha256::digest(SHADER)),"kind":"wgsl","license":"CC0-1.0"}},
        "parameters":{"material.warmth":{"initial":[0.25]}},
        "resources":{"final":{"kind":"texture","format":"rgba8unorm","extent":{"mode":"native"},"history":false,"role":"output","mips":1,"color_space":"linear","alpha":"opaque"}},
        "passes":[{"id":"display","kind":"render","program":"shader","reads":[],"writes":["final"]}]
    })).unwrap();
    Package {
        manifest,
        assets: BTreeMap::from([("shader".to_owned(), SHADER.to_vec())]),
    }
}

fn archive(entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, data) in entries {
        writer.start_file(name, options).unwrap();
        writer.write_all(&data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn entries() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "manifest.json".into(),
            serde_json::to_vec(&fixture().manifest).unwrap(),
        ),
        ("programs/field.wgsl".into(), SHADER.to_vec()),
    ]
}

#[test]
fn standalone_archive_survives_removal_of_original_inputs() {
    // Reading through an external source path instead of owned package bytes breaks this.
    let original = tempfile::tempdir().unwrap();
    let source = original.path().join("field.wgsl");
    std::fs::write(&source, SHADER).unwrap();
    let mut package = fixture();
    package
        .assets
        .insert("shader".into(), std::fs::read(&source).unwrap());
    let mut bytes = Cursor::new(Vec::new());
    package.write(&mut bytes).unwrap();
    original.close().unwrap();
    let loaded = Package::read(Cursor::new(bytes.into_inner()), &Limits::default()).unwrap();
    assert_eq!(loaded.assets["shader"], SHADER);
    assert_eq!(loaded.manifest.name, "Independent spectrum");
    assert_eq!(loaded.manifest.notices, ["Original fixture artwork"]);
}

#[test]
fn repeated_export_is_byte_identical_and_contains_only_declared_inputs() {
    // Time-dependent ZIP metadata or inclusion of source/debug files breaks portability.
    let package = fixture();
    let mut first = Cursor::new(Vec::new());
    let mut second = Cursor::new(Vec::new());
    package.write(&mut first).unwrap();
    package.write(&mut second).unwrap();
    assert_eq!(first.get_ref(), second.get_ref());
    let zip = zip::ZipArchive::new(Cursor::new(first.into_inner())).unwrap();
    assert_eq!(zip.len(), 2);
    assert!(zip.file_names().any(|n| n == "manifest.json"));
    assert!(zip.file_names().any(|n| n == "programs/field.wgsl"));
}

#[test]
fn modified_asset_cannot_load_under_an_unchanged_manifest_hash() {
    // Omitting byte-hash verification accepts a different executable program.
    let mut data = entries();
    data[1].1 = b"tampered shader".to_vec();
    let error = Package::read(Cursor::new(archive(data)), &Limits::default()).unwrap_err();
    assert!(format!("{error:#}").contains("hash"));
}

#[test]
fn missing_and_undeclared_members_are_rejected() {
    // A placeholder for missing content, or acceptance of hidden files, breaks self-containment.
    let mut missing = entries();
    missing.pop();
    assert!(Package::read(Cursor::new(archive(missing)), &Limits::default()).is_err());
    let mut extra = entries();
    extra.push(("unexpected.milk".into(), b"legacy".to_vec()));
    let error = Package::read(Cursor::new(archive(extra)), &Limits::default()).unwrap_err();
    assert!(format!("{error:#}").contains("undeclared"));
}

#[test]
fn unsafe_and_duplicate_archive_paths_cannot_be_normalized_into_valid_inputs() {
    // Sanitizing traversal or allowing last-writer-wins names can hide conflicting assets.
    for name in [
        "../outside",
        "/absolute",
        "C:/absolute",
        "programs/../field.wgsl",
    ] {
        let mut data = entries();
        data.push((name.into(), vec![1]));
        assert!(
            Package::read(Cursor::new(archive(data)), &Limits::default()).is_err(),
            "{name}"
        );
    }
    let mut data = entries();
    data.push(("programs/other.wgsl".into(), SHADER.to_vec()));
    let bytes = archive(data);
    // Equal-length rename in both headers creates a real duplicate ZIP member.
    let mut duplicate = bytes;
    let old = b"programs/other.wgsl";
    let new = b"programs/field.wgsl";
    assert_eq!(old.len(), new.len());
    for i in 0..=duplicate.len() - old.len() {
        if duplicate[i..i + old.len()] == *old {
            duplicate[i..i + old.len()].copy_from_slice(new);
        }
    }
    assert!(Package::read(Cursor::new(duplicate), &Limits::default()).is_err());
}

#[test]
fn symlink_entries_are_rejected_without_touching_the_filesystem() {
    // Treating a symlink payload as a normal shader permits path indirection.
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    writer.start_file("manifest.json", options).unwrap();
    writer
        .write_all(&serde_json::to_vec(&fixture().manifest).unwrap())
        .unwrap();
    writer
        .add_symlink("programs/field.wgsl", "../../outside", options)
        .unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let error = Package::read(Cursor::new(bytes), &Limits::default()).unwrap_err();
    assert!(format!("{error:#}").contains("symlink"));
}

#[test]
fn declared_limits_bound_entries_manifest_and_aggregate_payload() {
    // Loading all bytes before enforcing caller budgets permits oversized packages.
    let bytes = archive(entries());
    for limits in [
        Limits {
            max_entries: 1,
            ..Limits::default()
        },
        Limits {
            max_manifest_bytes: 8,
            ..Limits::default()
        },
        Limits {
            max_entry_bytes: 8,
            ..Limits::default()
        },
        Limits {
            max_total_bytes: 32,
            ..Limits::default()
        },
        Limits {
            max_archive_bytes: 32,
            ..Limits::default()
        },
    ] {
        assert!(Package::read(Cursor::new(&bytes), &limits).is_err());
    }
}

#[test]
fn unsupported_versions_and_compression_do_not_silently_fallback() {
    let mut data = entries();
    let mut manifest: serde_json::Value = serde_json::from_slice(&data[0].1).unwrap();
    manifest["version"] = json!(2);
    data[0].1 = serde_json::to_vec(&manifest).unwrap();
    let error = Package::read(Cursor::new(archive(data)), &Limits::default()).unwrap_err();
    assert!(format!("{error:#}").contains("version"));
    let mut bytes = archive(entries());
    // Compression identifiers occur at +8 in local and +10 in central headers.
    for i in 0..bytes.len() - 12 {
        if &bytes[i..i + 4] == b"PK\x03\x04" {
            bytes[i + 8..i + 10].copy_from_slice(&8u16.to_le_bytes());
        }
        if &bytes[i..i + 4] == b"PK\x01\x02" {
            bytes[i + 10..i + 12].copy_from_slice(&8u16.to_le_bytes());
        }
    }
    let error = Package::read(Cursor::new(bytes), &Limits::default()).unwrap_err();
    assert!(format!("{error:#}").contains("compression"));
}

#[test]
fn invalid_writer_inputs_fail_before_emitting_any_archive_bytes() {
    // Writing an incomplete archive before detecting absent/incorrect bytes breaks atomic export.
    let mut package = fixture();
    package.assets.clear();
    let mut output = Cursor::new(Vec::new());
    assert!(package.write(&mut output).is_err());
    assert!(output.get_ref().is_empty());
    package.assets.insert("shader".into(), b"wrong".to_vec());
    assert!(package.write(&mut output).is_err());
    assert!(output.get_ref().is_empty());
}

#[test]
fn nonempty_writer_cannot_leave_an_old_footer_after_a_new_export() {
    // Generic Write+Seek cannot truncate an existing file; accepting it can
    // leave a previous trailer behind and produce an unreadable package.
    let original = vec![0x55; 4096];
    let mut output = Cursor::new(original.clone());
    assert!(fixture().write(&mut output).is_err());
    assert_eq!(output.into_inner(), original);
}
