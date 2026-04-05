use crate::adapter::AgentAdapter;
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
    repo_url: Option<&'static str>,
}

static KNOWN_CLIS: &[KnownCli] = &[
    KnownCli {
        binary_name: "wecom-cli",
        display_name: "WeChat Work CLI",
        api_domains: &["qyapi.weixin.qq.com"],
        credentials_path: Some("~/.config/wecom/bot.enc"),
        repo_url: None,
    },
    KnownCli {
        binary_name: "lark-cli",
        display_name: "Lark / Feishu CLI",
        api_domains: &["open.feishu.cn", "open.larksuite.com"],
        credentials_path: Some("~/.config/lark/credentials"),
        repo_url: None,
    },
    KnownCli {
        binary_name: "dws",
        display_name: "DingTalk Workspace CLI",
        api_domains: &["api.dingtalk.com"],
        credentials_path: Some("~/.config/dws/auth.json"),
        repo_url: None,
    },
    KnownCli {
        binary_name: "meitu",
        display_name: "Meitu CLI",
        api_domains: &["openapi.mtlab.meitu.com"],
        credentials_path: Some("~/.meitu/credentials.json"),
        repo_url: None,
    },
    KnownCli {
        binary_name: "officecli",
        display_name: "OfficeCLI",
        api_domains: &[],
        credentials_path: None,
        repo_url: None,
    },
    KnownCli {
        binary_name: "notion-cli",
        display_name: "Notion CLI",
        api_domains: &["mcp.notion.com"],
        credentials_path: Some("~/.config/notion-cli/token.json"),
        repo_url: None,
    },
    KnownCli {
        binary_name: "opencli",
        display_name: "OpenCLI",
        api_domains: &[],
        credentials_path: None,
        repo_url: None,
    },
    KnownCli {
        binary_name: "cli-anything",
        display_name: "CLI-Anything",
        api_domains: &[],
        credentials_path: None,
        repo_url: None,
    },
];

/// FNV-1a 64-bit hash — deterministic across Rust versions (unlike DefaultHasher).
/// NOTE: FNV-1a is not collision-resistant. With a very large number of extensions,
/// ID collisions are theoretically possible. Consider SHA-256 truncated if this
/// becomes an issue. The database UPSERT on primary key mitigates silent data loss.
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

/// Scan a skill directory and return Extension entries.
pub fn scan_skill_dir(dir: &Path, agent_name: &str) -> Vec<Extension> {
    let mut extensions = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return extensions;
    };

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
        let Ok(content) = std::fs::read_to_string(&skill_file) else {
            continue;
        };

        let (name, description, _requires_bins) =
            parse_skill_frontmatter(&content).unwrap_or_else(|| {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                (name, String::new(), vec![])
            });

        let source = detect_source(&path, true);
        let pack = source.url.as_deref().and_then(extract_pack_from_url);
        extensions.push(Extension {
            id: stable_id(&name, "skill", agent_name),
            kind: ExtensionKind::Skill,
            name,
            description,
            source,
            agents: vec![agent_name.to_string()],
            tags: vec![],
            pack,
            permissions: infer_skill_permissions(&content),
            enabled: !is_disabled,
            trust_score: None,
            installed_at: file_created_time(&path),
            updated_at: file_modified_time(&path),

            source_path: Some(if is_disabled {
                path.join("SKILL.md").to_string_lossy().to_string()
            } else {
                skill_file.to_string_lossy().to_string()
            }),
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
        });
    }
    extensions
}

