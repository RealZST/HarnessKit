use anyhow::{Context, Result};
use crate::models::*;
use crate::store::Store;
use crate::{adapter, deployer, scanner};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallResult {
    pub name: String,
    pub was_update: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiscoveredSkill {
    pub skill_id: String,
    pub name: String,
    pub description: String,
    pub path: String,
}

pub struct Manager {
    pub store: Store,
}

impl Manager {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<()> {
        let ext = self.store.get_extension(id)?
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", id))?;

        match ext.kind {
            ExtensionKind::Skill => self.toggle_skill(&ext, enabled)?,
            ExtensionKind::Mcp => self.toggle_mcp(&ext, enabled)?,
            ExtensionKind::Hook => self.toggle_hook(&ext, enabled)?,
            ExtensionKind::Plugin => self.toggle_plugin(&ext, enabled)?,
            ExtensionKind::Cli => {} // CLI toggle not yet implemented
        }

        // For skills: toggle all siblings (shared skills in ~/.agents/skills/)
        if ext.kind == ExtensionKind::Skill {
            let sibling_ids = self.store.find_siblings_by_source_path(id)?;
            for sib_id in &sibling_ids {
                self.store.set_enabled(sib_id, enabled)?;
            }
        } else {
            self.store.set_enabled(id, enabled)?;
        }

        Ok(())
    }

    fn toggle_skill(&self, ext: &Extension, enabled: bool) -> Result<()> {
        let source_path = ext.source_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Skill has no source_path"))?;
        let skill_file = PathBuf::from(source_path);
        let disabled_file = skill_file.with_file_name("SKILL.md.disabled");

        if enabled {
            if disabled_file.exists() {
                std::fs::rename(&disabled_file, &skill_file)
                    .context("Failed to re-enable skill")?;
            }
        } else if skill_file.exists() {
            std::fs::rename(&skill_file, &disabled_file)
                .context("Failed to disable skill")?;
        }
        Ok(())
    }

    fn toggle_mcp(&self, ext: &Extension, enabled: bool) -> Result<()> {
        let adapters = adapter::all_adapters();
        for adapter in &adapters {
            if !ext.agents.contains(&adapter.name().to_string()) { continue; }
            let config_path = adapter.mcp_config_path();

            if enabled {
                let saved = self.store.get_disabled_config(&ext.id)?
                    .ok_or_else(|| anyhow::anyhow!("No saved config for MCP server '{}'", ext.name))?;
                let entry: serde_json::Value = serde_json::from_str(&saved)?;
                deployer::restore_mcp_server(&config_path, &ext.name, &entry)?;
                self.store.set_disabled_config(&ext.id, None)?;
            } else {
                let entry = deployer::read_mcp_server_config(&config_path, &ext.name)?
                    .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found in config", ext.name))?;
                self.store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
                deployer::remove_mcp_server(&config_path, &ext.name)?;
            }
        }
        Ok(())
    }

    fn toggle_hook(&self, ext: &Extension, enabled: bool) -> Result<()> {
        let adapters = adapter::all_adapters();
        // Hook name format: "event:matcher:command"
        let parts: Vec<&str> = ext.name.splitn(3, ':').collect();
        if parts.len() < 3 {
            anyhow::bail!("Invalid hook name format: {}", ext.name);
        }
        let (event, matcher_str, command) = (parts[0], parts[1], parts[2]);
        let matcher = if matcher_str == "*" { None } else { Some(matcher_str) };

        for adapter in &adapters {
            if !ext.agents.contains(&adapter.name().to_string()) { continue; }
            let config_path = adapter.hook_config_path();

            if enabled {
                let saved = self.store.get_disabled_config(&ext.id)?
                    .ok_or_else(|| anyhow::anyhow!("No saved config for hook '{}'", ext.name))?;
                let entry: serde_json::Value = serde_json::from_str(&saved)?;
                deployer::restore_hook(&config_path, event, &entry)?;
                self.store.set_disabled_config(&ext.id, None)?;
            } else {
                let entry = deployer::read_hook_config(&config_path, event, matcher, command)?
                    .ok_or_else(|| anyhow::anyhow!("Hook '{}' not found in config", ext.name))?;
                self.store.set_disabled_config(&ext.id, Some(&entry.to_string()))?;
                deployer::remove_hook(&config_path, event, matcher, command)?;
            }
        }
        Ok(())
    }

    fn toggle_plugin(&self, ext: &Extension, enabled: bool) -> Result<()> {
        let adapters = adapter::all_adapters();
        for adapter in &adapters {
            if !ext.agents.contains(&adapter.name().to_string()) { continue; }

            if adapter.name() == "claude" {
                // Claude: config-driven (enabledPlugins in settings.json)
                // Must reconstruct the full "name@source" key used in enabledPlugins.
                let config_path = adapter.mcp_config_path();
                let plugin_key = {
                    let mut found_key = None;
                    for plugin in adapter.read_plugins() {
                        let id_name = format!("{}:{}", plugin.name, plugin.source);
                        if scanner::stable_id_for(&id_name, "plugin", adapter.name()) == ext.id {
                            found_key = Some(if plugin.source.is_empty() {
                                plugin.name.clone()
                            } else {
                                format!("{}@{}", plugin.name, plugin.source)
                            });
                            break;
                        }
                    }
                    found_key.ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in agent config", ext.name))?
                };
                if enabled {
                    let saved = self.store.get_disabled_config(&ext.id)?
                        .ok_or_else(|| anyhow::anyhow!("No saved config for plugin '{}'", ext.name))?;
                    let value: serde_json::Value = serde_json::from_str(&saved)?;
                    deployer::restore_plugin_entry(&config_path, &plugin_key, &value)?;
                    self.store.set_disabled_config(&ext.id, None)?;
                } else {
                    let value = deployer::read_plugin_config(&config_path, &plugin_key)?
                        .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found in config", ext.name))?;
                    self.store.set_disabled_config(&ext.id, Some(&value.to_string()))?;
                    deployer::remove_plugin_entry(&config_path, &plugin_key)?;
                }
            } else {
                // Cursor/Codex: filesystem-driven — rename manifest
                for plugin in adapter.read_plugins() {
                    let plugin_id_name = format!("{}:{}", plugin.name, plugin.source);
                    if scanner::stable_id_for(&plugin_id_name, "plugin", adapter.name()) != ext.id {
                        continue;
                    }
                    if let Some(ref path) = plugin.path {
                        let manifest = path.join("plugin.json");
                        let disabled_manifest = path.join("plugin.json.disabled");
                        if enabled && disabled_manifest.exists() {
                            std::fs::rename(&disabled_manifest, &manifest)?;
                        } else if !enabled && manifest.exists() {
                            std::fs::rename(&manifest, &disabled_manifest)?;
                        }
                    }
                }
            }
        }
        Ok(())
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
pub fn install_from_git(url: &str, target_dir: &Path) -> Result<InstallResult> {
    install_from_git_with_id(url, target_dir, None)
}

pub fn install_from_git_with_id(url: &str, target_dir: &Path, skill_id: Option<&str>) -> Result<InstallResult> {
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
fn resolve_and_copy_skill(clone_dir: &Path, target_dir: &Path, skill_id: Option<&str>, url: &str) -> Result<InstallResult> {
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
                let was_update = dest.is_dir();
                copy_dir_contents(candidate, &dest)?;
                return Ok(InstallResult { name, was_update });
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
                let was_update = dest.is_dir();
                copy_dir_contents(clone_dir, &dest)?;
                return Ok(InstallResult { name, was_update });
            }
        }
        anyhow::bail!("Skill '{}' not found in repository. Looked in skills/{0}/, {0}/, and root", sid);
    }

    // Generic: look for SKILL.md in root or immediate subdirectories
    if clone_dir.join("SKILL.md").exists() {
        let name = crate::scanner::parse_skill_name(&clone_dir.join("SKILL.md"))
            .unwrap_or_else(|| repo_name_from_url(url));
        let dest = target_dir.join(&name);
        let was_update = dest.is_dir();
        copy_dir_contents(clone_dir, &dest)?;
        return Ok(InstallResult { name, was_update });
    }

    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("SKILL.md").exists() {
                let name = crate::scanner::parse_skill_name(&p.join("SKILL.md"))
                    .unwrap_or_else(|| p.file_name().unwrap_or_default().to_string_lossy().to_string());
                let dest = target_dir.join(&name);
                let was_update = dest.is_dir();
                copy_dir_contents(&p, &dest)?;
                return Ok(InstallResult { name, was_update });
            }
        }
    }

    anyhow::bail!("No SKILL.md found in repository")
}

