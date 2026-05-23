use crate::HkError;
use std::path::PathBuf;

/// Canonical directory holding all Kit zip files: `~/.harnesskit/kits/`.
pub fn kits_dir() -> Result<PathBuf, HkError> {
    let home = dirs::home_dir()
        .ok_or_else(|| HkError::Internal("home directory not found".into()))?;
    Ok(home.join(".harnesskit").join("kits"))
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
