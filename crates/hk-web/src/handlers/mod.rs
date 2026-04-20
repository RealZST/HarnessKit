pub mod agents;
pub mod audit;
pub mod extensions;
pub mod install;
pub mod marketplace;
pub mod projects;
pub mod settings;

use hk_core::store::Store;
use std::path::Path;

/// Normalize a path by stripping the `\\?\` extended-length prefix that
/// `std::fs::canonicalize()` adds on Windows. This ensures `starts_with()`
/// comparisons work regardless of whether paths are canonicalized or not.
#[cfg(target_os = "windows")]
fn normalize(p: &Path) -> std::path::PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

#[cfg(not(target_os = "windows"))]
fn normalize(p: &Path) -> std::path::PathBuf {
    p.to_path_buf()
}

/// Check if a path is within the home directory or any registered project path.
pub(crate) fn is_path_allowed(path: &Path, store: &Store) -> bool {
    let normalized = normalize(path);
    if let Some(home) = dirs::home_dir() {
        if normalized.starts_with(&home) {
            return true;
        }
    }
    if let Ok(projects) = store.list_projects() {
        for p in &projects {
            let proj_path = normalize(Path::new(&p.path));
            if normalized.starts_with(&proj_path) {
                return true;
            }
        }
    }
    false
}
