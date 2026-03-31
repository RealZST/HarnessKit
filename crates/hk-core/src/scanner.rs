use crate::adapter::{AgentAdapter, HookEntry, McpServerEntry};
use crate::models::*;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

struct KnownCli {
    binary_name: &'static str,
    display_name: &'static str,
    api_domains: &'static [&'static str],
    credentials_path: Option<&'static str>,
}

static KNOWN_CLIS: &[KnownCli] = &[
    KnownCli {
        binary_name: "wecom-cli",
        display_name: "WeChat Work CLI",
        api_domains: &["qyapi.weixin.qq.com"],
        credentials_path: Some("~/.config/wecom/bot.enc"),
    },
    KnownCli {
        binary_name: "lark-cli",
        display_name: "Lark / Feishu CLI",
        api_domains: &["open.feishu.cn", "open.larksuite.com"],
        credentials_path: Some("~/.config/lark/credentials"),
    },
    KnownCli {
        binary_name: "dws",
        display_name: "DingTalk Workspace CLI",
        api_domains: &["api.dingtalk.com"],
        credentials_path: Some("~/.config/dws/auth.json"),
    },
    KnownCli {
        binary_name: "meitu",
        display_name: "Meitu CLI",
        api_domains: &["openapi.mtlab.meitu.com"],
        credentials_path: Some("~/.meitu/credentials.json"),
    },
    KnownCli {
        binary_name: "officecli",
        display_name: "OfficeCLI",
        api_domains: &[],
        credentials_path: None,
    },
];

/// FNV-1a 64-bit hash — deterministic across Rust versions (unlike DefaultHasher).
pub fn fnv1a(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Public wrapper for stable_id, used by other modules for ID matching.
pub fn stable_id_for(name: &str, kind: &str, agent: &str) -> String {
    stable_id(name, kind, agent)
}

/// Generate a deterministic ID from name + kind + agent so re-scans produce the same ID
fn stable_id(name: &str, kind: &str, agent: &str) -> String {
    let key = format!("{}:{}:{}", kind, agent, name);
    format!("{:016x}", fnv1a(key.as_bytes()))
}

/// Generate a deterministic ID for CLI extensions based on binary name
fn cli_stable_id(binary_name: &str) -> String {
    let key = format!("cli::{}", binary_name);
    format!("{:016x}", fnv1a(key.as_bytes()))
}

/// Scan a skill directory and return Extension entries
pub fn scan_skill_dir(dir: &Path, agent_name: &str) -> Vec<Extension> {
    let mut extensions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return extensions };

    for entry in entries.flatten() {
        let path = entry.path();
        // Skills can be either: a directory containing SKILL.md (or SKILL.md.disabled), or a standalone .md file
        let (skill_file, is_disabled) = if path.is_dir() {
            let enabled_file = path.join("SKILL.md");
            let disabled_file = path.join("SKILL.md.disabled");
            if enabled_file.exists() {
                (enabled_file, false)
            } else if disabled_file.exists() {
                (disabled_file, true)
            } else {
                continue;
            }
        } else if path.extension().is_some_and(|ext| ext == "md") {
            (path.clone(), false)
        } else {
            continue;
        };
        // Capture atime BEFORE reading content (read_to_string updates atime)
        let last_used = skill_last_used_at(&skill_file);
        let Ok(content) = std::fs::read_to_string(&skill_file) else { continue; };

        let (name, description, _requires_bins) = parse_skill_frontmatter(&content)
            .unwrap_or_else(|| {
                let name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                (name, String::new(), vec![])
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
            enabled: !is_disabled,
            trust_score: None,
            installed_at: file_created_time(&path),
            updated_at: file_modified_time(&path),
            last_used_at: last_used,
            source_path: Some(if is_disabled {
                path.join("SKILL.md").to_string_lossy().to_string()
            } else {
                skill_file.to_string_lossy().to_string()
            }),
            cli_parent_id: None,
            cli_meta: None,
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
        if cmd_basename == "npx" || cmd_basename == "uvx" {
            permissions.push(Permission::Network { domains: vec!["*".into()] });
        } else {
            let domains: Vec<String> = server.args.iter()
                .flat_map(|a| SKILL_URL_DOMAINS.captures_iter(a).map(|c| c[1].to_string()))
                .collect::<HashSet<_>>().into_iter().collect();
            if !domains.is_empty() {
                permissions.push(Permission::Network { domains });
            }
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
            last_used_at: None,
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
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
            permissions: vec![Permission::Shell {
                commands: if hook.command == cmd_basename {
                    vec![cmd_basename]
                } else {
                    vec![hook.command.clone(), cmd_basename]
                },
            }],
            enabled: true,
            trust_score: None,
            installed_at: config_created,
            updated_at: config_modified,
            last_used_at: None,
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
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
            last_used_at: None,
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
        }
    }).collect()
}

/// Run `which` to find a binary's absolute path
fn which_binary(name: &str) -> Option<String> {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
}

/// Run `<binary> --version` and extract a version number via regex
fn get_binary_version(name: &str) -> Option<String> {
    static VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(\d+\.\d+(?:\.\d+)?)").unwrap()
    });
    let output = std::process::Command::new(name)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let text = if text.trim().is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        text
    };
    VERSION_RE.captures(&text).map(|c| c[1].to_string())
}

