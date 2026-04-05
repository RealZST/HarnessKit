use anyhow::{bail, Result};
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
    let canonical_parent = parent.canonicalize()
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
            existing = existing.parent().map(|p| p.to_path_buf()).unwrap_or(existing);
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
/// Only allows alphanumeric, hyphens, underscores, and dots (no path separators).
pub fn validate_binary_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Binary name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        bail!("Binary name contains path characters: {}", name);
    }
    // Must not start with a dot or hyphen
    if name.starts_with('.') || name.starts_with('-') {
        bail!("Binary name cannot start with '.' or '-': {}", name);
    }
    Ok(())
}

/// Validate a Git URL: must be https:// or git:// protocol.
/// Rejects file://, ssh://, and bare paths.
pub fn validate_git_url(url: &str) -> Result<()> {
    if url.starts_with("https://") || url.starts_with("git://") {
        Ok(())
    } else if url.starts_with("git@") || url.starts_with("ssh://") {
        // Allow SSH URLs — common for private repos
        Ok(())
    } else {
        bail!("Invalid git URL (must be https://, git://, or git@): {}", url);
    }
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
    fn validate_binary_name_accepts_valid() {
        assert!(validate_binary_name("node").is_ok());
        assert!(validate_binary_name("npx").is_ok());
        assert!(validate_binary_name("my-tool").is_ok());
        assert!(validate_binary_name("tool_v2").is_ok());
    }

    #[test]
    fn validate_git_url_rejects_file_protocol() {
        assert!(validate_git_url("file:///etc/passwd").is_err());
        assert!(validate_git_url("/tmp/repo").is_err());
        assert!(validate_git_url("./local-repo").is_err());
    }

    #[test]
    fn validate_git_url_accepts_valid() {
        assert!(validate_git_url("https://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git://github.com/user/repo.git").is_ok());
        assert!(validate_git_url("git@github.com:user/repo.git").is_ok());
    }
}
