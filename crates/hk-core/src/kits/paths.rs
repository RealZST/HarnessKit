use crate::HkError;
use std::path::PathBuf;

/// Canonical directory holding all Kit zip files: `~/.harnesskit/kits/`.
///
/// `HARNESSKIT_HOME` overrides the home root (used by tests on Windows,
/// where `crate::home_dir()` reads the shell profile folder and ignores
/// `HOME`/`USERPROFILE`, so temp-home redirection can't work).
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
