use crate::HkError;
use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Validate that a name (skill name, skill_id) contains no path traversal sequences.
/// Rejects: empty strings, "..", "/", "\", and names starting with "."
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        bail!("Name contains invalid path characters: {}", name);
    }
    if name.starts_with('.') {
        bail!("Name cannot start with '.': {}", name);
    }
    Ok(())
}

/// Validate that `child` resolved under `parent` stays within `parent`.
/// Both paths are canonicalized before comparison.
/// If `child` does not exist yet, canonicalizes the longest existing prefix.
pub fn validate_path_within(parent: &Path, child: &Path) -> Result<PathBuf> {
    let canonical_parent = parent
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Cannot canonicalize parent {}: {}", parent.display(), e))?;

    // For paths that don't exist yet, canonicalize the longest existing ancestor
    let canonical_child = if child.exists() {
        child.canonicalize()?
    } else {
        // Walk up to find the first existing ancestor, then append the remaining components
        let mut existing = child.to_path_buf();
        let mut remaining = Vec::new();
        while !existing.exists() {
            if let Some(file_name) = existing.file_name() {
                remaining.push(file_name.to_os_string());
            } else {
                break;
            }
            existing = existing
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(existing);
        }
        let mut result = existing.canonicalize().unwrap_or(existing);
        for component in remaining.into_iter().rev() {
            result = result.join(component);
        }
        result
    };

    if !canonical_child.starts_with(&canonical_parent) {
        bail!(
            "Path escapes target directory: {} is not within {}",
            child.display(),
            parent.display()
        );
    }
    Ok(canonical_child)
}

/// Validate that a binary name is safe for `which`/`--version` execution.
/// Positive allowlist: only ASCII alphanumeric, hyphens, underscores, and dots.
/// Must not start with a dot or hyphen.
pub fn validate_binary_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Binary name cannot be empty");
    }
    // Must not start with a dot or hyphen
    if name.starts_with('.') || name.starts_with('-') {
        bail!("Binary name cannot start with '.' or '-': {}", name);
    }
    // Positive allowlist: only safe characters
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        bail!("Binary name contains disallowed characters (only alphanumeric, '-', '_', '.' allowed): {}", name);
    }
    Ok(())
}

/// Validate a Git URL: must use a recognized protocol.
/// Accepts: https://, git://, git@, ssh://, file://.
/// Rejects: bare paths (no protocol) and flag-like URLs starting with '-'.
pub fn validate_git_url(url: &str) -> Result<()> {
    if url.starts_with("https://") || url.starts_with("git://") {
        Ok(())
    } else if url.starts_with("git@") || url.starts_with("ssh://") {
        // Allow SSH URLs — common for private repos
        Ok(())
    } else if url.starts_with("file://") {
        // Allow file:// — used for local git repos (harmless with -- separator)
        Ok(())
    } else {
        bail!(
            "Invalid git URL (must be https://, git://, ssh://, or git@): {}",
            url
        );
    }
}

