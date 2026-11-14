pub mod adapter;
pub mod auditor;
pub mod config;
pub mod deployer;
pub mod error;
pub mod kits;
pub mod manager;
pub mod marketplace;
pub mod models;
pub mod process;
pub mod sanitize;
pub mod scanner;
pub mod service;
pub mod skills_cli;
pub mod store;

pub use error::HkError;
pub use process::command;

/// Process-wide home directory used by every adapter and path helper.
///
/// `HARNESSKIT_HOME` wins when set (test isolation on Windows, where
/// `dirs::home_dir()` reads the shell profile folder and cannot be
/// redirected); otherwise it is `dirs::home_dir()`.
pub fn home_dir() -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("HARNESSKIT_HOME").filter(|v| !v.is_empty()) {
        return std::path::PathBuf::from(root);
    }
    dirs::home_dir().unwrap_or_default()
}

/// Compare two filesystem paths, resolving symlinks where possible so that
/// e.g. `~/.gemini/antigravity/skills` (symlink) and `~/.claude/skills`
/// (target) are recognized as the same location. Falls back to
/// textual equality when either path can't be canonicalized (doesn't yet
/// exist, permission error, etc.) so non-existent paths still compare
/// correctly against each other.
pub fn paths_equal(a: &std::path::Path, b: &std::path::Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::paths_equal;
    #[cfg(unix)]
    use tempfile::TempDir;

    #[test]
    fn paths_equal_textual_match_for_nonexistent_paths() {
        // Falls back to textual eq when canonicalize fails (paths don't exist).
        let p = std::path::Path::new("/nonexistent/path/x");
        assert!(paths_equal(p, p));
    }

    #[test]
    fn paths_equal_different_nonexistent_paths_not_equal() {
        // Different non-existent paths return false (canonicalize fails, no fallback match).
        let a = std::path::Path::new("/nonexistent/a");
        let b = std::path::Path::new("/nonexistent/b");
        assert!(!paths_equal(a, b));
    }

    #[test]
    #[cfg(unix)]
    fn paths_equal_resolves_symlink_to_same_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(paths_equal(&target, &link));
        assert!(paths_equal(&link, &target));
    }

    #[test]
    #[cfg(windows)]
    fn paths_equal_however_windows_spells_the_path() {
        // Claude records a project's path as the shell handed it over: a
        // lower-case drive letter, forward slashes, or another letter case.
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("Proj");
        std::fs::create_dir_all(&project).unwrap();
        let spelled = project.to_string_lossy().into_owned();
        let lower_drive = format!("{}{}", spelled[..1].to_lowercase(), &spelled[1..]);
        let same = |other: &str| paths_equal(std::path::Path::new(other), &project);

        assert!(same(&lower_drive));
        assert!(same(&spelled.replace('\\', "/")));
        assert!(same(&spelled.to_lowercase()));
        assert!(!paths_equal(tmp.path(), &project));
    }

    #[test]
    #[cfg(unix)]
    fn paths_equal_distinct_real_dirs_not_equal() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        assert!(!paths_equal(&a, &b));
    }
}
