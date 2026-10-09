use milk_import::{
    corpus::{verify, verify_entry},
    parser::parse,
};
use std::path::Path;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn all_100_frozen_presets_and_74_textures_are_hash_verified_and_inspectable() {
    let corpus = verify(root()).unwrap();
    assert_eq!(corpus.presets.len(), 100);
    assert_eq!(corpus.textures.len(), 74);
    for entry in &corpus.presets {
        let bytes = verify_entry(root(), entry, "presets").unwrap();
        let p = parse(&bytes).unwrap();
        assert!(p.numeric.contains_key("fDecay"), "{}", entry.path);
    }
}

#[test]
fn changed_bytes_and_path_escape_cannot_pass_verification() {
    let mut entry = verify(root()).unwrap().presets.remove(0);
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(temp.path().join("corpus/presets")).unwrap();
    let bytes = std::fs::read(root().join(&entry.copied_path)).unwrap();
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    std::fs::write(temp.path().join(&entry.copied_path), changed).unwrap();
    assert!(verify_entry(temp.path(), &entry, "presets").is_err());
    entry.path = "../../outside.milk".into();
    entry.copied_path = "outside.milk".into();
    assert!(verify_entry(root(), &entry, "presets").is_err());
}

#[test]
fn manifest_identity_is_frozen_including_source_ids_and_attribution() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("corpus")).unwrap();
    let original = std::fs::read_to_string(root().join("corpus/manifest.json")).unwrap();
    std::fs::write(
        temp.path().join("corpus/manifest.json"),
        original.replace("Cream of the Crop", "Different attribution"),
    )
    .unwrap();
    let error = verify(temp.path()).unwrap_err().to_string();
    assert!(error.contains("manifest identity"), "{error}");
}
