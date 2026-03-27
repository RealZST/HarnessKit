use anyhow::{Context, Result};
use crate::models::{Source, SourceOrigin, UpdateStatus};
use crate::store::Store;
use std::path::Path;
use std::process::Command;

pub struct Manager {
    pub store: Store,
}

impl Manager {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<()> {
        self.store.set_enabled(id, enabled)
    }

    pub fn uninstall(&self, id: &str) -> Result<()> {
        self.store.delete_extension(id)
    }

    pub fn update_tags(&self, _id: &str, _tags: Vec<String>) -> Result<()> {
        // v1: tags stored in extension_tags_json, update via store
        // Implementation: read extension, modify tags, write back
        Ok(())
    }
}

/// Check if a git-sourced extension has an update available
pub fn check_update(source: &Source) -> UpdateStatus {
    if source.origin != SourceOrigin::Git {
        return UpdateStatus::Error { message: "Not a git-sourced extension".into() };
    }
    let url = match &source.url {
        Some(u) => u,
        None => return UpdateStatus::Error { message: "No remote URL".into() },
    };
    let local_hash = match &source.commit_hash {
        Some(h) => h,
        None => return UpdateStatus::Error { message: "No local commit hash".into() },
    };

    match get_remote_head(url) {
        Ok(remote_hash) => {
            if remote_hash.starts_with(local_hash) || local_hash.starts_with(&remote_hash) {
                UpdateStatus::UpToDate
            } else {
                UpdateStatus::UpdateAvailable { remote_hash }
            }
        }
        Err(e) => UpdateStatus::Error { message: e.to_string() },
    }
}

fn get_remote_head(url: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["ls-remote", "--heads", url])
        .output()
        .context("Failed to run git ls-remote")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git ls-remote failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // First line typically has the main/master branch hash
    // Format: "<hash>\trefs/heads/<branch>"
    // Prefer main, then master, then first entry
    let lines: Vec<&str> = stdout.lines().collect();
    for suffix in &["refs/heads/main", "refs/heads/master"] {
        if let Some(line) = lines.iter().find(|l| l.ends_with(suffix)) {
            if let Some(hash) = line.split_whitespace().next() {
                return Ok(hash.to_string());
            }
        }
    }
    // Fallback to first entry
    if let Some(first) = lines.first() {
        if let Some(hash) = first.split_whitespace().next() {
            return Ok(hash.to_string());
        }
    }
    anyhow::bail!("No refs found for remote")
}

/// Install a skill from a git URL by cloning and copying to the skills directory.
/// If `skill_id` is provided and non-empty, install only the matching skill subdirectory.
pub fn install_from_git(url: &str, target_dir: &Path) -> Result<String> {
    install_from_git_with_id(url, target_dir, None)
}

pub fn install_from_git_with_id(url: &str, target_dir: &Path, skill_id: Option<&str>) -> Result<String> {
    let temp = tempfile::tempdir().context("Failed to create temp directory")?;
    let clone_dir = temp.path().join("repo");

    let output = Command::new("git")
        .args(["clone", "--depth", "1", url, &clone_dir.to_string_lossy()])
        .output()
        .context("Failed to run git clone")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git clone failed: {}", stderr.trim());
    }

    resolve_and_copy_skill(&clone_dir, target_dir, skill_id, url)
}

/// Given an already-cloned repo directory, resolve which skill to install and copy it.
/// Extracted from `install_from_git_with_id` for testability.
fn resolve_and_copy_skill(clone_dir: &Path, target_dir: &Path, skill_id: Option<&str>, url: &str) -> Result<String> {
    let skill_id = skill_id.filter(|s| !s.is_empty());

    // If skill_id is specified, look for it in specific paths
    if let Some(sid) = skill_id {
        // Try: skills/{skill_id}/, {skill_id}/
        let candidates = [
            clone_dir.join("skills").join(sid),
            clone_dir.join(sid),
        ];
        for candidate in &candidates {
            if candidate.is_dir() && candidate.join("SKILL.md").exists() {
                let name = crate::scanner::parse_skill_name(&candidate.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                let dest = target_dir.join(&name);
                copy_dir_contents(candidate, &dest)?;
                return Ok(name);
            }
        }
        // Fallback: root-level SKILL.md, but only for genuine single-skill repos.
        // If any subdirectory also contains SKILL.md, the specified skill_id should
        // have matched one of them — don't silently install the root.
        if clone_dir.join("SKILL.md").exists() {
            let has_sub_skills = std::fs::read_dir(clone_dir).ok()
                .map(|entries| entries.flatten().any(|e| {
                    let p = e.path();
                    if !p.is_dir() { return false; }
                    let name = e.file_name();
                    if name == ".git" { return false; }
                    if name == "skills" {
                        // Check inside skills/ directory
                        return std::fs::read_dir(&p).ok()
                            .map(|subs| subs.flatten().any(|s| s.path().join("SKILL.md").exists()))
                            .unwrap_or(false);
                    }
                    p.join("SKILL.md").exists()
                }))
                .unwrap_or(false);
            if !has_sub_skills {
                let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
                    .unwrap_or_else(|| sid.to_string());
                let dest = target_dir.join(&name);
                copy_dir_contents(clone_dir, &dest)?;
                return Ok(name);
            }
        }
        anyhow::bail!("Skill '{}' not found in repository. Looked in skills/{0}/, {0}/, and root", sid);
    }

    // Generic: look for SKILL.md in root or immediate subdirectories
    let skill_name = if clone_dir.join("SKILL.md").exists() {
        let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
            .unwrap_or_else(|| repo_name_from_url(url));
        let dest = target_dir.join(&name);
        copy_dir_contents(clone_dir, &dest)?;
        name
    } else {
        let mut found_name = None;
        if let Ok(entries) = std::fs::read_dir(clone_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    let name = crate::scanner::parse_skill_name(&p.join("SKILL.md"))
                        .unwrap_or_else(|| p.file_name().unwrap_or_default().to_string_lossy().to_string());
                    let dest = target_dir.join(&name);
                    copy_dir_contents(&p, &dest)?;
                    found_name = Some(name);
                    break;
                }
            }
        }
        found_name.ok_or_else(|| anyhow::anyhow!("No SKILL.md found in repository"))?
    };

    Ok(skill_name)
}

