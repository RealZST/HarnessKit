use crate::HkError;
use std::path::PathBuf;

/// Canonical directory holding all Kit zip files: `~/.harnesskit/kits/`.
///
/// The home root is `crate::home_dir()`, which tests redirect through
/// `HARNESSKIT_HOME` (the only override that works on Windows).
pub fn kits_dir() -> Result<PathBuf, HkError> {
    Ok(crate::home_dir().join(".harnesskit").join("kits"))
}

/// Ensure `~/.harnesskit/kits/` exists; idempotent.
pub fn ensure_kits_dir() -> Result<PathBuf, HkError> {
    let dir = kits_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Build the canonical zip path for a Kit id.
pub fn zip_path_for(kit_id: &str) -> Result<PathBuf, HkError> {
    Ok(kits_dir()?.join(format!("{kit_id}.hk-kit.zip")))
}