/// Scan MCP servers from an agent adapter
pub fn scan_mcp_servers(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    let config_path = adapter.mcp_config_path();
    let config_created = file_created_time(&config_path);
    let config_modified = file_modified_time(&config_path);

    adapter
        .read_mcp_servers()
        .into_iter()
        .map(|server| {
            let cmd_basename = Path::new(&server.command)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let mut permissions = Vec::new();
            if !server.env.is_empty() {
                permissions.push(Permission::Env {
                    keys: server.env.keys().cloned().collect(),
                });
            }
            permissions.push(Permission::Shell {
                commands: vec![cmd_basename.clone()],
            });
            if cmd_basename == "npx" || cmd_basename == "uvx" {
                permissions.push(Permission::Network {
                    domains: vec!["*".into()],
                });
            } else {
                let domains: Vec<String> = server
                    .args
                    .iter()
                    .flat_map(|a| SKILL_URL_DOMAINS.captures_iter(a).map(|c| c[1].to_string()))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                if !domains.is_empty() {
                    permissions.push(Permission::Network { domains });
                }
            }

            // Build a human-readable description from the command
            let description = if cmd_basename == "npx" || cmd_basename == "uvx" {
                // Show the package name (usually the last meaningful arg)
                let pkg = server.args.iter().rfind(|a| !a.starts_with('-'));
                match pkg {
                    Some(p) => format!("Runs {} via {}", p, cmd_basename),
                    None => format!("Runs via {}", cmd_basename),
                }
            } else {
                let args_summary: Vec<&str> = server
                    .args
                    .iter()
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

            Extension {
                id: stable_id(&server.name, "mcp", adapter.name()),
                kind: ExtensionKind::Mcp,
                name: server.name,
                description,
                source: Source {
                    origin: SourceOrigin::Agent,
                    url: None,
                    version: None,
                    commit_hash: None,
                },
                agents: vec![adapter.name().to_string()],
                tags: vec![],
                pack: None,
                permissions,
                enabled: true,
                trust_score: None,
                installed_at: config_created,
                updated_at: config_modified,

                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
                install_meta: None,
            }
        })
        .collect()
}

/// Scan hooks from an agent adapter
pub fn scan_hooks(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    let config_path = adapter.hook_config_path();
    let config_created = file_created_time(&config_path);
    let config_modified = file_modified_time(&config_path);

    adapter
        .read_hooks()
        .into_iter()
        .map(|hook| {
            let hook_name = format!(
                "{}:{}:{}",
                hook.event,
                hook.matcher.as_deref().unwrap_or("*"),
                hook.command
            );
            let cmd_basename = hook
                .command
                .split_whitespace()
                .next()
                .map(|c| {
                    Path::new(c)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_default();
            let description = format!("Runs `{}` on {} event", hook.command, hook.event);

            Extension {
                id: stable_id(&hook_name, "hook", adapter.name()),
                kind: ExtensionKind::Hook,
                name: hook_name,
                description,
                source: Source {
                    origin: SourceOrigin::Agent,
                    url: None,
                    version: None,
                    commit_hash: None,
                },
                agents: vec![adapter.name().to_string()],
                tags: vec![],
                pack: None,
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

                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
                install_meta: None,
            }
        })
        .collect()
}

/// Scan plugins from an agent adapter
pub fn scan_plugins(adapter: &dyn AgentAdapter) -> Vec<Extension> {
    adapter
        .read_plugins()
        .into_iter()
        .map(|plugin| {
            let description = if plugin.source.is_empty() {
                format!("Plugin for {}", adapter.name())
            } else {
                format!("Plugin from {}", plugin.source)
            };
            // Plugins run code, so they implicitly have shell/filesystem permissions
            let permissions = vec![
                Permission::Shell { commands: vec![] },
                Permission::FileSystem { paths: vec![] },
            ];

            let (installed_at, updated_at) = match (plugin.installed_at, plugin.updated_at) {
                (Some(i), Some(u)) => (i, u),
                _ => plugin
                    .path
                    .as_ref()
                    .map(|p| (file_created_time(p), file_modified_time(p)))
                    .unwrap_or_else(|| (Utc::now(), Utc::now())),
            };

            Extension {
                id: stable_id(
                    &format!("{}:{}", plugin.name, plugin.source),
                    "plugin",
                    adapter.name(),
                ),
                kind: ExtensionKind::Plugin,
                name: plugin.name,
                description,
                source: Source {
                    origin: SourceOrigin::Agent,
                    url: None,
                    version: None,
                    commit_hash: None,
                },
                agents: vec![adapter.name().to_string()],
                tags: vec![],
                pack: None,
                permissions,
                enabled: plugin.enabled,
                trust_score: None,
                installed_at,
                updated_at,

                source_path: None,
                cli_parent_id: None,
                cli_meta: None,
                install_meta: None,
            }
        })
        .collect()
}

/// Run `which` to find a binary's absolute path
fn which_binary(name: &str) -> Option<String> {
    if crate::sanitize::validate_binary_name(name).is_err() {
        return None;
    }
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
    if crate::sanitize::validate_binary_name(name).is_err() {
        return None;
    }
    static VERSION_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(\d+\.\d+(?:\.\d+)?)").unwrap());
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
    } else if lower.contains("/pip")
        || lower.contains("/python")
        || lower.contains("/site-packages/")
    {
        Some("pip".into())
    } else if lower.contains("/homebrew/")
        || lower.contains("/cellar/")
        || lower.contains("/linuxbrew/")
    {
        Some("brew".into())
    } else {
        None
    }
}

/// Try to read the install timestamp from Homebrew's INSTALL_RECEIPT.json.
/// Brew stores a `"time"` field (Unix epoch) in each Cellar version directory.
fn brew_install_time(bin_path: &str) -> Option<DateTime<Utc>> {
    let real_path = std::fs::canonicalize(bin_path).ok()?;
    let mut dir: &Path = real_path.parent()?;
    // Walk up (max 5 levels) looking for INSTALL_RECEIPT.json
    for _ in 0..5 {
        let receipt = dir.join("INSTALL_RECEIPT.json");
        if receipt.exists() {
            let content = std::fs::read_to_string(&receipt).ok()?;
            let json: serde_json::Value = serde_json::from_str(&content).ok()?;
            let time = json.get("time")?.as_i64()?;
            return DateTime::from_timestamp(time, 0);
        }
        dir = dir.parent()?;
    }
    None
}

/// Determine install and update timestamps for a CLI binary.
/// Uses brew's INSTALL_RECEIPT.json when available, otherwise falls back to file metadata.
fn cli_timestamps(
    bin_path: &Option<String>,
    install_method: &Option<String>,
) -> (DateTime<Utc>, DateTime<Utc>) {
    match bin_path {
        Some(p) => {
            let path = Path::new(p.as_str());
            let installed = if install_method.as_deref() == Some("brew") {
                brew_install_time(p).unwrap_or_else(|| file_created_time(path))
            } else {
                file_created_time(path)
            };
            (installed, file_modified_time(path))
        }
        None => {
            let now = Utc::now();
            (now, now)
        }
    }
}

/// Scan for CLI binaries referenced by skills and from the KNOWN_CLIS registry.
///
/// Returns a tuple of:
/// - CLI Extension entries
/// - Map from CLI extension ID -> list of skill extension IDs that depend on it
fn scan_cli_binaries(
    existing_extensions: &[Extension],
) -> (Vec<Extension>, HashMap<String, Vec<String>>) {
    let mut candidate_bins: HashSet<String> = HashSet::new();
    // Map: binary_name -> Vec<skill extension id>
    let mut bin_to_skills: HashMap<String, Vec<String>> = HashMap::new();

    // 1. Iterate scanned skills, read their SKILL.md content to extract requires_bins
    for ext in existing_extensions {
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        if let Some(ref path_str) = ext.source_path
            && let Ok(content) = std::fs::read_to_string(path_str)
            && let Some((_, _, requires_bins)) = parse_skill_frontmatter(&content)
        {
            for bin in requires_bins {
                candidate_bins.insert(bin.clone());
                bin_to_skills.entry(bin).or_default().push(ext.id.clone());
            }
        }
    }

    // 2. Add all KNOWN_CLIS binary names to candidate set
    for known in KNOWN_CLIS {
        candidate_bins.insert(known.binary_name.to_string());
    }

    // 2b. Name-based fallback: if a skill's name matches a KNOWN_CLI binary_name,
    // treat it as a child even without explicit bins: in frontmatter.
    for ext in existing_extensions {
        if ext.kind != ExtensionKind::Skill {
            continue;
        }
        for known in KNOWN_CLIS {
            if ext.name == known.binary_name {
                bin_to_skills
                    .entry(known.binary_name.to_string())
                    .or_default()
                    .push(ext.id.clone());
            }
        }
    }

    let mut cli_extensions = Vec::new();
    let mut child_links: HashMap<String, Vec<String>> = HashMap::new();

    // 3. For each candidate, check if it exists
    for bin_name in &candidate_bins {
        let bin_path = which_binary(bin_name);
        // Skip if binary is not installed — we only track CLIs that are actually present
        if bin_path.is_none() {
            continue;
        }
        let known = KNOWN_CLIS
            .iter()
            .find(|k| k.binary_name == bin_name.as_str());

        let version = bin_path.as_ref().and_then(|_| get_binary_version(bin_name));
        let install_method = bin_path.as_ref().and_then(|p| detect_install_method(p));

        let display_name = known
            .map(|k| k.display_name.to_string())
            .unwrap_or_else(|| bin_name.clone());
        let api_domains: Vec<String> = known
            .map(|k| k.api_domains.iter().map(|d| d.to_string()).collect())
            .unwrap_or_default();
        let credentials_path = known.and_then(|k| k.credentials_path.map(|p| p.to_string()));

        // 4. Auto-derive permissions from CliMeta
        let mut permissions = Vec::new();
        if !api_domains.is_empty() {
            permissions.push(Permission::Network {
                domains: api_domains.clone(),
            });
        }
        if credentials_path.is_some() {
            permissions.push(Permission::FileSystem {
                paths: credentials_path.iter().cloned().collect(),
            });
        }
        if bin_path.is_some() {
            permissions.push(Permission::Shell {
                commands: vec![bin_name.clone()],
            });
        }

        let cli_id = cli_stable_id(bin_name);

        // 5. Build child_links: CLI ID -> skill IDs (deduplicated)
        if let Some(skill_ids) = bin_to_skills.get(bin_name.as_str()) {
            let entry = child_links.entry(cli_id.clone()).or_default();
            for sid in skill_ids {
                if !entry.contains(sid) {
                    entry.push(sid.clone());
                }
            }
        }

        // 5b. Derive agents and description from child skills
        let child_skill_ids = child_links.get(&cli_id);
        let mut cli_agents: Vec<String> = Vec::new();
        let mut skill_description: Option<String> = None;
        if let Some(ids) = child_skill_ids {
            for ext in existing_extensions {
                if ids.contains(&ext.id) {
                    for agent in &ext.agents {
                        if !cli_agents.contains(agent) {
                            cli_agents.push(agent.clone());
                        }
                    }
                    if skill_description.is_none() && !ext.description.is_empty() {
                        skill_description = Some(ext.description.clone());
                    }
                }
            }
        }

        let description = if let Some(desc) = skill_description {
            desc
        } else if let Some(ref v) = version {
            format!("{} v{}", display_name, v)
        } else if bin_path.is_some() {
            format!("{} (installed)", display_name)
        } else {
            format!("{} (not installed)", display_name)
        };

        let source = Source {
            origin: if bin_path.is_some() {
                SourceOrigin::Local
            } else {
                SourceOrigin::Registry
            },
            url: known.and_then(|k| k.repo_url.map(|u| u.to_string())),
            version: version.clone(),
            commit_hash: None,
        };
        let pack = source.url.as_deref().and_then(extract_pack_from_url);

        let (installed_at, updated_at) = cli_timestamps(&bin_path, &install_method);

        cli_extensions.push(Extension {
            id: cli_id,
            kind: ExtensionKind::Cli,
            name: display_name,
            description,
            source,
            agents: cli_agents,
            tags: vec![],
            pack,
            permissions,
            enabled: bin_path.is_some(),
            trust_score: None,
            installed_at,
            updated_at,

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
            install_meta: None,
        });
    }

    (cli_extensions, child_links)
}

/// Scan all extension kinds for a specific adapter.
pub fn scan_adapter(adapter: &dyn crate::adapter::AgentAdapter) -> Vec<Extension> {
    let mut all = Vec::new();
    for skill_dir in adapter.skill_dirs() {
        all.extend(scan_skill_dir(&skill_dir, adapter.name()));
    }
    all.extend(scan_mcp_servers(adapter));
    all.extend(scan_hooks(adapter));
    all.extend(scan_plugins(adapter));
    all
}

/// Scan only skills for a specific adapter.
pub fn scan_skills_for(adapter: &dyn crate::adapter::AgentAdapter) -> Vec<Extension> {
    let mut exts = Vec::new();
    for skill_dir in adapter.skill_dirs() {
        exts.extend(scan_skill_dir(&skill_dir, adapter.name()));
    }
    exts
}

/// Scan all extensions from all detected agents
pub fn scan_all(adapters: &[Box<dyn AgentAdapter>]) -> Vec<Extension> {
    let mut all = Vec::new();
    for adapter in adapters {
        if !adapter.detect() {
            continue;
        }
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

    // Back-fill cli_parent_id on MCPs whose command matches a CLI binary
    for ext in &mut all {
        if ext.kind == ExtensionKind::Mcp {
            for cli_ext in &cli_extensions {
                if let Some(ref meta) = cli_ext.cli_meta {
                    // Match by name (e.g. MCP named "officecli" -> CLI binary "officecli")
                    if ext.name == meta.binary_name {
                        ext.cli_parent_id = Some(cli_ext.id.clone());
                        break;
                    }
                    // Match by command path (MCP command contains the CLI binary path)
                    if let Some(ref bin_path) = meta.binary_path {
                        let cmd_in_perms = ext.permissions.iter().any(|p| {
                            if let Permission::Shell { commands } = p {
                                commands
                                    .iter()
                                    .any(|c| c == &meta.binary_name || c == bin_path)
                            } else {
                                false
                            }
                        });
                        if cmd_in_perms {
                            ext.cli_parent_id = Some(cli_ext.id.clone());
                            break;
                        }
                    }
                }
            }
        }
    }

    all.extend(cli_extensions);
    all
}

/// Find all physical directories where a skill is installed, across all detected adapters.
/// Returns (agent_name, skill_dir_path) pairs.
pub fn skill_locations(
    name: &str,
    adapters: &[Box<dyn AgentAdapter>],
) -> Vec<(String, std::path::PathBuf)> {
    let mut locations = Vec::new();
    for adapter in adapters {
        if !adapter.detect() {
            continue;
        }
        for skill_dir in adapter.skill_dirs() {
            let skill_path = skill_dir.join(name);
            if skill_path.join("SKILL.md").exists() || skill_path.join("SKILL.md.disabled").exists()
            {
                locations.push((adapter.name().to_string(), skill_path));
            }
        }
    }
    locations
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

    // Check if this directory is a project (any agent's config present)
    let is_project =
        // Claude Code: .claude/ directory
        dir.join(".claude").is_dir()
        // Claude Code: .mcp.json
        || dir.join(".mcp.json").is_file()
        // Codex: .codex/ directory
        || dir.join(".codex").is_dir()
        // Gemini: .gemini/ directory
        || dir.join(".gemini").is_dir()
        // Cursor: .cursor/rules/ directory or .cursorrules file
        || dir.join(".cursor").join("rules").is_dir()
        || dir.join(".cursorrules").is_file()
        // Copilot: .github/copilot-instructions.md or .github/instructions/
        || dir.join(".github").join("copilot-instructions.md").is_file()
        || dir.join(".github").join("instructions").is_dir()
        // Antigravity: .agent/rules/ or .agent/skills/
        || dir.join(".agent").join("rules").is_dir()
        || dir.join(".agent").join("skills").is_dir();

    if is_project {
        let name = dir
            .file_name()
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
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Skip hidden directories and common non-project directories
        let dir_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if dir_name.starts_with('.')
            || matches!(
                dir_name.as_str(),
                "node_modules"
                    | "target"
                    | "__pycache__"
                    | "vendor"
                    | "dist"
                    | "build"
                    | "venv"
                    | ".venv"
            )
        {
            continue;
        }

        discover_projects_recursive(&path, max_depth, current_depth + 1, projects);
    }
}

// --- Helpers ---

/// Extract the skill name from a SKILL.md file (public for use in commands)
pub fn parse_skill_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    parse_skill_frontmatter(&content).map(|(name, _, _)| name)
}

pub fn parse_skill_frontmatter(content: &str) -> Option<(String, String, Vec<String>)> {
    if !content.starts_with("---") {
        return None;
    }
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

/// Detect source info for a path (public wrapper for install flows).
pub fn detect_source_for(path: &Path) -> Source {
    detect_source(path, false)
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
    let origin = if agent_managed {
        SourceOrigin::Agent
    } else {
        SourceOrigin::Local
    };
    Source {
        origin,
        url: None,
        version: None,
        commit_hash: None,
    }
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

/// Extract "owner/repo" from a git remote URL or short reference.
/// Handles: https://github.com/owner/repo.git, git@github.com:owner/repo.git,
/// and short "owner/repo" or "owner/repo/subpath" formats.
pub fn extract_pack_from_url(url: &str) -> Option<String> {
    // SSH: git@host:owner/repo.git
    if let Some(path) = url.strip_prefix("git@") {
        let after_colon = path.split_once(':')?.1;
        let clean = after_colon.trim_end_matches(".git");
        let parts: Vec<&str> = clean.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(format!("{}/{}", parts[0], parts[1]));
        }
    }
    // HTTPS/SSH URL: https://host/owner/repo.git
    if let Some(pos) = url.find("://") {
        let after_scheme = &url[pos + 3..];
        let after_host = after_scheme.split_once('/')?.1;
        let clean = after_host.trim_end_matches(".git");
        let parts: Vec<&str> = clean.splitn(3, '/').collect();
        if parts.len() >= 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Some(format!("{}/{}", parts[0], parts[1]));
        }
    }
    // Short format: "owner/repo" or "owner/repo/subpath"
    let parts: Vec<&str> = url.splitn(3, '/').collect();
    if parts.len() >= 2
        && !parts[0].is_empty()
        && !parts[1].is_empty()
        && !parts[0].contains('.')
        && !parts[0].contains(':')
    {
        return Some(format!("{}/{}", parts[0], parts[1]));
    }
    None
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

static SKILL_SENSITIVE_PATHS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:~|/(?:etc|home/\w+))/[\w.\-/]+").unwrap());

static SKILL_URL_DOMAINS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://([\w.\-]+)").unwrap());

static SKILL_SHELL_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```(?:bash|shell|sh|zsh)\s*\n(.*?)```").unwrap());

static SKILL_DB_ENGINES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(postgres(?:ql)?|mysql|mariadb|sqlite|mongodb|redis)\b").unwrap()
});

