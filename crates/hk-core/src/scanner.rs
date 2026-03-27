use crate::adapter::{AgentAdapter, HookEntry, McpServerEntry};
use crate::models::*;
use chrono::Utc;
use std::path::Path;

/// FNV-1a 64-bit hash — deterministic across Rust versions (unlike DefaultHasher).
fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Generate a deterministic ID from name + kind + agent so re-scans produce the same ID
fn stable_id(name: &str, kind: &str, agent: &str) -> String {
    let key = format!("{}:{}:{}", kind, agent, name);
    format!("{:016x}", fnv1a(key.as_bytes()))
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

        let category = infer_category(&name, &content);
        extensions.push(Extension {
            id: stable_id(&name, "skill", agent_name),
            kind: ExtensionKind::Skill,
            name,
            description,
            source: detect_source(&path, true),
            agents: vec![agent_name.to_string()],
            tags: vec![],
            category,
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
    let config_path = adapter.mcp_config_path();
    let config_created = file_created_time(&config_path);
    let config_modified = file_modified_time(&config_path);

    adapter.read_mcp_servers().into_iter().map(|server| {
        let cmd_basename = Path::new(&server.command)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut permissions = Vec::new();
        if !server.env.is_empty() {
            permissions.push(Permission::Env { keys: server.env.keys().cloned().collect() });
        }
        permissions.push(Permission::Shell { commands: vec![cmd_basename.clone()] });
        if cmd_basename == "npx" || cmd_basename == "uvx" || server.args.iter().any(|a| a.contains("http")) {
            permissions.push(Permission::Network { domains: vec!["*".into()] });
        }

        // Build a human-readable description from the command
        let description = if cmd_basename == "npx" || cmd_basename == "uvx" {
            // Show the package name (usually the last meaningful arg)
            let pkg = server.args.iter().filter(|a| !a.starts_with('-')).last();
            match pkg {
                Some(p) => format!("Runs {} via {}", p, cmd_basename),
                None => format!("Runs via {}", cmd_basename),
            }
        } else {
            let args_summary: Vec<&str> = server.args.iter()
                .filter(|a| !a.starts_with('-'))
                .map(|s| s.as_str())
                .take(2)
                .collect();
            if args_summary.is_empty() {
                format!("Runs {}", cmd_basename)
            } else {
                format!("Runs {} {}", cmd_basename, args_summary.join(" "))
            }
        };

        // Build rich text for categorization: name + command + args + env keys
        let cat_text = format!("{} {} {} {}",
            description,
            server.command,
            server.args.join(" "),
            server.env.keys().cloned().collect::<Vec<_>>().join(" "),
        );
        let category = infer_category(&server.name, &cat_text);

        Extension {
            id: stable_id(&server.name, "mcp", &adapter.name()),
            kind: ExtensionKind::Mcp,
            name: server.name,
            description,
            source: Source { origin: SourceOrigin::Agent, url: None, version: None, commit_hash: None },
            agents: vec![adapter.name().to_string()],
            tags: vec![],
            category,
            permissions,
            enabled: true,
            trust_score: None,
            installed_at: config_created,
            updated_at: config_modified,
        }
    }).collect()
}

/// Scan hooks from an agent adapter
pub fn scan_hooks(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    let config_path = adapter.hook_config_path();
    let config_created = file_created_time(&config_path);
    let config_modified = file_modified_time(&config_path);

    adapter.read_hooks().into_iter().map(|hook| {
        let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
        let cmd_basename = hook.command.split_whitespace().next()
            .map(|c| Path::new(c).file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_default();
        let description = format!("Runs `{}` on {} event", hook.command, hook.event);
        let category = infer_category(&hook_name, &hook.command);

        Extension {
            id: stable_id(&hook_name, "hook", &adapter.name()),
            kind: ExtensionKind::Hook,
            name: hook_name,
            description,
            source: Source { origin: SourceOrigin::Agent, url: None, version: None, commit_hash: None },
            agents: vec![adapter.name().to_string()],
            tags: vec![],
            category,
            permissions: vec![Permission::Shell { commands: vec![cmd_basename] }],
            enabled: true,
            trust_score: None,
            installed_at: config_created,
            updated_at: config_modified,
        }
    }).collect()
}

/// Scan plugins from an agent adapter
pub fn scan_plugins(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    adapter.read_plugins().into_iter().map(|plugin| {
        let description = if plugin.source.is_empty() {
            format!("Plugin for {}", adapter.name())
        } else {
            format!("Plugin from {}", plugin.source)
        };
        // Read manifest content for richer categorization
        let manifest_text = plugin.path.as_ref().and_then(|p| {
            // Try common manifest files
            for name in &["plugin.json", ".cursor-plugin/plugin.json", ".codex-plugin/plugin.json"] {
                let manifest = p.join(name);
                if manifest.exists() {
                    if let Ok(content) = std::fs::read_to_string(&manifest) {
                        return Some(content);
                    }
                }
            }
            None
        }).unwrap_or_default();
        let category = infer_category(&plugin.name, &format!("{} {}", description, manifest_text));

        // Plugins run code, so they implicitly have shell/filesystem permissions
        let permissions = vec![
            Permission::Shell { commands: vec![] },
            Permission::FileSystem { paths: vec![] },
        ];

        let (installed_at, updated_at) = plugin.path.as_ref()
            .map(|p| (file_created_time(p), file_modified_time(p)))
            .unwrap_or_else(|| (Utc::now(), Utc::now()));

        Extension {
            id: stable_id(&format!("{}:{}", plugin.name, plugin.source), "plugin", adapter.name()),
            kind: ExtensionKind::Plugin,
            name: plugin.name,
            description,
            source: Source { origin: SourceOrigin::Agent, url: None, version: None, commit_hash: None },
            agents: vec![adapter.name().to_string()],
            tags: vec![],
            category,
            permissions,
            enabled: plugin.enabled,
            trust_score: None,
            installed_at,
            updated_at,
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
        all.extend(scan_plugins(adapter.as_ref()));
    }
    all
}

/// Generate a deterministic ID for project extensions, including the project path
/// to avoid collisions with user-level extensions.
fn project_stable_id(name: &str, kind: &str, project_path: &str) -> String {
    let key = format!("{}:project:{}:{}", kind, project_path, name);
    format!("{:016x}", fnv1a(key.as_bytes()))
}

/// Parse MCP servers from a JSON file containing `{"mcpServers": {...}}`
fn parse_mcp_servers_from_file(path: &Path) -> Vec<McpServerEntry> {
    let Ok(content) = std::fs::read_to_string(path) else { return vec![] };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else { return vec![] };
    let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object()) else { return vec![] };

    servers
        .iter()
        .map(|(name, val)| McpServerEntry {
            name: name.clone(),
            command: val.get("command").and_then(|v| v.as_str()).unwrap_or("").into(),
            args: val
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            env: val
                .get("env")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

/// Parse hooks from a JSON file containing `{"hooks": {...}}`
fn parse_hooks_from_file(path: &Path) -> Vec<HookEntry> {
    let Ok(content) = std::fs::read_to_string(path) else { return vec![] };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else { return vec![] };
    let Some(hooks) = val.get("hooks").and_then(|v| v.as_object()) else { return vec![] };

    let mut entries = Vec::new();
    for (event, hook_list) in hooks {
        let Some(arr) = hook_list.as_array() else { continue };
        for hook in arr {
            let matcher = hook.get("matcher").and_then(|v| v.as_str()).map(String::from);
            if let Some(cmds) = hook.get("hooks").and_then(|v| v.as_array()) {
                for cmd in cmds {
                    if let Some(cmd_str) = cmd.as_str() {
                        entries.push(HookEntry {
                            event: event.clone(),
                            matcher: matcher.clone(),
                            command: cmd_str.to_string(),
                        });
                    }
                }
            }
        }
    }
    entries
}

/// Scan a project directory for project-level extensions
pub fn scan_project(project_path: &Path) -> Vec<Extension> {
    let mut extensions = Vec::new();
    let project_path_str = project_path.to_string_lossy().to_string();

    // 1. Scan .claude/skills/ for project skills
    let skills_dir = project_path.join(".claude").join("skills");
    if skills_dir.is_dir() {
        for mut ext in scan_skill_dir(&skills_dir, "project") {
            // Override the ID to include project path for uniqueness
            ext.id = project_stable_id(&ext.name, "skill", &project_path_str);
            ext.source = Source {
                origin: SourceOrigin::Local,
                url: Some(project_path_str.clone()),
                version: None,
                commit_hash: None,
            };
            extensions.push(ext);
        }
    }

    // 2. Scan .mcp.json for project MCP servers
    let mcp_json_path = project_path.join(".mcp.json");
    if mcp_json_path.is_file() {
        let config_created = file_created_time(&mcp_json_path);
        let config_modified = file_modified_time(&mcp_json_path);

        for server in parse_mcp_servers_from_file(&mcp_json_path) {
            let cmd_basename = Path::new(&server.command)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let mut permissions = Vec::new();
            if !server.env.is_empty() {
                permissions.push(Permission::Env { keys: server.env.keys().cloned().collect() });
            }
            permissions.push(Permission::Shell { commands: vec![cmd_basename.clone()] });
            if cmd_basename == "npx" || cmd_basename == "uvx" || server.args.iter().any(|a| a.contains("http")) {
                permissions.push(Permission::Network { domains: vec!["*".into()] });
            }

            let description = if cmd_basename == "npx" || cmd_basename == "uvx" {
                let pkg = server.args.iter().filter(|a| !a.starts_with('-')).last();
                match pkg {
                    Some(p) => format!("Runs {} via {}", p, cmd_basename),
                    None => format!("Runs via {}", cmd_basename),
                }
            } else {
                let args_summary: Vec<&str> = server.args.iter()
                    .filter(|a| !a.starts_with('-'))
                    .map(|s| s.as_str())
                    .take(2)
                    .collect();
                if args_summary.is_empty() {
                    format!("Runs {}", cmd_basename)
                } else {
                    format!("Runs {} {}", cmd_basename, args_summary.join(" "))
                }
            };

            let cat_text = format!("{} {} {} {}",
                description,
                server.command,
                server.args.join(" "),
                server.env.keys().cloned().collect::<Vec<_>>().join(" "),
            );
            let category = infer_category(&server.name, &cat_text);

            extensions.push(Extension {
                id: project_stable_id(&server.name, "mcp", &project_path_str),
                kind: ExtensionKind::Mcp,
                name: server.name,
                description,
                source: Source {
                    origin: SourceOrigin::Local,
                    url: Some(project_path_str.clone()),
                    version: None,
                    commit_hash: None,
                },
                agents: vec!["project".to_string()],
                tags: vec![],
                category,
                permissions,
                enabled: true,
                trust_score: None,
                installed_at: config_created,
                updated_at: config_modified,
            });
        }
    }

    // 3. Scan .claude/settings.json and .claude/settings.local.json for project hooks/MCP
    let settings_files = [
        project_path.join(".claude").join("settings.json"),
        project_path.join(".claude").join("settings.local.json"),
    ];
    for settings_path in &settings_files {
    if settings_path.is_file() {
        let config_created = file_created_time(&settings_path);
        let config_modified = file_modified_time(&settings_path);

        for hook in parse_hooks_from_file(&settings_path) {
            let hook_name = format!("{}:{}:{}", hook.event, hook.matcher.as_deref().unwrap_or("*"), hook.command);
            let cmd_basename = hook.command.split_whitespace().next()
                .map(|c| Path::new(c).file_name().unwrap_or_default().to_string_lossy().to_string())
                .unwrap_or_default();
            let description = format!("Runs `{}` on {} event", hook.command, hook.event);
            let category = infer_category(&hook_name, &hook.command);

            extensions.push(Extension {
                id: project_stable_id(&hook_name, "hook", &project_path_str),
                kind: ExtensionKind::Hook,
                name: hook_name,
                description,
                source: Source {
                    origin: SourceOrigin::Local,
                    url: Some(project_path_str.clone()),
                    version: None,
                    commit_hash: None,
                },
                agents: vec!["project".to_string()],
                tags: vec![],
                category,
                permissions: vec![Permission::Shell { commands: vec![cmd_basename] }],
                enabled: true,
                trust_score: None,
                installed_at: config_created,
                updated_at: config_modified,
            });
        }

        // Also scan .claude/settings.json for MCP servers (same as user-level)
        for server in parse_mcp_servers_from_file(&settings_path) {
            let cmd_basename = Path::new(&server.command)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let mut permissions = Vec::new();
            if !server.env.is_empty() {
                permissions.push(Permission::Env { keys: server.env.keys().cloned().collect() });
            }
            permissions.push(Permission::Shell { commands: vec![cmd_basename.clone()] });
            if cmd_basename == "npx" || cmd_basename == "uvx" || server.args.iter().any(|a| a.contains("http")) {
                permissions.push(Permission::Network { domains: vec!["*".into()] });
            }

            let description = if cmd_basename == "npx" || cmd_basename == "uvx" {
                let pkg = server.args.iter().filter(|a| !a.starts_with('-')).last();
                match pkg {
                    Some(p) => format!("Runs {} via {}", p, cmd_basename),
                    None => format!("Runs via {}", cmd_basename),
                }
            } else {
                let args_summary: Vec<&str> = server.args.iter()
                    .filter(|a| !a.starts_with('-'))
                    .map(|s| s.as_str())
                    .take(2)
                    .collect();
                if args_summary.is_empty() {
                    format!("Runs {}", cmd_basename)
                } else {
                    format!("Runs {} {}", cmd_basename, args_summary.join(" "))
                }
            };

            let cat_text = format!("{} {} {} {}",
                description,
                server.command,
                server.args.join(" "),
                server.env.keys().cloned().collect::<Vec<_>>().join(" "),
            );
            let category = infer_category(&server.name, &cat_text);

            // Use a distinct ID prefix to avoid collision with .mcp.json entries
            let mcp_id_name = format!("settings:{}", server.name);
            extensions.push(Extension {
                id: project_stable_id(&mcp_id_name, "mcp", &project_path_str),
                kind: ExtensionKind::Mcp,
                name: server.name,
                description,
                source: Source {
                    origin: SourceOrigin::Local,
                    url: Some(project_path_str.clone()),
                    version: None,
                    commit_hash: None,
                },
                agents: vec!["project".to_string()],
                tags: vec![],
                category,
                permissions,
                enabled: true,
                trust_score: None,
                installed_at: file_created_time(&settings_path),
                updated_at: file_modified_time(&settings_path),
            });
        }
    } // end for settings_files
    }

    extensions
}

/// Discover projects under a root directory (max depth configurable).
/// A project is a directory containing .claude/skills/, .mcp.json, or .claude/settings.json.
pub fn discover_projects(root: &Path, max_depth: usize) -> Vec<DiscoveredProject> {
    let mut projects = Vec::new();
    discover_projects_recursive(root, max_depth, 0, &mut projects);
    projects
}

fn discover_projects_recursive(
    dir: &Path,
    max_depth: usize,
    current_depth: usize,
    projects: &mut Vec<DiscoveredProject>,
) {
    if current_depth > max_depth {
        return;
    }

    // Check if this directory is a project
    let has_claude_skills = dir.join(".claude").join("skills").is_dir();
    let has_mcp_json = dir.join(".mcp.json").is_file();
    let has_claude_settings = dir.join(".claude").join("settings.json").is_file();

    if has_claude_skills || has_mcp_json || has_claude_settings {
        let name = dir.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        projects.push(DiscoveredProject {
            name,
            path: dir.to_string_lossy().to_string(),
        });
        // Don't recurse into project subdirectories
        return;
    }

    // Recurse into subdirectories
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip hidden directories and common non-project directories
        let dir_name = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if dir_name.starts_with('.')
            || matches!(dir_name.as_str(), "node_modules" | "target" | "__pycache__" | "vendor" | "dist" | "build" | "venv" | ".venv")
        {
            continue;
        }

        discover_projects_recursive(&path, max_depth, current_depth + 1, projects);
    }
}

/// Infer a category based on name and content.
/// For short text (MCP, hooks, plugins) a single keyword match suffices.
/// For long text (skills with full content) requires 2+ matches to avoid false positives.
fn infer_category(name: &str, content: &str) -> Option<String> {
    let text = format!("{} {}", name, content).to_lowercase();
    let rules: &[(&str, &[&str])] = &[
        ("Testing", &["test", "spec", "assert", "mock", "fixture", "coverage", "jest", "pytest", "vitest", "cypress"]),
        ("Security", &["security", "auth", "permission", "encrypt", "credential", "vulnerability", "audit", "pentest", "firewall", "ssl", "tls"]),
        ("DevOps", &["docker", "kubernetes", "k8s", "ci/cd", "deploy", "terraform", "ansible", "nginx", "aws", "gcp", "azure", "infra", "cloudflare", "vercel", "netlify"]),
        ("Data", &["database", "sql", "csv", "json", "data", "analytics", "pandas", "spark", "etl", "migration", "postgres", "mysql", "sqlite", "mongo", "redis", "supabase", "bigquery"]),
        ("Design", &["css", "tailwind", "ui", "ux", "design", "figma", "layout", "responsive", "animation", "svg", "sketch", "canvas"]),
        ("Finance", &["finance", "payment", "stripe", "invoice", "accounting", "tax", "budget", "trading"]),
        ("Education", &["learn", "tutorial", "teach", "course", "quiz", "flashcard", "study", "education"]),
        ("Writing", &["write", "blog", "article", "documentation", "markdown", "content", "copywriting", "grammar", "proofread", "notion"]),
        ("Research", &["research", "paper", "arxiv", "citation", "literature", "survey", "experiment"]),
        ("Productivity", &["todo", "task", "calendar", "schedule", "workflow", "automate", "organize", "template", "slack", "email", "gmail", "trello", "jira", "linear", "asana", "discord"]),
        ("Coding", &["code", "programming", "refactor", "debug", "lint", "compile", "build", "api", "frontend", "backend", "react", "rust", "python", "typescript", "javascript", "github", "gitlab", "git", "npm", "cargo", "pip", "filesystem", "file-system", "editor", "lsp", "server-github"]),
    ];
    // Short text (MCP names, plugin names) → 1 match is enough
    // Long text (skill content) → require 2 matches to avoid false positives
    let threshold = if text.len() < 300 { 1 } else { 2 };
    for (category, keywords) in rules {
        let matches = keywords.iter().filter(|kw| text.contains(**kw)).count();
        if matches >= threshold { return Some(category.to_string()); }
    }
    None
}

// --- Helpers ---

/// Extract the skill name from a SKILL.md file (public for use in commands)
pub fn parse_skill_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_skill_frontmatter(&content).map(|(name, _)| name)
}

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

fn detect_source(path: &Path, agent_managed: bool) -> Source {
    // Check the path itself and all parent directories for .git
    let mut dir = path.to_path_buf();
    // Check current path first (the skill directory itself may be a git clone)
    if dir.join(".git").exists() {
        return Source {
            origin: SourceOrigin::Git,
            url: read_git_remote(&dir),
            version: None,
            commit_hash: read_git_commit_hash(&dir),
        };
    }
    while dir.pop() {
        if dir.join(".git").exists() {
            return Source {
                origin: SourceOrigin::Git,
                url: read_git_remote(&dir),
                version: None,
                commit_hash: read_git_commit_hash(&dir),
            };
        }
    }
    // Extensions found via agent adapters are agent-managed, not unknown
    let origin = if agent_managed { SourceOrigin::Agent } else { SourceOrigin::Local };
    Source { origin, url: None, version: None, commit_hash: None }
}

fn read_git_commit_hash(repo_dir: &Path) -> Option<String> {
    let head = std::fs::read_to_string(repo_dir.join(".git/HEAD")).ok()?;
    let head = head.trim();
    if let Some(ref_path) = head.strip_prefix("ref: ") {
        // HEAD points to a branch ref — read the actual commit hash
        std::fs::read_to_string(repo_dir.join(".git").join(ref_path))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        // Detached HEAD — the hash is directly in HEAD
        Some(head.to_string()).filter(|s| !s.is_empty())
    }
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

    #[test]
    fn test_infer_category_security() {
        let cat = infer_category("auth-checker", "Check security permissions for the auth module");
        assert_eq!(cat, Some("Security".to_string()));
    }

    #[test]
    fn test_infer_category_testing() {
        let cat = infer_category("test-runner", "Run jest tests and check assert results");
        assert_eq!(cat, Some("Testing".to_string()));
    }

    #[test]
    fn test_infer_category_coding() {
        let cat = infer_category("refactor-helper", "Helps refactor code and debug issues");
        assert_eq!(cat, Some("Coding".to_string()));
    }

    #[test]
    fn test_infer_category_none() {
        let cat = infer_category("my-tool", "A generic tool that does stuff");
        assert_eq!(cat, None);
    }

    #[test]
    fn test_infer_category_devops() {
        let cat = infer_category("deploy-tool", "Deploy to kubernetes using docker containers");
        assert_eq!(cat, Some("DevOps".to_string()));
    }

    #[test]
    fn test_scan_project_skills() {
        let dir = TempDir::new().unwrap();
        let skills_dir = dir.path().join(".claude").join("skills");
        std::fs::create_dir_all(skills_dir.join("my-skill")).unwrap();
        std::fs::write(
            skills_dir.join("my-skill").join("SKILL.md"),
            "---\nname: my-skill\ndescription: A project skill\n---\nDo things.",
        ).unwrap();

        let extensions = scan_project(dir.path());
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "my-skill");
        assert_eq!(extensions[0].kind, ExtensionKind::Skill);
        assert_eq!(extensions[0].agents, vec!["project"]);
        assert_eq!(extensions[0].source.origin, SourceOrigin::Local);
        assert_eq!(extensions[0].source.url, Some(dir.path().to_string_lossy().to_string()));
    }

    #[test]
    fn test_scan_project_mcp_json() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"local-server":{"command":"node","args":["server.js"],"env":{"PORT":"3000"}}}}"#,
        ).unwrap();

        let extensions = scan_project(dir.path());
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "local-server");
        assert_eq!(extensions[0].kind, ExtensionKind::Mcp);
        assert_eq!(extensions[0].agents, vec!["project"]);
        assert_eq!(extensions[0].source.origin, SourceOrigin::Local);
    }

    #[test]
    fn test_scan_project_hooks() {
        let dir = TempDir::new().unwrap();
        let claude_dir = dir.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo project-hook"]}]}}"#,
        ).unwrap();

        let extensions = scan_project(dir.path());
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].kind, ExtensionKind::Hook);
        assert_eq!(extensions[0].agents, vec!["project"]);
        assert!(extensions[0].name.contains("PreToolUse"));
    }

    #[test]
    fn test_scan_project_combined() {
        let dir = TempDir::new().unwrap();

        // Skills
        let skills_dir = dir.path().join(".claude").join("skills");
        std::fs::create_dir_all(skills_dir.join("proj-skill")).unwrap();
        std::fs::write(
            skills_dir.join("proj-skill").join("SKILL.md"),
            "---\nname: proj-skill\ndescription: desc\n---\ncontent",
        ).unwrap();

        // MCP
        std::fs::write(
            dir.path().join(".mcp.json"),
            r#"{"mcpServers":{"db":{"command":"sqlite-mcp","args":[],"env":{}}}}"#,
        ).unwrap();

        // Hooks in settings
        std::fs::write(
            dir.path().join(".claude").join("settings.json"),
            r#"{"hooks":{"PostToolUse":[{"hooks":["echo done"]}]}}"#,
        ).unwrap();

        let extensions = scan_project(dir.path());
        assert_eq!(extensions.len(), 3);
        let kinds: Vec<ExtensionKind> = extensions.iter().map(|e| e.kind).collect();
        assert!(kinds.contains(&ExtensionKind::Skill));
        assert!(kinds.contains(&ExtensionKind::Mcp));
        assert!(kinds.contains(&ExtensionKind::Hook));
    }

    #[test]
    fn test_project_ids_dont_collide_with_user_level() {
        // Same skill name should produce different IDs for project vs user-level
        let user_id = stable_id("my-skill", "skill", "claude");
        let project_id = project_stable_id("my-skill", "skill", "/tmp/my-project");
        assert_ne!(user_id, project_id);
    }

    #[test]
    fn test_discover_projects() {
        let root = TempDir::new().unwrap();

        // Project with .mcp.json
        let proj1 = root.path().join("project-a");
        std::fs::create_dir_all(&proj1).unwrap();
        std::fs::write(proj1.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();

        // Project with .claude/skills/
        let proj2 = root.path().join("project-b");
        std::fs::create_dir_all(proj2.join(".claude").join("skills")).unwrap();

        // Not a project
        let non_proj = root.path().join("not-a-project");
        std::fs::create_dir_all(&non_proj).unwrap();

        let discovered = discover_projects(root.path(), 4);
        assert_eq!(discovered.len(), 2);
        let names: Vec<&str> = discovered.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"project-a"));
        assert!(names.contains(&"project-b"));
    }

    #[test]
    fn test_discover_projects_skips_hidden_and_node_modules() {
        let root = TempDir::new().unwrap();

        // Hidden directory with project markers - should be skipped
        let hidden = root.path().join(".hidden-project");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();

        // node_modules with project markers - should be skipped
        let node_mod = root.path().join("node_modules");
        std::fs::create_dir_all(&node_mod).unwrap();
        std::fs::write(node_mod.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();

        let discovered = discover_projects(root.path(), 4);
        assert_eq!(discovered.len(), 0);
    }

    #[test]
    fn test_discover_projects_nested() {
        let root = TempDir::new().unwrap();

        // Nested project
        let nested = root.path().join("workspace").join("apps").join("my-app");
        std::fs::create_dir_all(nested.join(".claude").join("skills")).unwrap();

        let discovered = discover_projects(root.path(), 4);
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].name, "my-app");
    }

    #[test]
    fn test_discover_projects_respects_max_depth() {
        let root = TempDir::new().unwrap();

        // Project at depth 2
        let deep = root.path().join("a").join("b").join("c");
        std::fs::create_dir_all(deep.join(".claude").join("skills")).unwrap();

        // max_depth=1 should miss it
        let shallow = discover_projects(root.path(), 1);
        assert_eq!(shallow.len(), 0);

        // max_depth=3 should find it
        let deep_result = discover_projects(root.path(), 3);
        assert_eq!(deep_result.len(), 1);
    }
}