/// Detect the install method from the binary path
fn detect_install_method(path: &str) -> Option<String> {
    let lower = path.to_lowercase();
    if lower.contains("/node_modules/") || lower.contains("/.npm/") || lower.contains("/npx/") {
        Some("npm".into())
    } else if lower.contains("/.cargo/") {
        Some("cargo".into())
    } else if lower.contains("/pip") || lower.contains("/python") || lower.contains("/site-packages/") {
        Some("pip".into())
    } else if lower.contains("/homebrew/") || lower.contains("/cellar/") || lower.contains("/linuxbrew/") {
        Some("brew".into())
    } else {
        None
    }
}

/// Scan for CLI binaries referenced by skills and from the KNOWN_CLIS registry.
///
/// Returns a tuple of:
/// - CLI Extension entries
/// - Map from CLI extension ID -> list of skill extension IDs that depend on it
fn scan_cli_binaries(existing_extensions: &[Extension]) -> (Vec<Extension>, HashMap<String, Vec<String>>) {
    let mut candidate_bins: HashSet<String> = HashSet::new();
    // Map: binary_name -> Vec<skill extension id>
    let mut bin_to_skills: HashMap<String, Vec<String>> = HashMap::new();

    // 1. Iterate scanned skills, read their SKILL.md content to extract requires_bins
    for ext in existing_extensions {
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        if let Some(ref path_str) = ext.source_path {
            if let Ok(content) = std::fs::read_to_string(path_str) {
                if let Some((_, _, requires_bins)) = parse_skill_frontmatter(&content) {
                    for bin in requires_bins {
                        candidate_bins.insert(bin.clone());
                        bin_to_skills.entry(bin).or_default().push(ext.id.clone());
                    }
                }
            }
        }
    }

    // 2. Add all KNOWN_CLIS binary names to candidate set
    for known in KNOWN_CLIS {
        candidate_bins.insert(known.binary_name.to_string());
    }

    let mut cli_extensions = Vec::new();
    let mut child_links: HashMap<String, Vec<String>> = HashMap::new();
    let now = Utc::now();

    // 3. For each candidate, check if it exists
    for bin_name in &candidate_bins {
        let bin_path = which_binary(bin_name);
        // Skip if binary is not installed — we only track CLIs that are actually present
        if bin_path.is_none() {
            continue;
        }
        let known = KNOWN_CLIS.iter().find(|k| k.binary_name == bin_name.as_str());

        let version = bin_path.as_ref().and_then(|_| get_binary_version(bin_name));
        let install_method = bin_path.as_ref().and_then(|p| detect_install_method(p));

        let display_name = known.map(|k| k.display_name.to_string())
            .unwrap_or_else(|| bin_name.clone());
        let api_domains: Vec<String> = known
            .map(|k| k.api_domains.iter().map(|d| d.to_string()).collect())
            .unwrap_or_default();
        let credentials_path = known.and_then(|k| k.credentials_path.map(|p| p.to_string()));

        // 4. Auto-derive permissions from CliMeta
        let mut permissions = Vec::new();
        if !api_domains.is_empty() {
            permissions.push(Permission::Network { domains: api_domains.clone() });
        }
        if credentials_path.is_some() {
            permissions.push(Permission::FileSystem {
                paths: credentials_path.iter().cloned().collect(),
            });
        }
        if bin_path.is_some() {
            permissions.push(Permission::Shell { commands: vec![bin_name.clone()] });
        }

        let cli_id = cli_stable_id(bin_name);

        let description = if let Some(ref v) = version {
            format!("{} v{}", display_name, v)
        } else if bin_path.is_some() {
            format!("{} (installed)", display_name)
        } else {
            format!("{} (not installed)", display_name)
        };

        // 5. Build child_links: CLI ID -> skill IDs
        if let Some(skill_ids) = bin_to_skills.get(bin_name.as_str()) {
            child_links.entry(cli_id.clone()).or_default().extend(skill_ids.clone());
        }

        let source = Source {
            origin: if bin_path.is_some() { SourceOrigin::Local } else { SourceOrigin::Registry },
            url: None,
            version: version.clone(),
            commit_hash: None,
        };

        cli_extensions.push(Extension {
            id: cli_id,
            kind: ExtensionKind::Cli,
            name: display_name,
            description,
            source,
            agents: vec![],
            tags: vec![],
            category: None,
            permissions,
            enabled: bin_path.is_some(),
            trust_score: None,
            installed_at: now,
            updated_at: now,
            last_used_at: None,
            source_path: bin_path.clone(),
            cli_parent_id: None,
            cli_meta: Some(CliMeta {
                binary_name: bin_name.clone(),
                binary_path: bin_path,
                install_method,
                credentials_path,
                version,
                api_domains,
            }),
        });
    }

    (cli_extensions, child_links)
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

    // CLI scanning: discover CLIs from skills' requires.bins + KNOWN_CLIS
    let (cli_extensions, child_links) = scan_cli_binaries(&all);

    // Back-fill cli_parent_id on matching skills
    for ext in &mut all {
        if ext.kind == ExtensionKind::Skill {
            for (cli_id, skill_ids) in &child_links {
                if skill_ids.contains(&ext.id) {
                    ext.cli_parent_id = Some(cli_id.clone());
                    break;
                }
            }
        }
    }

    all.extend(cli_extensions);
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
            if cmd_basename == "npx" || cmd_basename == "uvx" {
                permissions.push(Permission::Network { domains: vec!["*".into()] });
            } else {
                let domains: Vec<String> = server.args.iter()
                    .flat_map(|a| SKILL_URL_DOMAINS.captures_iter(a).map(|c| c[1].to_string()))
                    .collect::<HashSet<_>>().into_iter().collect();
                if !domains.is_empty() {
                    permissions.push(Permission::Network { domains });
                }
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
                last_used_at: None,
                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
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
                permissions: vec![Permission::Shell {
                    commands: if hook.command == cmd_basename {
                        vec![cmd_basename]
                    } else {
                        vec![hook.command.clone(), cmd_basename]
                    },
                }],
                enabled: true,
                trust_score: None,
                installed_at: config_created,
                updated_at: config_modified,
                last_used_at: None,
                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
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
            if cmd_basename == "npx" || cmd_basename == "uvx" {
                permissions.push(Permission::Network { domains: vec!["*".into()] });
            } else {
                let domains: Vec<String> = server.args.iter()
                    .flat_map(|a| SKILL_URL_DOMAINS.captures_iter(a).map(|c| c[1].to_string()))
                    .collect::<HashSet<_>>().into_iter().collect();
                if !domains.is_empty() {
                    permissions.push(Permission::Network { domains });
                }
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
                last_used_at: None,
                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
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
    parse_skill_frontmatter(&content).map(|(name, _, _)| name)
}

pub fn parse_skill_frontmatter(content: &str) -> Option<(String, String, Vec<String>)> {
    if !content.starts_with("---") { return None; }
    let rest = &content[3..];
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    let mut name = None;
    let mut description = None;
    let mut bins: Vec<String> = Vec::new();

    // Track parsing state for block-style YAML arrays under bins:
    let mut in_bins_block = false;
    // Track nesting: we accept bins: at top level OR under metadata: -> requires: -> bins:
    let mut in_metadata = false;
    let mut in_requires = false;

    for line in frontmatter.lines() {
        let trimmed = line.trim();

        // Top-level fields
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().to_string());
            in_bins_block = false;
            continue;
        }
        if let Some(val) = trimmed.strip_prefix("description:") {
            description = Some(val.trim().to_string());
            in_bins_block = false;
            continue;
        }

        // Track metadata: / requires: nesting
        if trimmed == "metadata:" {
            in_metadata = true;
            in_bins_block = false;
            continue;
        }
        if in_metadata && trimmed == "requires:" {
            in_requires = true;
            in_bins_block = false;
            continue;
        }

        // bins: field — either top-level or nested under metadata: -> requires:
        let is_bins_line = if in_metadata && in_requires {
            trimmed.starts_with("bins:")
        } else {
            line.starts_with("bins:") || trimmed.starts_with("bins:")
        };

        if is_bins_line {
            let val = trimmed.strip_prefix("bins:").unwrap_or("").trim();
            if val.is_empty() {
                // Block-style array follows
                in_bins_block = true;
            } else {
                // Inline array: bins: ["wecom-cli", "lark-cli"]
                in_bins_block = false;
                let inner = val.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let b = item.trim().trim_matches('"').trim_matches('\'').trim();
                    if !b.is_empty() {
                        bins.push(b.to_string());
                    }
                }
            }
            continue;
        }

        // Block-style array items
        if in_bins_block {
            if let Some(item) = trimmed.strip_prefix("- ") {
                let b = item.trim().trim_matches('"').trim_matches('\'').trim();
                if !b.is_empty() {
                    bins.push(b.to_string());
                }
            } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Non-continuation line ends the block
                in_bins_block = false;
            }
        }
    }

    Some((name?, description.unwrap_or_default(), bins))
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

