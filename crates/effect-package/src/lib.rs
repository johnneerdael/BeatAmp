//! Self-contained, bounded, deterministic `.beat` archives. No disk extraction.
use anyhow::{Context, Result, ensure};
use effect_model::beat::{AssetKind, Manifest, safe_asset_path};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom, Write},
};
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MANIFEST_PATH: &str = "manifest.json";

#[derive(Debug)]
pub struct Package {
    pub manifest: Manifest,
    pub assets: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct Limits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_manifest_bytes: u64,
    pub max_archive_bytes: u64,
    pub max_directory_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 4096,
            max_entry_bytes: 256 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_manifest_bytes: 4 * 1024 * 1024,
            max_archive_bytes: 1024 * 1024 * 1024 + 16 * 1024 * 1024,
            max_directory_bytes: 4 * 1024 * 1024,
        }
    }
}

impl Package {
    pub fn read(mut reader: impl Read + Seek, limits: &Limits) -> Result<Self> {
        // zip8's Config has no entry/directory allocation limit. Preflight the
        // restricted v1 ZIP32 footer before allowing the library to build its
        // central-directory metadata. The library owns actual ZIP decoding.
        let expected_entries = preflight(&mut reader, limits)?;
        reader.rewind()?;
        let mut archive = ZipArchive::new(reader).context("invalid beat ZIP archive")?;
        ensure!(
            archive.len() == expected_entries,
            "duplicate archive entries"
        );
        ensure!(
            archive.offset() == 0,
            "prefixed beat archive is unsupported"
        );
        let mut entries = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let raw = archive.by_index_raw(i)?;
            let name = raw.name().to_owned();
            ensure!(
                safe_asset_path(&name) && name.len() <= 512,
                "invalid archive path: {name}"
            );
            ensure!(
                std::str::from_utf8(raw.name_raw())? == name,
                "archive names must be UTF-8"
            );
            ensure!(!raw.is_symlink(), "archive symlink is forbidden: {name}");
            ensure!(
                raw.is_file() && !raw.encrypted(),
                "non-file/encrypted archive entry: {name}"
            );
            ensure!(
                raw.compression() == CompressionMethod::Stored,
                "unsupported beat compression: {name}"
            );
            let size = raw.size();
            let bound = if name == MANIFEST_PATH {
                limits.max_manifest_bytes
            } else {
                limits.max_entry_bytes
            };
            ensure!(size <= bound, "archive entry budget exceeded: {name}");
            total = total.checked_add(size).context("archive size overflow")?;
            ensure!(
                total <= limits.max_total_bytes,
                "archive total budget exceeded"
            );
            ensure!(
                !entries.contains_key(&name),
                "duplicate archive path: {name}"
            );
            drop(raw);
            let mut file = archive.by_index(i).context("cannot read beat member")?;
            let mut bytes = Vec::new();
            file.by_ref()
                .take(bound.saturating_add(1))
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 == size && bytes.len() as u64 <= bound,
                "archive member size mismatch: {name}"
            );
            entries.insert(name, bytes);
        }
        let manifest_bytes = entries
            .remove(MANIFEST_PATH)
            .context("beat manifest is missing")?;
        let manifest: Manifest =
            serde_json::from_slice(&manifest_bytes).context("invalid beat manifest")?;
        let mut assets = BTreeMap::new();
        for (id, description) in &manifest.assets {
            let bytes = entries
                .remove(&description.path)
                .with_context(|| format!("missing beat asset: {id}"))?;
            assets.insert(id.clone(), bytes);
        }
        ensure!(entries.is_empty(), "undeclared beat archive members");
        let result = Self { manifest, assets };
        result.validate_contents()?;
        Ok(result)
    }

    pub fn write(&self, writer: impl Write + Seek) -> Result<()> {
        self.write_with_limits(writer, &Limits::default())
    }

    pub fn write_with_limits(&self, mut writer: impl Write + Seek, limits: &Limits) -> Result<()> {
        self.validate_contents()?;
        validate_limits(limits)?;
        let manifest = serde_json::to_vec(&self.manifest)?;
        ensure!(
            manifest.len() as u64 <= limits.max_manifest_bytes,
            "manifest budget exceeded"
        );
        ensure!(
            self.assets.len() < limits.max_entries,
            "archive entry count budget exceeded"
        );
        let mut entries = BTreeMap::from([(MANIFEST_PATH.to_owned(), manifest)]);
        let mut total = entries[MANIFEST_PATH].len() as u64;
        let mut directory_bytes = 46 + MANIFEST_PATH.len() as u64;
        for (id, description) in &self.manifest.assets {
            let bytes = &self.assets[id];
            ensure!(
                bytes.len() as u64 <= limits.max_entry_bytes,
                "asset entry budget exceeded: {id}"
            );
            total = total
                .checked_add(bytes.len() as u64)
                .context("package size overflow")?;
            directory_bytes = directory_bytes
                .checked_add(46 + description.path.len() as u64)
                .context("directory size overflow")?;
            entries.insert(description.path.clone(), bytes.clone());
        }
        ensure!(
            total <= limits.max_total_bytes,
            "package total budget exceeded"
        );
        ensure!(
            directory_bytes <= limits.max_directory_bytes,
            "archive directory budget exceeded"
        );
        let names_bytes: u64 = entries.keys().map(|s| s.len() as u64).sum();
        let archive_bytes = total
            .checked_add(76 * entries.len() as u64 + 2 * names_bytes + 22)
            .context("archive size overflow")?;
        ensure!(
            archive_bytes <= limits.max_archive_bytes && archive_bytes < u32::MAX as u64,
            "archive size budget exceeded"
        );
        ensure!(
            writer.stream_position()? == 0,
            "beat archive must start at offset zero"
        );
        ensure!(
            writer.seek(SeekFrom::End(0))? == 0,
            "beat archive writer must be empty"
        );
        writer.rewind()?;
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644);
        let mut archive = ZipWriter::new(writer);
        for (name, data) in entries {
            archive.start_file(name, options)?;
            archive.write_all(&data)?;
        }
        archive.finish()?;
        Ok(())
    }

    pub fn validate_contents(&self) -> Result<()> {
        ensure!(
            self.manifest.format == "beat" && self.manifest.version == 1,
            "unsupported beat format/version"
        );
        ensure!(
            !self.manifest.name.trim().is_empty() && !self.manifest.author.trim().is_empty(),
            "empty effect identity"
        );
        ensure!(
            self.assets.len() == self.manifest.assets.len(),
            "undeclared/missing asset bytes"
        );
        let mut paths = BTreeSet::new();
        for (id, description) in &self.manifest.assets {
            ensure!(
                !id.is_empty()
                    && safe_asset_path(&description.path)
                    && description.path.len() <= 512,
                "invalid asset path/identity: {id}"
            );
            ensure!(
                description.path != MANIFEST_PATH && paths.insert(&description.path),
                "duplicate/reserved asset path: {id}"
            );
            let bytes = self
                .assets
                .get(id)
                .with_context(|| format!("missing asset bytes: {id}"))?;
            let actual = format!("{:x}", Sha256::digest(bytes));
            ensure!(
                description.sha256.len() == 64
                    && description.sha256.bytes().all(|c| c.is_ascii_hexdigit())
                    && actual.eq_ignore_ascii_case(&description.sha256),
                "asset hash mismatch: {id}"
            );
            if description.kind == AssetKind::Wgsl {
                std::str::from_utf8(bytes).context("WGSL asset is not UTF-8")?;
            }
        }
        Ok(())
    }
}

