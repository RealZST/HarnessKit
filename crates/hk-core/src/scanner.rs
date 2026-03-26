use crate::adapter::AgentAdapter;
use crate::models::*;
use chrono::Utc;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// Generate a deterministic ID from name + kind + agent so re-scans produce the same ID
fn stable_id(name: &str, kind: &str, agent: &str) -> String {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    kind.hash(&mut hasher);
    agent.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Scan a skill directory and return Extension entries
pub fn scan_skill_dir(dir: &Path, agent_name: &str) -> Vec<Extension> {
    let mut extensions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return extensions };

    for entry in entries.flatten() {
        let path = entry.path();
        // Skills can be either: a directory containing SKILL.md, or a standalone .md file
        let skill_file = if path.is_dir() {
            path.join("SKILL.md")
        } else if path.extension().is_some_and(|ext| ext == "md") {
            path.clone()
        } else {
            continue;
        };

        if !skill_file.exists() { continue; }
        let Ok(content) = std::fs::read_to_string(&skill_file) else { continue; };

        let (name, description) = parse_skill_frontmatter(&content)
            .unwrap_or_else(|| {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                (name, String::new())
            });

        extensions.push(Extension {
            id: stable_id(&name, "skill", agent_name),
            kind: ExtensionKind::Skill,
            name,
            description,
            source: detect_source(&path),
            agents: vec![agent_name.to_string()],
            tags: vec![],
            permissions: infer_skill_permissions(&content),
            enabled: true,
            trust_score: None,
            installed_at: file_created_time(&path),
            updated_at: file_modified_time(&path),
        });
    }
    extensions
}

/// Scan MCP servers from an agent adapter
pub fn scan_mcp_servers(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    adapter.read_mcp_servers().into_iter().map(|server| {
        let mut permissions = Vec::new();
        if !server.env.is_empty() {
            permissions.push(Permission::Env { keys: server.env.keys().cloned().collect() });
        }
        permissions.push(Permission::Shell { commands: vec![server.command.clone()] });
        // Infer network permission if command is known network tool
        if server.command.contains("npx") || server.args.iter().any(|a| a.contains("http")) {
            permissions.push(Permission::Network { domains: vec!["*".into()] });
        }

        Extension {
            id: stable_id(&server.name, "mcp", &adapter.name()),
            kind: ExtensionKind::Mcp,
            name: server.name,
            description: format!("{} {}", server.command, server.args.join(" ")),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec![adapter.name().to_string()],
            tags: vec![],
            permissions,
            enabled: true,
            trust_score: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }).collect()
}

/// Scan hooks from an agent adapter
pub fn scan_hooks(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    adapter.read_hooks().into_iter().map(|hook| {
        let hook_name = format!("{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"));
        Extension {
            id: stable_id(&hook_name, "hook", &adapter.name()),
            kind: ExtensionKind::Hook,
            name: hook_name,
            description: hook.command.clone(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec![adapter.name().to_string()],
            tags: vec![],
            permissions: vec![Permission::Shell { commands: vec![hook.command] }],
            enabled: true,
            trust_score: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }).collect()
}

/// Scan all extensions from all detected agents
pub fn scan_all(adapters: &[Box<dyn AgentAdapter>]) -> Vec<Extension> {
    let mut all = Vec::new();
    for adapter in adapters {
        if !adapter.detect() { continue; }
        for skill_dir in adapter.skill_dirs() {
            all.extend(scan_skill_dir(&skill_dir, adapter.name()));
        }
        all.extend(scan_mcp_servers(adapter.as_ref()));
        all.extend(scan_hooks(adapter.as_ref()));
    }
    all
}

// --- Helpers ---

fn parse_skill_frontmatter(content: &str) -> Option<(String, String)> {
    if !content.starts_with("---") { return None; }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    let mut name = None;
    let mut description = None;
    for line in frontmatter.lines() {
        if let Some(val) = line.strip_prefix("name:") {
            name = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("description:") {
            description = Some(val.trim().to_string());
        }
    }
    Some((name?, description.unwrap_or_default()))
}

fn detect_source(path: &Path) -> Source {
    let mut dir = path.to_path_buf();
    while dir.pop() {
        if dir.join(".git").exists() {
            return Source {
                origin: SourceOrigin::Git,
                url: read_git_remote(&dir),
                version: None,
                commit_hash: None,
            };
        }
    }
    Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None }
}

fn read_git_remote(repo_dir: &Path) -> Option<String> {
    let config = std::fs::read_to_string(repo_dir.join(".git/config")).ok()?;
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("url = ") {
            return Some(trimmed.strip_prefix("url = ")?.to_string());
        }
    }
    None
}

fn infer_skill_permissions(content: &str) -> Vec<Permission> {
    let mut perms = Vec::new();
    let lower = content.to_lowercase();
    if lower.contains("file") || lower.contains("read") || lower.contains("write") || lower.contains("path") {
        perms.push(Permission::FileSystem { paths: vec![] });
    }
    if lower.contains("http") || lower.contains("api") || lower.contains("fetch") || lower.contains("url") {
        perms.push(Permission::Network { domains: vec![] });
    }
    if lower.contains("bash") || lower.contains("shell") || lower.contains("command") || lower.contains("exec") {
        perms.push(Permission::Shell { commands: vec![] });
    }
    if lower.contains("database") || lower.contains("sql") || lower.contains("postgres") || lower.contains("mysql") {
        perms.push(Permission::Database { engines: vec![] });
    }
    perms
}

fn file_created_time(path: &Path) -> chrono::DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.created())
        .map(|t| chrono::DateTime::<Utc>::from(t))
        .unwrap_or_else(|_| Utc::now())
}

fn file_modified_time(path: &Path) -> chrono::DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| chrono::DateTime::<Utc>::from(t))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_claude_skills(dir: &TempDir) {
        let skills_dir = dir.path().join(".claude").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        std::fs::create_dir_all(skills_dir.join("eslint-skill")).unwrap();
        std::fs::write(
            skills_dir.join("eslint-skill").join("SKILL.md"),
            "---\nname: eslint-skill\ndescription: Enforce ESLint rules\n---\nAlways run eslint before committing.",
        ).unwrap();
    }

    fn setup_claude_mcp(dir: &TempDir) {
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y","@modelcontextprotocol/server-github"],"env":{"GITHUB_TOKEN":"test"}}}}"#,
        ).unwrap();
    }

    #[test]
    fn test_scan_skills_from_directory() {
        let dir = TempDir::new().unwrap();
        setup_claude_skills(&dir);
        let skills_dir = dir.path().join(".claude").join("skills");
        let extensions = scan_skill_dir(&skills_dir, "claude");
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "eslint-skill");
        assert_eq!(extensions[0].kind, ExtensionKind::Skill);
    }

    #[test]
    fn test_scan_mcp_from_adapter() {
        let dir = TempDir::new().unwrap();
        setup_claude_mcp(&dir);
        let adapter = crate::adapter::claude::ClaudeAdapter::with_home(dir.path().to_path_buf());
        let extensions = scan_mcp_servers(&adapter);
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "github");
        assert_eq!(extensions[0].kind, ExtensionKind::Mcp);
    }
}