/// Discover all skills in a cloned repository directory.
pub fn scan_repo_skills(clone_dir: &Path) -> Vec<DiscoveredSkill> {
    let mut skills = Vec::new();

    // Check root SKILL.md
    let root_skill = clone_dir.join("SKILL.md");
    if root_skill.exists() {
        let (name, desc, _) = crate::scanner::parse_skill_frontmatter(
            &std::fs::read_to_string(&root_skill).unwrap_or_default()
        ).unwrap_or_else(|| (repo_name_from_url(""), String::new(), vec![]));
        // Only add root if there are no subdirectory skills
        let has_sub = has_subdirectory_skills(clone_dir);
        if !has_sub {
            skills.push(DiscoveredSkill {
                skill_id: String::new(),
                name,
                description: desc,
                path: ".".into(),
            });
            return skills;
        }
    }

    // Check skills/*/ subdirectories
    let skills_dir = clone_dir.join("skills");
    if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("SKILL.md").exists() {
                    let content = std::fs::read_to_string(p.join("SKILL.md")).unwrap_or_default();
                    let (name, desc, _) = crate::scanner::parse_skill_frontmatter(&content)
                        .unwrap_or_else(|| (entry.file_name().to_string_lossy().to_string(), String::new(), vec![]));
                    skills.push(DiscoveredSkill {
                        skill_id: entry.file_name().to_string_lossy().to_string(),
                        name,
                        description: desc,
                        path: format!("skills/{}", entry.file_name().to_string_lossy()),
                    });
                }
            }
        }
    }

    // Check immediate subdirectories (top-level)
    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let fname = entry.file_name();
            if fname == ".git" || fname == "skills" { continue; }
            if p.is_dir() && p.join("SKILL.md").exists() {
                let content = std::fs::read_to_string(p.join("SKILL.md")).unwrap_or_default();
                let (name, desc, _) = crate::scanner::parse_skill_frontmatter(&content)
                    .unwrap_or_else(|| (fname.to_string_lossy().to_string(), String::new(), vec![]));
                // Avoid duplicate if already found in skills/ scan
                if !skills.iter().any(|s| s.skill_id == fname.to_string_lossy().as_ref()) {
                    skills.push(DiscoveredSkill {
                        skill_id: fname.to_string_lossy().to_string(),
                        name,
                        description: desc,
                        path: fname.to_string_lossy().to_string(),
                    });
                }
            }
        }
    }

    skills
}