fn infer_skill_permissions(content: &str) -> Vec<Permission> {
    let mut perms = Vec::new();
    let lower = content.to_lowercase();

    if lower.contains("file")
        || lower.contains("read")
        || lower.contains("write")
        || lower.contains("path")
    {
        let paths: Vec<String> = SKILL_SENSITIVE_PATHS
            .find_iter(content)
            .map(|m| m.as_str().to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        perms.push(Permission::FileSystem { paths });
    }
    if lower.contains("http")
        || lower.contains("api")
        || lower.contains("fetch")
        || lower.contains("url")
    {
        let domains: Vec<String> = SKILL_URL_DOMAINS
            .captures_iter(content)
            .map(|c| c[1].to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        perms.push(Permission::Network { domains });
    }
    if lower.contains("bash")
        || lower.contains("shell")
        || lower.contains("command")
        || lower.contains("exec")
    {
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
        perms.push(Permission::Shell {
            commands: cmds.into_iter().collect(),
        });
    }
    if lower.contains("database")
        || lower.contains("sql")
        || lower.contains("postgres")
        || lower.contains("mysql")
    {
        let engines: Vec<String> = SKILL_DB_ENGINES
            .captures_iter(&lower)
            .map(|c| c[1].to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        perms.push(Permission::Database { engines });
    }
    perms
}

fn file_created_time(path: &Path) -> chrono::DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.created())
        .map(chrono::DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc::now())
}

fn file_modified_time(path: &Path) -> chrono::DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(chrono::DateTime::<Utc>::from)
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
            if let Some(cf) = stat_config_file(path, adapter.name(), *category, ConfigScope::Global)
            {
                configs.push(cf);
            }
        }
    }

    // --- Project files ---
    let project_groups: [(ConfigCategory, Vec<String>); 4] = [
        (ConfigCategory::Rules, adapter.project_rules_patterns()),
        (ConfigCategory::Memory, adapter.project_memory_patterns()),
        (
            ConfigCategory::Settings,
            adapter.project_settings_patterns(),
        ),
        (ConfigCategory::Ignore, adapter.project_ignore_patterns()),
    ];

    for (project_name, project_path) in projects {
        let project_root = std::path::Path::new(project_path);
        if !project_root.is_dir() {
            continue;
        }

        let scope = ConfigScope::Project {
            name: project_name.clone(),
            path: project_path.clone(),
        };

        for (category, patterns) in &project_groups {
            for pattern in patterns {
                let resolved = resolve_pattern(project_root, pattern);
                for path in resolved {
                    if let Some(cf) =
                        stat_config_file(&path, adapter.name(), *category, scope.clone())
                    {
                        configs.push(cf);
                    }
                }
            }
        }
    }

    // Sort by category order, then by scope (global first), then by file name
    configs.sort_by(|a, b| {
        a.category
            .order()
            .cmp(&b.category.order())
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
    if !metadata.is_file() {
        return None;
    }

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
        is_dir: metadata.is_dir(),
        exists: true,
        custom_id: None,
        custom_label: None,
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
    fn test_extract_pack_https() {
        assert_eq!(
            extract_pack_from_url("https://github.com/alice/repo.git"),
            Some("alice/repo".into())
        );
        assert_eq!(
            extract_pack_from_url("https://github.com/alice/repo"),
            Some("alice/repo".into())
        );
        assert_eq!(
            extract_pack_from_url("https://gitlab.com/org/project.git"),
            Some("org/project".into())
        );
    }

    #[test]
    fn test_extract_pack_ssh() {
        assert_eq!(
            extract_pack_from_url("git@github.com:alice/repo.git"),
            Some("alice/repo".into())
        );
        assert_eq!(
            extract_pack_from_url("git@gitlab.com:org/project.git"),
            Some("org/project".into())
        );
    }

    #[test]
    fn test_extract_pack_short() {
        assert_eq!(
            extract_pack_from_url("alice/repo"),
            Some("alice/repo".into())
        );
        assert_eq!(
            extract_pack_from_url("alice/repo/subpath"),
            Some("alice/repo".into())
        );
    }

    #[test]
    fn test_extract_pack_none() {
        assert_eq!(extract_pack_from_url("not-a-url"), None);
        assert_eq!(extract_pack_from_url(""), None);
    }

    #[test]
    fn test_discover_projects() {
        let root = TempDir::new().unwrap();

        // Project with .mcp.json (Claude Code)
        let proj1 = root.path().join("project-a");
        std::fs::create_dir_all(&proj1).unwrap();
        std::fs::write(proj1.join(".mcp.json"), r#"{"mcpServers":{}}"#).unwrap();

        // Project with .claude/ (Claude Code)
        let proj2 = root.path().join("project-b");
        std::fs::create_dir_all(proj2.join(".claude").join("skills")).unwrap();

        // Project with .codex/ (Codex)
        let proj3 = root.path().join("project-c");
        std::fs::create_dir_all(proj3.join(".codex")).unwrap();

        // Project with .cursor/rules/ (Cursor)
        let proj4 = root.path().join("project-d");
        std::fs::create_dir_all(proj4.join(".cursor").join("rules")).unwrap();

        // Project with .gemini/ (Gemini)
        let proj5 = root.path().join("project-e");
        std::fs::create_dir_all(proj5.join(".gemini")).unwrap();

        // Not a project
        let non_proj = root.path().join("not-a-project");
        std::fs::create_dir_all(&non_proj).unwrap();

        // .github/ alone is NOT a project (too generic)
        let github_only = root.path().join("github-repo");
        std::fs::create_dir_all(github_only.join(".github")).unwrap();

        let discovered = discover_projects(root.path(), 4);
        assert_eq!(discovered.len(), 5);
        let names: Vec<&str> = discovered.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"project-a"));
        assert!(names.contains(&"project-b"));
        assert!(names.contains(&"project-c"));
        assert!(names.contains(&"project-d"));
        assert!(names.contains(&"project-e"));
        assert!(!names.contains(&"github-repo"));
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
        )
        .unwrap();

        let extensions = super::scan_skill_dir(dir.path(), "claude");
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].name, "my-skill");
        assert!(
            !extensions[0].enabled,
            "Disabled skill should have enabled=false"
        );
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
        std::fs::rename(
            skill_dir.join("SKILL.md"),
            skill_dir.join("SKILL.md.disabled"),
        )
        .unwrap();
        let disabled_exts = super::scan_skill_dir(dir.path(), "claude");
        let disabled_id = disabled_exts[0].id.clone();

        assert_eq!(
            enabled_id, disabled_id,
            "Same skill should produce same ID whether enabled or disabled"
        );
    }

    #[test]
    fn test_disabled_skill_source_path_is_enabled_path() {
        let dir = TempDir::new().unwrap();
        let skill_dir = dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md.disabled"),
            "---\nname: my-skill\n---\n",
        )
        .unwrap();

        let extensions = super::scan_skill_dir(dir.path(), "claude");
        assert_eq!(extensions.len(), 1);
        let source_path = extensions[0].source_path.as_ref().unwrap();
        assert!(
            source_path.ends_with("SKILL.md"),
            "source_path should point to SKILL.md, not SKILL.md.disabled, got: {}",
            source_path
        );
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

        let rules: Vec<_> = configs
            .iter()
            .filter(|c| c.category == ConfigCategory::Rules)
            .collect();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].file_name, "CLAUDE.md");
        assert!(matches!(rules[0].scope, ConfigScope::Global));

        let settings: Vec<_> = configs
            .iter()
            .filter(|c| c.category == ConfigCategory::Settings)
            .collect();
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
        let projects = vec![(
            "myproject".to_string(),
            project.to_string_lossy().to_string(),
        )];
        let configs = scan_agent_configs(&adapter, &projects);

        let project_rules: Vec<_> = configs
            .iter()
            .filter(|c| {
                c.category == ConfigCategory::Rules
                    && matches!(&c.scope, ConfigScope::Project { .. })
            })
            .collect();
        assert_eq!(project_rules.len(), 1);

        // Claude Code does not have .claudeignore
        let ignores: Vec<_> = configs
            .iter()
            .filter(|c| c.category == ConfigCategory::Ignore)
            .collect();
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
        let content =
            "---\nname: lark-cal\ndescription: Calendar\nbins:\n  - \"lark-cli\"\n---\nBody";
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
        assert_eq!(
            detect_install_method("/usr/local/lib/node_modules/.bin/wecom-cli"),
            Some("npm".into())
        );
        assert_eq!(
            detect_install_method("/Users/test/.cargo/bin/tool"),
            Some("cargo".into())
        );
        assert_eq!(detect_install_method("/usr/local/bin/tool"), None);
    }
}
