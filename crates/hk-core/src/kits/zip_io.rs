use crate::kits::manifest::KitManifest;
use crate::HkError;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipArchive;

const MANIFEST_NAME: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct PackEntry {
    /// Path inside the zip (e.g. `"assets/ext-a/SKILL.md"`).
    pub zip_path: String,
    pub bytes: Vec<u8>,
}

/// Pack a Kit zip atomically: write to `<target>.tmp`, fsync, rename.
pub fn pack_kit(
    target: &Path,
    manifest: &KitManifest,
    entries: &[PackEntry],
) -> Result<(), HkError> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = tmp_sibling(target);
    // Disallow proceeding if .tmp is a directory or otherwise non-file blocker:
    if tmp.exists() && tmp.is_dir() {
        return Err(HkError::Internal(format!(
            "temp path occupied by directory: {}",
            tmp.display()
        )));
    }
    let mut guard = crate::kits::util::RemovePathGuard::new(&tmp);
    // Scope the writer so the file is closed before rename.
    {
        let f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        let mut w = zip::ZipWriter::new(f);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let manifest_json = serde_json::to_vec_pretty(manifest)
            .map_err(|e| HkError::Internal(format!("manifest serialize: {e}")))?;
        w.start_file(MANIFEST_NAME, opts).map_err(zip_err)?;
        w.write_all(&manifest_json)?;

        for entry in entries {
            validate_entry_path(&entry.zip_path)?;
            w.start_file(&entry.zip_path, opts).map_err(zip_err)?;
            w.write_all(&entry.bytes)?;
        }

        let mut f = w.finish().map_err(zip_err)?;
        f.flush()?;
        f.sync_all().ok(); // best-effort fsync; some platforms reject on tempfs
    }
    // Atomic rename. On Windows this can fail if target is held open elsewhere;
    // for our use case the target is owned by this process.
    std::fs::rename(&tmp, target)?;
    guard.disarm();
    Ok(())
}

/// Parse `manifest.json` out of a Kit zip without extracting anything else.
pub fn read_manifest_from_zip(zip_path: &Path) -> Result<KitManifest, HkError> {
    let f = File::open(zip_path)?;
    let mut archive = ZipArchive::new(f).map_err(zip_err)?;
    let mut file = archive.by_name(MANIFEST_NAME).map_err(|_| {
        HkError::ConfigCorrupted(format!(
            "manifest.json missing from {}",
            zip_path.display()
        ))
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let manifest: KitManifest = serde_json::from_slice(&bytes)
        .map_err(|e| HkError::Internal(format!("manifest parse: {e}")))?;
    Ok(manifest)
}

/// Extract a single entry (file) from a zip to a target absolute path.
pub fn extract_entry_to_path(
    zip_path: &Path,
    entry_name: &str,
    target: &Path,
) -> Result<(), HkError> {
    validate_entry_path(entry_name)?;
    let f = File::open(zip_path)?;
    let mut archive = ZipArchive::new(f).map_err(zip_err)?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|_| HkError::NotFound(format!("zip entry missing: {entry_name}")))?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = File::create(target)?;
    std::io::copy(&mut entry, &mut out)?;
    out.flush()?;
    Ok(())
}

/// Extract all entries whose names start with `prefix` into `target_dir`,
/// preserving the relative structure after the prefix.
/// Returns the absolute paths written.
///
/// `prefix` must end with `/` to avoid matching e.g. `assets/ext-abcdef/`
/// when the caller intended `assets/ext-abc/`.
pub fn extract_prefix_to_dir(
    zip_path: &Path,
    prefix: &str,
    target_dir: &Path,
) -> Result<Vec<PathBuf>, HkError> {
    if !prefix.ends_with('/') {
        return Err(HkError::Internal(format!(
            "extract_prefix_to_dir: prefix must end with '/', got: {prefix}"
        )));
    }
    let f = File::open(zip_path)?;
    let mut archive = ZipArchive::new(f).map_err(zip_err)?;
    let mut written = Vec::new();
    std::fs::create_dir_all(target_dir)?;
    let base_canon = target_dir.canonicalize().unwrap_or_else(|_| target_dir.to_path_buf());
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(zip_err)?;
        let raw = entry.name().to_string();
        if !raw.starts_with(prefix) {
            continue;
        }
        validate_entry_path(&raw)?;
        let rel = raw.strip_prefix(prefix).unwrap_or(&raw);
        if rel.is_empty() || rel.ends_with('/') {
            continue;
        }
        let dst = target_dir.join(rel);

        // Defense in depth: verify dst is still within target_dir even after
        // path resolution. Use canonical path matching where possible.
        // path doesn't exist yet (expected), skip canonical check
        let dst_canon = dst.canonicalize().ok();
        if let Some(d) = dst_canon
            && !d.starts_with(&base_canon)
        {
            return Err(HkError::PathNotAllowed(format!(
                "zip entry escapes target dir: {raw}"
            )));
        }
        // Component-level check as a fallback for the non-canonicalize case.
        for comp in rel.split('/') {
            if comp == ".." || comp == "." {
                return Err(HkError::PathNotAllowed(format!(
                    "zip entry escapes target dir via rel: {raw}"
                )));
            }
        }

        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&dst)?;
        std::io::copy(&mut entry, &mut out)?;
        out.flush()?;
        written.push(dst);
    }
    Ok(written)
}

/// Reject paths that would escape the destination directory.
pub fn validate_entry_path(p: &str) -> Result<(), HkError> {
    if p.is_empty() {
        return Err(HkError::PathNotAllowed("empty zip entry name".into()));
    }
    if p.contains('\\') {
        return Err(HkError::PathNotAllowed(format!(
            "backslash in entry name: {p}"
        )));
    }
    if p.starts_with('/') {
        return Err(HkError::PathNotAllowed(format!("absolute entry name: {p}")));
    }
    // Windows drive letter (C:, D:, …)
    let bytes = p.as_bytes();
    if bytes.len() >= 2
        && bytes[1] == b':'
        && (bytes[0] as char).is_ascii_alphabetic()
    {
        return Err(HkError::PathNotAllowed(format!("drive-letter entry: {p}")));
    }
    for segment in p.split('/') {
        if segment == ".." {
            return Err(HkError::PathNotAllowed(format!("traversal in entry: {p}")));
        }
        if segment == "." {
            return Err(HkError::PathNotAllowed(format!("dot segment in entry: {p}")));
        }
    }
    Ok(())
}

fn tmp_sibling(target: &Path) -> PathBuf {
    let mut s = target.as_os_str().to_owned();
    s.push(".tmp");
    PathBuf::from(s)
}

fn zip_err(e: zip::result::ZipError) -> HkError {
    HkError::Internal(format!("zip error: {e}"))
}