fn has_subdirectory_skills(clone_dir: &Path) -> bool {
    // Check skills/*/SKILL.md
    if let Ok(entries) = std::fs::read_dir(clone_dir.join("skills")) {
        for entry in entries.flatten() {
            if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                return true;
            }
        }
    }
    // Check */SKILL.md (immediate subdirs)
    if let Ok(entries) = std::fs::read_dir(clone_dir) {
        for entry in entries.flatten() {
            let fname = entry.file_name();
            if fname == ".git" || fname == "skills" { continue; }
            if entry.path().is_dir() && entry.path().join("SKILL.md").exists() {
                return true;
            }
        }
    }
    false
}

/// Install a specific skill from an already-cloned repository directory.
pub fn install_from_clone(clone_dir: &Path, target_dir: &Path, skill_id: Option<&str>, url: &str) -> Result<InstallResult> {
    resolve_and_copy_skill(clone_dir, target_dir, skill_id, url)
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
    use tempfile::TempDir;

    #[test]
    fn test_toggle_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();

        // Create a fake skill file so toggle_skill can rename it
        let skill_dir = dir.path().join("skills").join("test");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "---\nname: test\n---\n").unwrap();

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

            source_path: Some(skill_file.to_string_lossy().to_string()),
            cli_parent_id: None,
            cli_meta: None,
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.toggle(&ext.id, false).unwrap();
        let fetched = manager.store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_toggle_skill_renames_file() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();

        // Create a fake skill directory
        let skill_dir = dir.path().join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_file = skill_dir.join("SKILL.md");
        std::fs::write(&skill_file, "---\nname: my-skill\n---\n").unwrap();

        let ext = Extension {
            id: "test-skill-id".into(),
            kind: ExtensionKind::Skill,
            name: "my-skill".into(),
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

            source_path: Some(skill_file.to_string_lossy().to_string()),
            cli_parent_id: None,
            cli_meta: None,
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);

        // Disable
        manager.toggle("test-skill-id", false).unwrap();
        assert!(!skill_file.exists(), "SKILL.md should be renamed away");
        assert!(skill_dir.join("SKILL.md.disabled").exists(), "SKILL.md.disabled should exist");
        let fetched = manager.store.get_extension("test-skill-id").unwrap().unwrap();
        assert!(!fetched.enabled);

        // Re-enable
        manager.toggle("test-skill-id", true).unwrap();
        assert!(skill_file.exists(), "SKILL.md should be restored");
        assert!(!skill_dir.join("SKILL.md.disabled").exists());
        let fetched = manager.store.get_extension("test-skill-id").unwrap().unwrap();
        assert!(fetched.enabled);
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

            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
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

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(result.name, "Alpha Skill");
        assert!(!result.was_update);
        assert!(target.path().join("Alpha Skill").join("SKILL.md").exists());
    }

    #[test]
    fn resolve_skill_reports_was_update_when_dest_exists() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(&repo.path().join("skills").join("alpha"), "Alpha Skill");
        // Pre-create destination to simulate a previous install
        std::fs::create_dir_all(target.path().join("Alpha Skill")).unwrap();

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("alpha"), "").unwrap();
        assert_eq!(result.name, "Alpha Skill");
        assert!(result.was_update);
    }

    #[test]
    fn resolve_skill_from_top_level_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Skill directly at {skill_id}/
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("my-skill"), "").unwrap();
        assert_eq!(result.name, "My Skill");
    }

    #[test]
    fn resolve_root_skill_with_skill_id_single_skill_repo() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // Single-skill repo: only root SKILL.md, no subdirectories with skills
        write_skill_md(repo.path(), "Root Skill");
        // Add a non-skill subdirectory (src/)
        std::fs::create_dir_all(repo.path().join("src")).unwrap();

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some("whatever-id"), "").unwrap();
        assert_eq!(result.name, "Root Skill");
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

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "https://github.com/user/repo.git").unwrap();
        assert_eq!(result.name, "Root Skill");
    }

    #[test]
    fn resolve_generic_no_skill_id_picks_first_subdir() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        // No root SKILL.md, one subdirectory with a skill
        write_skill_md(&repo.path().join("my-skill"), "My Skill");

        let result = super::resolve_and_copy_skill(repo.path(), target.path(), None, "").unwrap();
        assert_eq!(result.name, "My Skill");
    }

    #[test]
    fn resolve_empty_skill_id_treated_as_none() {
        let repo = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();

        write_skill_md(repo.path(), "Root Skill");

        // Empty string should be treated as None (generic path)
        let result = super::resolve_and_copy_skill(repo.path(), target.path(), Some(""), "").unwrap();
        assert_eq!(result.name, "Root Skill");
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