static SKILL_SENSITIVE_PATHS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:~|/(?:etc|home/\w+))/[\w.\-/]+").unwrap()
});

static SKILL_URL_DOMAINS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://([\w.\-]+)").unwrap()
});

static SKILL_SHELL_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```(?:bash|shell|sh|zsh)\s*\n(.*?)```").unwrap()
});

static SKILL_DB_ENGINES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(postgres(?:ql)?|mysql|mariadb|sqlite|mongodb|redis)\b").unwrap()
});

fn infer_skill_permissions(content: &str) -> Vec<Permission> {
    let mut perms = Vec::new();
    let lower = content.to_lowercase();

    if lower.contains("file") || lower.contains("read") || lower.contains("write") || lower.contains("path") {
        let paths: Vec<String> = SKILL_SENSITIVE_PATHS.find_iter(content)
            .map(|m| m.as_str().to_string())
            .collect::<HashSet<_>>().into_iter().collect();
        perms.push(Permission::FileSystem { paths });
    }
    if lower.contains("http") || lower.contains("api") || lower.contains("fetch") || lower.contains("url") {
        let domains: Vec<String> = SKILL_URL_DOMAINS.captures_iter(content)
            .map(|c| c[1].to_string())
            .collect::<HashSet<_>>().into_iter().collect();
        perms.push(Permission::Network { domains });
    }
    if lower.contains("bash") || lower.contains("shell") || lower.contains("command") || lower.contains("exec") {
        let mut cmds = HashSet::new();
        for block_cap in SKILL_SHELL_BLOCK.captures_iter(content) {
            let body = &block_cap[1];
            for line in body.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some(token) = trimmed.split_whitespace().next() {
                    let basename = Path::new(token)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if !basename.is_empty() {
                        cmds.insert(basename);
                    }
                }
            }
        }
        perms.push(Permission::Shell { commands: cmds.into_iter().collect() });
    }
    if lower.contains("database") || lower.contains("sql") || lower.contains("postgres") || lower.contains("mysql") {
        let engines: Vec<String> = SKILL_DB_ENGINES.captures_iter(&lower)
            .map(|c| c[1].to_string())
            .collect::<HashSet<_>>().into_iter().collect();
        perms.push(Permission::Database { engines });
    }
    perms
}

