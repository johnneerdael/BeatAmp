use anyhow::{Context, Result, ensure};
use effect_model::beat::{Manifest, safe_asset_path};
use effect_package::{Limits, Package};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::Path;

fn resolve(root: &Path, path: &Path) -> std::path::PathBuf {
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

pub fn export(root: &Path, manifest_path: &Path, output: &Path) -> Result<()> {
    let path = resolve(root, manifest_path);
    let limits = Limits::default();
    ensure!(
        std::fs::metadata(&path)?.len() <= limits.max_manifest_bytes,
        "authoring manifest budget exceeded"
    );
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&path)?)?;
    let directory = path
        .parent()
        .context("manifest has no parent")?
        .canonicalize()?;
    let mut assets = BTreeMap::new();
    let mut total = 0u64;
    for (id, asset) in &manifest.assets {
        ensure!(
            safe_asset_path(&asset.path),
            "invalid authoring asset path: {id}"
        );
        let mut file = directory.clone();
        for part in asset.path.split('/') {
            file.push(part);
            ensure!(
                !std::fs::symlink_metadata(&file)?.file_type().is_symlink(),
                "authoring asset symlink: {id}"
            );
        }
        ensure!(
            file.canonicalize()?.starts_with(&directory),
            "authoring asset escapes source directory"
        );
        let metadata = std::fs::metadata(&file)?;
        ensure!(
            metadata.is_file() && metadata.len() <= limits.max_entry_bytes,
            "authoring asset file/size invalid: {id}"
        );
        total = total
            .checked_add(metadata.len())
            .context("authoring asset size overflow")?;
        ensure!(
            total <= limits.max_total_bytes,
            "authoring payload budget exceeded"
        );
        assets.insert(id.clone(), std::fs::read(&file)?);
    }
    let package = Package { manifest, assets };
    let mut bytes = Cursor::new(Vec::new());
    package.write_with_limits(&mut bytes, &limits)?;
    crate::write_bytes(root, &resolve(root, output), &bytes.into_inner())?;
    println!(
        "Exported standalone .beat; {} declared assets; runtime rendering remains unverified.",
        package.assets.len()
    );
    Ok(())
}

pub fn inspect(root: &Path, input: &Path) -> Result<serde_json::Value> {
    let package = Package::read(
        std::fs::File::open(resolve(root, input))?,
        &Limits::default(),
    )?;
    let payload: usize = package.assets.values().map(Vec::len).sum();
    let assets: Vec<_> = package.manifest.assets.iter().map(|(id, asset)| serde_json::json!({
        "id":id,"path":asset.path,"kind":asset.kind,"sha256":asset.sha256,"bytes":package.assets[id].len(),"license":asset.license
    })).collect();
    Ok(serde_json::json!({
        "format":package.manifest.format,"version":package.manifest.version,
        "name":package.manifest.name,"author":package.manifest.author,"notices":package.manifest.notices,
        "quality":package.manifest.quality,"asset_count":assets.len(),"assets":assets,
        "payload_bytes":payload,"container_verified":true,"runtime_verified":false,
        "graph_validation":"performed later against the selected runtime device",
        "passes":package.manifest.passes.len(),"resources":package.manifest.resources.len(),
        "parameters":package.manifest.parameters.len()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn source(root: &Path) {
        let source = root.join("editable");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("program.wgsl"), b"hello").unwrap();
        let manifest = json!({
            "format":"beat", "version":1, "name":"Editable original", "author":"Artist",
            "quality":"native", "output":"final",
            "assets":{"field":{"path":"program.wgsl","sha256":"2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824","kind":"wgsl"}},
            "resources":{},"passes":[]
        });
        std::fs::write(
            source.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn exported_effect_inspection_never_requires_its_editable_inputs() {
        // Export that leaves external paths instead of owned bytes fails after removal.
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        source(root.path());
        export(
            root.path(),
            Path::new("editable/manifest.json"),
            Path::new("effects/original.beat"),
        )
        .unwrap();
        std::fs::remove_dir_all(root.path().join("editable")).unwrap();
        let report = inspect(root.path(), Path::new("effects/original.beat")).unwrap();
        assert_eq!(report["name"], "Editable original");
        assert_eq!(report["asset_count"], 1);
        assert_eq!(report["payload_bytes"], 5);
        assert_eq!(report["container_verified"], true);
        assert_eq!(report["runtime_verified"], false);
    }

    #[test]
    fn export_preserves_the_frozen_corpus_write_boundary() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        source(root.path());
        assert!(
            export(
                root.path(),
                Path::new("editable/manifest.json"),
                Path::new("corpus/new/output.beat")
            )
            .is_err()
        );
        assert!(!root.path().join("corpus/new").exists());
    }

    #[test]
    fn authoring_asset_symlinks_cannot_escape_the_manifest_directory() {
        // Canonical containment and symlink rejection must precede asset reads.
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("corpus")).unwrap();
        source(root.path());
        let file = root.path().join("editable/program.wgsl");
        std::fs::remove_file(&file).unwrap();
        let outside = root.path().join("outside.wgsl");
        std::fs::write(&outside, b"hello").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside, file).unwrap();
        assert!(
            export(
                root.path(),
                Path::new("editable/manifest.json"),
                Path::new("effects/output.beat")
            )
            .is_err()
        );
        assert!(!root.path().join("effects/output.beat").exists());
    }
}
