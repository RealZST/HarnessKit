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
///
/// Both the input path and each candidate root are canonicalized before
/// comparison so that environments where the home directory is a symlink
/// (e.g. Lumi's `/users/<user>` → `/pfs/lustrep*/users/<user>`) match
/// correctly. Non-existent paths fail canonicalization and are rejected,
/// matching the desktop's `is_path_within_allowed_dirs` behavior.
pub(crate) fn is_path_allowed(path: &Path, store: &Store) -> bool {
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return false;
    };
    let canonical = normalize(&canonical);

    let under = |root: &Path| -> bool {
        std::fs::canonicalize(root)
            .map(|r| canonical.starts_with(normalize(&r)))
            .unwrap_or(false)
    };

    if let Some(home) = dirs::home_dir()
        && under(&home)
    {
        return true;
    }
    if let Ok(projects) = store.list_projects() {
        for p in &projects {
            if under(Path::new(&p.path)) {
                return true;
            }
        }
    }
    false
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use chrono::Utc;
    use hk_core::models::Project;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn test_store(db_dir: &Path) -> Store {
        Store::open(&db_dir.join("test.db")).unwrap()
    }

    fn register_project(store: &Store, path: &Path) {
        store
            .insert_project(&Project {
                id: "proj-test".into(),
                name: "test".into(),
                path: path.to_string_lossy().to_string(),
                created_at: Utc::now(),
                exists: true,
            })
            .unwrap();
    }

    /// Symlinked project root: a file path under the symlink should be allowed
    /// even though canonicalize resolves it to a different real path. This is
    /// the Lumi case (`/users/<user>` → `/pfs/lustrep*/users/<user>`).
    #[test]
    fn allows_path_under_symlinked_root() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        let link = tmp.path().join("link");
        fs::create_dir_all(&real).unwrap();
        symlink(&real, &link).unwrap();
        let file = real.join("config.json");
        fs::write(&file, "{}").unwrap();

        let store = test_store(tmp.path());
        register_project(&store, &link);

        // Caller passes a path through the symlink; canonical resolves to real.
        let via_link = link.join("config.json");
        assert!(is_path_allowed(&via_link, &store));
        // Caller passes the already-canonical real path.
        assert!(is_path_allowed(&file, &store));
    }

    /// Non-existent path: canonicalize fails, function rejects. Matches the
    /// desktop's `is_path_within_allowed_dirs` behavior.
    #[test]
    fn rejects_nonexistent_path() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let store = test_store(tmp.path());
        register_project(&store, &project);

        let ghost = project.join("does-not-exist.json");
        assert!(!is_path_allowed(&ghost, &store));
    }

    /// Path outside any registered root: rejected.
    #[test]
    fn rejects_path_outside_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let project = tmp.path().join("project");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let stray = outside.join("file.json");
        fs::write(&stray, "{}").unwrap();

        let store = test_store(tmp.path());
        register_project(&store, &project);

        assert!(!is_path_allowed(&stray, &store));
    }
}