/// Check if a string looks like a Windows absolute path (e.g. "C:\foo" or "D:/bar").
pub fn is_windows_abs_path(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

/// Resolve `~` and validate a user-supplied custom config path.
///
/// Custom config paths are user-typed pointers to files/directories the user
/// wants HarnessKit to track. They are not subject to a home/project gate —
/// the OS already gates whether HK can read those paths via filesystem
/// permissions, and forcing HPC users (or anyone working outside `$HOME`) to
/// register a project just to track a single config file is unnecessary
/// friction.
///
/// Used by both web (`hk-web::handlers::agents`) and desktop
/// (`hk-desktop::commands::settings`) so the two surfaces stay in lockstep.
///
/// Returns `HkError` with kind-specific failure modes:
/// - `PathNotAllowed`: path contains `..` (literal traversal attempt)
/// - `Validation`: path does not exist, or equals home dir itself
pub fn resolve_and_validate_config_path(path: &str) -> Result<String, HkError> {
    let resolved = if path == "~" {
        dirs::home_dir()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string())
    } else if let Some(rest) = path.strip_prefix("~/") {
        dirs::home_dir()
            .map(|h| h.join(rest).to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string())
    } else {
        path.to_string()
    };
    if resolved.contains("..") {
        return Err(HkError::PathNotAllowed(
            "Config paths cannot contain '..' components".into(),
        ));
    }
    let resolved_path = Path::new(&resolved);
    if !resolved_path.exists() {
        return Err(HkError::Validation(
            "Path does not exist on disk. Create the file or directory before adding it.".into(),
        ));
    }
    // Compare canonical paths so trailing slashes, symlinks, and other
    // surface variants (e.g. `/Users/zoe/` vs `/Users/zoe`, or a symlink
    // pointing to home) all reject as "home itself".
    let home = dirs::home_dir().unwrap_or_default();
    if let (Ok(rp), Ok(hp)) = (
        std::fs::canonicalize(resolved_path),
        std::fs::canonicalize(&home),
    ) && rp == hp
    {
        return Err(HkError::Validation(
            "Cannot use home directory itself as a config path".into(),
        ));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn validate_name_rejects_traversal() {
        assert!(validate_name("..").is_err());
        assert!(validate_name("../etc").is_err());
        assert!(validate_name("foo/../bar").is_err());
        assert!(validate_name("foo/bar").is_err());
        assert!(validate_name("foo\\bar").is_err());
        assert!(validate_name(".hidden").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn validate_name_accepts_valid() {
        assert!(validate_name("my-skill").is_ok());
        assert!(validate_name("my_skill").is_ok());
        assert!(validate_name("MySkill123").is_ok());
        assert!(validate_name("skill.v2").is_ok());
    }

    #[test]
    fn validate_path_within_catches_traversal() {
        let dir = TempDir::new().unwrap();
        let parent = dir.path().join("skills");
        std::fs::create_dir_all(&parent).unwrap();

        // "../etc" escapes parent
        let bad = parent.join("../etc");
        assert!(validate_path_within(&parent, &bad).is_err());
    }

    #[test]
    fn validate_path_within_allows_valid() {
        let dir = TempDir::new().unwrap();
        let parent = dir.path().join("skills");
        std::fs::create_dir_all(&parent).unwrap();

        let good = parent.join("my-skill");
        assert!(validate_path_within(&parent, &good).is_ok());
    }

    #[test]
    fn validate_binary_name_rejects_paths() {
        assert!(validate_binary_name("/tmp/evil").is_err());
        assert!(validate_binary_name("./evil").is_err());
        assert!(validate_binary_name("../evil").is_err());
        assert!(validate_binary_name("").is_err());
    }

    #[test]
    fn validate_binary_name_rejects_shell_injection() {
        assert!(validate_binary_name("node;rm").is_err());
        assert!(validate_binary_name("node$(whoami)").is_err());
        assert!(validate_binary_name("node`id`").is_err());
        assert!(validate_binary_name("node|cat").is_err());
        assert!(validate_binary_name("node&bg").is_err());
        assert!(validate_binary_name("node rm").is_err()); // space
        assert!(validate_binary_name("node\ttab").is_err()); // tab
        assert!(validate_binary_name("node>file").is_err());
        assert!(validate_binary_name("node<file").is_err());
        assert!(validate_binary_name("no'de").is_err());
        assert!(validate_binary_name("no\"de").is_err());
    }

    #[test]
    fn validate_binary_name_accepts_valid() {
        assert!(validate_binary_name("node").is_ok());
        assert!(validate_binary_name("npx").is_ok());
        assert!(validate_binary_name("my-tool").is_ok());
        assert!(validate_binary_name("tool_v2").is_ok());
        assert!(validate_binary_name("tool.exe").is_ok());
        assert!(validate_binary_name("Python3").is_ok());
    }

    #[test]
    fn validate_git_url_rejects_bare_paths() {
        assert!(validate_git_url("/tmp/repo").is_err());
        assert!(validate_git_url("./local-repo").is_err());
        assert!(validate_git_url("../parent-repo").is_err());
    }

    #[test]
    fn validate_git_url_accepts_file_protocol() {
        assert!(validate_git_url("file:///tmp/repo").is_ok());
        assert!(validate_git_url("file:///home/user/project.git").is_ok());
    }

    #[test]
    fn validate_git_url_rejects_flag_injection() {
        // URLs starting with "--" could be interpreted as git flags
        assert!(validate_git_url("--upload-pack=evil").is_err());
        assert!(validate_git_url("-c http.proxy=evil").is_err());
        assert!(validate_git_url("--config=core.sshCommand=evil").is_err());
    }

    #[test]
    fn validate_git_url_accepts_valid() {
        assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
    }

    #[test]
    fn test_is_windows_abs_path() {
        assert!(is_windows_abs_path(r"C:\Users\test"));
        assert!(is_windows_abs_path("D:/Projects/foo"));
        assert!(!is_windows_abs_path("/usr/bin/env"));
        assert!(!is_windows_abs_path("relative/path"));
        assert!(!is_windows_abs_path("~/foo"));
        assert!(!is_windows_abs_path("C:"));  // too short
    }

    /// D-spirit regression: a custom config path under a directory that is
    /// neither home nor a registered project should be accepted. The OS
    /// gates whether HK can later read the file; HK does not impose its
    /// own home/project boundary on user-typed config paths.
    #[test]
    fn config_path_accepts_path_outside_home() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("custom_config.json");
        std::fs::write(&outside, "{}").unwrap();

        let result = resolve_and_validate_config_path(&outside.to_string_lossy());
        assert!(result.is_ok(), "expected ok, got: {result:?}");
    }

    #[test]
    fn config_path_rejects_dotdot() {
        let tmp = tempfile::tempdir().unwrap();
        let path = format!("{}/foo/../bar.json", tmp.path().display());
        let result = resolve_and_validate_config_path(&path);
        assert!(matches!(result, Err(HkError::PathNotAllowed(_))));
    }

    #[test]
    fn config_path_rejects_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("does-not-exist.json");
        let result = resolve_and_validate_config_path(&path.to_string_lossy());
        assert!(matches!(result, Err(HkError::Validation(_))));
    }

    /// Bare "~" should expand to home and be rejected as "home itself" — not
    /// treated as a literal "~" path which then fails the existence check.
    #[test]
    fn config_path_bare_tilde_expands_to_home_then_rejected() {
        let result = resolve_and_validate_config_path("~");
        match result {
            Err(HkError::Validation(msg)) => assert!(
                msg.contains("home directory itself"),
                "expected home-itself error, got: {msg}"
            ),
            other => panic!("expected Validation('home directory itself'), got: {other:?}"),
        }
    }

    /// Home with trailing slash (or any surface variant pointing at home)
    /// should be caught by the canonicalize-based comparison.
    #[test]
    fn config_path_rejects_home_with_trailing_slash() {
        let home = match dirs::home_dir() {
            Some(h) => h,
            None => return, // skip on environments without a home dir
        };
        let with_trailing = format!("{}/", home.to_string_lossy());
        let result = resolve_and_validate_config_path(&with_trailing);
        match result {
            Err(HkError::Validation(msg)) => assert!(
                msg.contains("home directory itself"),
                "expected home-itself error, got: {msg}"
            ),
            other => panic!("expected Validation('home directory itself'), got: {other:?}"),
        }
    }
}
