use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

pub const POPULATION_SHA256: &str =
    "0ee287748d2bbd0c8ec834c1c6963170480da8af01134b2189b5a09430387bb2";
pub const MANIFEST_SCHEMA: u32 = 1;
pub const MANIFEST_SHA256: &str =
    "512324ed1ba4323b0e2fbaebef7cc75155b969c2d2b7a46e8f6d2b8b88613477";
pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub copied_path: String,
    pub bytes: u64,
    pub sha256: String,
    pub id: Option<String>,
    pub selection_rank: Option<u32>,
    pub selection_hash: Option<String>,
    pub split: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corpus {
    pub schema_version: u32,
    pub sample_count: usize,
    pub source_population_count: usize,
    pub population_manifest: String,
    pub population_manifest_sha256: String,
    pub presets: Vec<Entry>,
    pub textures: Vec<Entry>,
    pub attribution: String,
}
pub fn verify(root: &Path) -> Result<Corpus> {
    let manifest_bytes = std::fs::read(root.join("corpus/manifest.json"))?;
    ensure!(
        manifest_bytes.len() <= 1024 * 1024,
        "oversized corpus manifest"
    );
    ensure!(
        sha256(&manifest_bytes) == MANIFEST_SHA256,
        "frozen manifest identity changed"
    );
    let corpus: Corpus = serde_json::from_slice(&manifest_bytes)?;
    ensure!(
        corpus.schema_version == MANIFEST_SCHEMA,
        "unknown corpus schema"
    );
    ensure!(
        corpus.sample_count == 100 && corpus.presets.len() == 100,
        "frozen corpus must contain exactly 100 presets"
    );
    ensure!(
        corpus.textures.len() == 74,
        "frozen corpus must contain exactly 74 textures"
    );
    ensure!(
        corpus.source_population_count == 9606,
        "population count changed"
    );
    ensure!(
        corpus.population_manifest == "corpus/population.jsonl"
            && corpus.population_manifest_sha256 == POPULATION_SHA256,
        "frozen population identity changed"
    );
    let population = std::fs::read(root.join(&corpus.population_manifest))?;
    ensure!(
        sha256(&population) == POPULATION_SHA256,
        "population manifest hash mismatch"
    );
    let mut ranked = vec![];
    for row in std::str::from_utf8(&population)?.lines() {
        let value: serde_json::Value = serde_json::from_str(row)?;
        let path = value["path"]
            .as_str()
            .context("population row missing path")?
            .to_owned();
        let hash = value["sha256"]
            .as_str()
            .context("population row missing hash")?
            .to_owned();
        let bytes = value["bytes"]
            .as_u64()
            .context("population row missing byte count")?;
        let mut input = b"preset-convertor/sample-v1\0".to_vec();
        input.extend_from_slice(b"42f428dc26a55bdd24b5d74c0db1c0f4\0");
        input.extend_from_slice(path.as_bytes());
        ranked.push((sha256(&input), path, hash, bytes));
    }
    ensure!(ranked.len() == 9606, "population inventory length changed");
    ranked.sort();
    for (index, entry) in corpus.presets.iter().enumerate() {
        let (rank_hash, path, content_hash, bytes) = &ranked[index];
        ensure!(
            &entry.path == path && &entry.sha256 == content_hash && &entry.bytes == bytes,
            "sampling identity mismatch at rank {}",
            index + 1
        );
        ensure!(
            entry.selection_hash.as_ref() == Some(rank_hash)
                && entry.selection_rank == Some(index as u32 + 1),
            "sampling rank mismatch"
        );
        let split = if index < 80 { "development" } else { "holdout" };
        ensure!(
            entry.split.as_deref() == Some(split),
            "frozen development/holdout split changed"
        );
    }
    for (directory, entries) in [("presets", &corpus.presets), ("textures", &corpus.textures)] {
        let mut expected = BTreeSet::new();
        for entry in entries {
            ensure!(
                expected.insert(entry.path.clone()),
                "duplicate corpus entry: {}",
                entry.path
            );
            verify_entry(root, entry, directory).with_context(|| entry.path.clone())?;
        }
        let mut actual = BTreeSet::new();
        inventory(&root.join("corpus").join(directory), "", 0, &mut actual)?;
        ensure!(actual == expected, "{directory} inventory changed");
    }
    Ok(corpus)
}

pub fn verify_entry(root: &Path, entry: &Entry, directory: &str) -> Result<Vec<u8>> {
    ensure!(
        matches!(directory, "presets" | "textures"),
        "invalid corpus directory"
    );
    let relative = Path::new(&entry.path);
    ensure!(
        !entry.path.is_empty()
            && !entry.path.contains('\\')
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe corpus path: {}",
        entry.path
    );
    ensure!(
        entry.copied_path == format!("corpus/{directory}/{}", entry.path),
        "copied path mismatch"
    );
    let base = root.join("corpus").join(directory).canonicalize()?;
    let path = base.join(relative);
    ensure!(
        !std::fs::symlink_metadata(&path)?.file_type().is_symlink(),
        "symlink corpus entry rejected"
    );
    ensure!(
        path.canonicalize()?.starts_with(&base),
        "corpus path escaped root"
    );
    let metadata = std::fs::metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() == entry.bytes,
        "file size mismatch"
    );
    let cap = if directory == "presets" {
        512 * 1024
    } else {
        32 * 1024 * 1024
    };
    ensure!(
        metadata.len() <= cap,
        "corpus entry exceeds containment limit"
    );
    let bytes = std::fs::read(path)?;
    ensure!(sha256(&bytes) == entry.sha256, "file hash mismatch");
    Ok(bytes)
}

fn inventory(base: &Path, prefix: &str, depth: usize, output: &mut BTreeSet<String>) -> Result<()> {
    ensure!(depth <= 16, "corpus directory nesting exceeded");
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-UTF8 corpus name"))?;
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "symlink corpus entry rejected");
        if kind.is_dir() {
            inventory(&entry.path(), &relative, depth + 1, output)?;
        } else {
            ensure!(kind.is_file(), "non-file corpus entry");
            output.insert(relative);
        }
    }
    Ok(())
}