/// Read the last access time of a skill file to determine when it was last used by an agent.
/// Must be called BEFORE reading file content (which updates atime).
/// Returns `None` if atime matches creation time at second precision (never accessed after install).
fn skill_last_used_at(path: &Path) -> Option<chrono::DateTime<Utc>> {
    let meta = std::fs::metadata(path).ok()?;
    let atime = meta.accessed().ok()?;
    let ctime = meta.created().ok()?;
    let atime_sec = chrono::DateTime::<Utc>::from(atime).timestamp();
    let ctime_sec = chrono::DateTime::<Utc>::from(ctime).timestamp();
    if atime_sec == ctime_sec {
        None
    } else {
        Some(chrono::DateTime::<Utc>::from(atime))
    }
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

/// Scan an agent adapter for config files (rules, memory, settings, ignore).
/// `projects` is a list of (project_name, project_path) pairs.
pub fn scan_agent_configs(
    adapter: &dyn AgentAdapter,
    projects: &[(String, String)],
) -> Vec<AgentConfigFile> {
    let mut configs = Vec::new();

    // --- Global files ---
    let global_groups: [(ConfigCategory, Vec<std::path::PathBuf>); 3] = [
        (ConfigCategory::Rules, adapter.global_rules_files()),
        (ConfigCategory::Memory, adapter.global_memory_files()),
        (ConfigCategory::Settings, adapter.global_settings_files()),
    ];

    for (category, paths) in &global_groups {
        for path in paths {
            if let Some(cf) = stat_config_file(path, adapter.name(), *category, ConfigScope::Global) {
                configs.push(cf);
            }
        }
    }

    // --- Project files ---
    let project_groups: [(ConfigCategory, Vec<String>); 4] = [
        (ConfigCategory::Rules, adapter.project_rules_patterns()),
        (ConfigCategory::Memory, adapter.project_memory_patterns()),
        (ConfigCategory::Settings, adapter.project_settings_patterns()),
        (ConfigCategory::Ignore, adapter.project_ignore_patterns()),
    ];

    for (project_name, project_path) in projects {
        let project_root = std::path::Path::new(project_path);
        if !project_root.is_dir() { continue; }

        let scope = ConfigScope::Project {
            name: project_name.clone(),
            path: project_path.clone(),
        };

        for (category, patterns) in &project_groups {
            for pattern in patterns {
                let resolved = resolve_pattern(project_root, pattern);
                for path in resolved {
                    if let Some(cf) = stat_config_file(&path, adapter.name(), *category, scope.clone()) {
                        configs.push(cf);
                    }
                }
            }
        }
    }

    // Sort by category order, then by scope (global first), then by file name
    configs.sort_by(|a, b| {
        a.category.order().cmp(&b.category.order())
            .then_with(|| {
                let a_is_global = matches!(a.scope, ConfigScope::Global);
                let b_is_global = matches!(b.scope, ConfigScope::Global);
                b_is_global.cmp(&a_is_global)
            })
            .then_with(|| a.file_name.cmp(&b.file_name))
    });

    configs
}

/// Stat a file and build an AgentConfigFile if it exists.
fn stat_config_file(
    path: &std::path::Path,
    agent: &str,
    category: ConfigCategory,
    scope: ConfigScope,
) -> Option<AgentConfigFile> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() { return None; }

    let modified_at = metadata.modified().ok().map(|t| {
        let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0).unwrap_or_default()
    });

    Some(AgentConfigFile {
        path: path.to_string_lossy().to_string(),
        agent: agent.to_string(),
        category,
        scope,
        file_name: path.file_name()?.to_string_lossy().to_string(),
        size_bytes: metadata.len(),
        modified_at,
    })
}