fn repo_name_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .strip_suffix(".git")
        .unwrap_or(url.rsplit('/').next().unwrap_or("unknown"))
        .to_string()
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            if entry.file_name() == ".git" { continue; }
            copy_dir_contents(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use tempfile::TempDir;

    #[test]
    fn test_toggle_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();
        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "test".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            category: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.toggle(&ext.id, false).unwrap();
        let fetched = manager.store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_uninstall_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();
        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "to-delete".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            category: None,
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.uninstall(&ext.id).unwrap();
        assert!(manager.store.get_extension(&ext.id).unwrap().is_none());
    }

    // --- resolve_and_copy_skill tests ---

    /// Helper: write a minimal SKILL.md with frontmatter
    fn write_skill_md(dir: &std::path::Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), format!("---\nname: {}\n---\n", name)).unwrap();
    }

    #[test]
    fn resolve_skill_from_subdirectory() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Multi-skill repo: skills/alpha/ and skills/beta/
        write_skill_md(&repo.path().join("skills").join("alpha"), "Alpha Skill");
        write_skill_md(&repo.path().join("skills").join("beta"), "Beta Skill");

        let name = super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(name, "Alpha Skill");
        assert!(target.path().join("Alpha Skill").join("SKILL.md").exists());
    }

    #[test]
    fn resolve_skill_from_top_level_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Skill directly at {skill_id}/
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let name = super::resolve_and_copy_skill(repo.path(), target.path(), Some("my-skill"), "").unwrap();
        assert_eq!(name, "My Skill");
    }

    #[test]
    fn resolve_root_skill_with_skill_id_single_skill_repo() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Single-skill repo: only root SKILL.md, no subdirectories with skills
        write_skill_md(repo.path(), "Root Skill");
        // Add a non-skill subdirectory (src/)
        std::fs::create_dir_all(repo.path().join("src")).unwrap();

        let name = super::resolve_and_copy_skill(repo.path(), target.path(), Some("whatever-id"), "").unwrap();
        assert_eq!(name, "Root Skill");
    }

    #[test]
    fn resolve_root_fallback_blocked_when_subdirectory_skills_exist() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Multi-skill repo with root SKILL.md AND subdirectory skills
        write_skill_md(repo.path(), "Root Skill");
        write_skill_md(&repo.path().join("skills").join("real-skill"), "Real Skill");

        // Asking for wrong skill_id should NOT silently install root
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("wrong-id"), "");
        assert!(result.is_err(), "Should error when skill_id doesn't match and sub-skills exist");
        let err = result.unwrap_err().to_string();
        assert!(err.contains("wrong-id"), "Error should mention the requested skill_id");
    }

    #[test]
    fn resolve_wrong_skill_id_with_subdir_skills_at_top_level() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Root SKILL.md + top-level subdirectory skill (not under skills/)
        write_skill_md(repo.path(), "Root Skill");
        write_skill_md(&repo.path().join("other-skill"), "Other Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("nonexistent"), "");
        assert!(result.is_err(), "Should error, not silently install root");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_root() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        let name = super::resolve_and_copy_skill(repo.path(), target.path(), None, "https://github.com/user/repo.git").unwrap();
        assert_eq!(name, "Root Skill");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_first_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // No root SKILL.md, one subdirectory with a skill
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let name = super::resolve_and_copy_skill(repo.path(), target.path(), None, "").unwrap();
        assert_eq!(name, "My Skill");
    }

    #[test]
    fn resolve_empty_skill_id_treated_as_none() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        // Empty string should be treated as None (generic path)
        let name = super::resolve_and_copy_skill(repo.path(), target.path(), Some(""), "").unwrap();
        assert_eq!(name, "Root Skill");
    }

    #[test]
    fn resolve_no_skill_md_anywhere_errors() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Empty repo
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "");
        assert!(result.is_err());
    }
}