fn validate_limits(limits: &Limits) -> Result<()> {
    ensure!(
        limits.max_entries > 0 && limits.max_entries < u16::MAX as usize,
        "unsupported archive entry limit"
    );
    ensure!(
        limits.max_archive_bytes < u32::MAX as u64,
        "ZIP64 beat archives are unsupported in version1"
    );
    Ok(())
}

fn preflight(reader: &mut (impl Read + Seek), limits: &Limits) -> Result<usize> {
    validate_limits(limits)?;
    let length = reader.seek(SeekFrom::End(0))?;
    ensure!(
        length >= 22 && length <= limits.max_archive_bytes,
        "archive size budget exceeded"
    );
    reader.seek(SeekFrom::End(-22))?;
    let mut footer = [0; 22];
    reader.read_exact(&mut footer)?;
    ensure!(
        &footer[..4] == b"PK\x05\x06" && footer[20..] == [0, 0],
        "unsupported ZIP footer/comment"
    );
    let word = |n| u16::from_le_bytes([footer[n], footer[n + 1]]);
    let dword = |n| u32::from_le_bytes(footer[n..n + 4].try_into().unwrap()) as u64;
    let count = word(10) as usize;
    ensure!(
        word(4) == 0 && word(6) == 0 && word(8) as usize == count,
        "split archives are unsupported"
    );
    ensure!(
        count > 0 && count < u16::MAX as usize && count <= limits.max_entries,
        "archive entry count budget exceeded"
    );
    let directory_size = dword(12);
    let directory_start = dword(16);
    ensure!(
        directory_size <= limits.max_directory_bytes,
        "archive directory budget exceeded"
    );
    ensure!(
        directory_start.checked_add(directory_size) == Some(length - 22),
        "unsupported ZIP64/directory layout"
    );
    Ok(count)
}