/// Resolve a pattern (possibly with glob `*`) against a project root.
fn resolve_pattern(root: &std::path::Path, pattern: &str) -> Vec<std::path::PathBuf> {
    if pattern.contains('*') {
        let full_pattern = root.join(pattern).to_string_lossy().to_string();
        glob::glob(&full_pattern)
            .map(|paths| paths.filter_map(|p| p.ok()).collect())
            .unwrap_or_default()
    } else {
        let path = root.join(pattern);
        if path.exists() { vec![path] } else { vec![] }
    }
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

    #[test]
    fn test_scan_discovers_disabled_skills() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md.disabled"),
            "---\nname: my-skill\ndescription: A test skill\n---\nContent here",
        ).unwrap();

        let extensions = super::scan_skill_dir(dir.path(), "claude");
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "my-skill");
        assert!(!extensions[0].enabled, "Disabled skill should have enabled=false");
    }

    #[test]
    fn test_disabled_skill_same_id_as_enabled() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();

        // Scan as enabled
        std::fs::write(skill_dir.join("SKILL.md"), "---\nname: my-skill\n---\n").unwrap();
        let enabled_exts = super::scan_skill_dir(dir.path(), "claude");
        let enabled_id = enabled_exts[0].id.clone();

        // Rename to disabled
        std::fs::rename(skill_dir.join("SKILL.md"), skill_dir.join("SKILL.md.disabled")).unwrap();
        let disabled_exts = super::scan_skill_dir(dir.path(), "claude");
        let disabled_id = disabled_exts[0].id.clone();

        assert_eq!(enabled_id, disabled_id, "Same skill should produce same ID whether enabled or disabled");
    }

    #[test]
    fn test_disabled_skill_source_path_is_enabled_path() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md.disabled"),
            "---\nname: my-skill\n---\n",
        ).unwrap();

        let extensions = super::scan_skill_dir(dir.path(), "claude");
        assert_eq!(extensions.len(), 1);
        let source_path = extensions[0].source_path.as_ref().unwrap();
        assert!(source_path.ends_with("SKILL.md"), "source_path should point to SKILL.md, not SKILL.md.disabled, got: {}", source_path);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use crate::adapter::claude::ClaudeAdapter;
    use std::fs;

    #[test]
    fn test_scan_agent_configs_global_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(claude_dir.join("CLAUDE.md"), "# Rules\nUse Rust.").unwrap();
        fs::write(claude_dir.join("settings.json"), "{}").unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let configs = scan_agent_configs(&adapter, &[]);

        let rules: Vec<_> = configs.iter().filter(|c| c.category == ConfigCategory::Rules).collect();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].file_name, "CLAUDE.md");
        assert!(matches!(rules[0].scope, ConfigScope::Global));

        let settings: Vec<_> = configs.iter().filter(|c| c.category == ConfigCategory::Settings).collect();
        assert_eq!(settings.len(), 1);
    }

    #[test]
    fn test_scan_agent_configs_project_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let project = tmp.path().join("myproject");
        fs::create_dir_all(home.join(".claude")).unwrap();
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::write(project.join("CLAUDE.md"), "# Project rules").unwrap();
        fs::write(project.join(".claude").join("settings.json"), "{}").unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let projects = vec![("myproject".to_string(), project.to_string_lossy().to_string())];
        let configs = scan_agent_configs(&adapter, &projects);

        let project_rules: Vec<_> = configs.iter()
            .filter(|c| c.category == ConfigCategory::Rules && matches!(&c.scope, ConfigScope::Project { .. }))
            .collect();
        assert_eq!(project_rules.len(), 1);

        // Claude Code does not have .claudeignore
        let ignores: Vec<_> = configs.iter().filter(|c| c.category == ConfigCategory::Ignore).collect();
        assert_eq!(ignores.len(), 0);
    }

    #[test]
    fn test_scan_agent_configs_skips_missing_files() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        fs::create_dir_all(home.join(".claude")).unwrap();

        let adapter = ClaudeAdapter::with_home(home.to_path_buf());
        let configs = scan_agent_configs(&adapter, &[]);
        assert!(configs.is_empty());
    }

    #[test]
    fn test_parse_skill_frontmatter_with_bins_inline() {
        let content = "---\nname: wecomcli-send\ndescription: Send messages\nbins: [\"wecom-cli\"]\n---\nBody";
        let (name, desc, bins) = parse_skill_frontmatter(content).unwrap();
        assert_eq!(name, "wecomcli-send");
        assert_eq!(desc, "Send messages");
        assert_eq!(bins, vec!["wecom-cli"]);
    }

    #[test]
    fn test_parse_skill_frontmatter_with_bins_block() {
        let content = "---\nname: lark-cal\ndescription: Calendar\nbins:\n  - \"lark-cli\"\n---\nBody";
        let (_, _, bins) = parse_skill_frontmatter(content).unwrap();
        assert_eq!(bins, vec!["lark-cli"]);
    }

    #[test]
    fn test_parse_skill_frontmatter_no_bins() {
        let content = "---\nname: plain-skill\ndescription: No CLI\n---\nBody";
        let (_, _, bins) = parse_skill_frontmatter(content).unwrap();
        assert!(bins.is_empty());
    }

    #[test]
    fn test_cli_stable_id_deterministic() {
        let id1 = cli_stable_id("wecom-cli");
        let id2 = cli_stable_id("wecom-cli");
        let id3 = cli_stable_id("lark-cli");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_detect_install_method() {
        assert_eq!(detect_install_method("/usr/local/lib/node_modules/.bin/wecom-cli"), Some("npm".into()));
        assert_eq!(detect_install_method("/Users/test/.cargo/bin/tool"), Some("cargo".into()));
        assert_eq!(detect_install_method("/usr/local/bin/tool"), None);
    }
}
