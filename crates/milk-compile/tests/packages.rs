use std::path::Path;
#[test]
fn all_frozen_presets_have_source_bound_packages_and_bounded_shaders() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let c = milk_import::corpus::verify(root).unwrap();
    let mut shaders = 0;
    for entry in &c.presets {
        let package = milk_compile::package::from_artifacts(root, entry).unwrap();
        assert_eq!(package.source_sha256, entry.sha256);
        assert_eq!(package.source_name, entry.path);
        for shader in [package.warp, package.comp].into_iter().flatten() {
            assert_eq!(shader.loop_budget, Some(64));
            shaders += 1;
        }
    }
    assert_eq!(shaders, 170);
}
