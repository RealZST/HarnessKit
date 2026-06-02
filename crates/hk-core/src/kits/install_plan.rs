use crate::adapter::{AgentAdapter, McpFormat};
use crate::kits::zip_io::read_manifest_from_zip;
use crate::models::{ConfigCategory, ExtensionKind};
use crate::HkError;
use std::path::{Path, PathBuf};

/// Returns `true` when `name` is already present as an MCP server entry in the
/// config file at `config_path`, parsing per the agent's `McpFormat`. Returns
/// `false` if the file does not exist, fails to parse, or the key is absent.
fn mcp_entry_exists(config_path: &Path, name: &str, format: McpFormat) -> bool {
    if !config_path.exists() {
        return false;
    }
    match format {
        McpFormat::McpServers | McpFormat::Servers => {
            let key = if matches!(format, McpFormat::McpServers) {
                "mcpServers"
            } else {
                "servers"
            };
            let Ok(bytes) = std::fs::read(config_path) else {
                return false;
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return false;
            };
            v.get(key).and_then(|m| m.get(name)).is_some()
        }
        McpFormat::Toml => {
            let Ok(s) = std::fs::read_to_string(config_path) else {
                return false;
            };
            let Ok(doc) = s.parse::<toml::Table>() else {
                return false;
            };
            doc.get("mcp_servers")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get(name))
                .is_some()
        }
        McpFormat::Opencode => {
            // OpenCode uses JSONC with `mcp` top-level key. For v1, parse as
            // strict JSON; failure means we conservatively return false (no
            // conflict detected). OpenCode authors typically don't comment
            // their config.
            let Ok(bytes) = std::fs::read(config_path) else {
                return false;
            };
            let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return false;
            };
            v.get("mcp").and_then(|m| m.get(name)).is_some()
        }
        McpFormat::HermesYaml => {
            let Ok(s) = std::fs::read_to_string(config_path) else {
                return false;
            };
            let Ok(doc) = serde_yaml::from_str::<serde_yaml::Value>(&s) else {
                return false;
            };
            doc.get("mcp_servers")
                .and_then(|v| v.get(name))
                .is_some()
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlanItem {
    pub asset_kind: PlanItemKind,
    pub zip_entry_path: String,
    pub target_path: PathBuf,
    pub conflicts_with_existing: bool,
}

#[derive(Debug, Clone)]
pub enum PlanItemKind {
    Extension {
        source_extension_id: String,
        kind: ExtensionKind,
        asset_name: String,
        /// Source URL preserved from the Kit manifest. When present, the
        /// sync path writes it into the deployed extension's install_meta
        /// so the new instance merges with its source via `extensionGroupKey`
        /// instead of falling back to scope-keyed isolation.
        source_url: Option<String>,
        /// Git revision / branch carried from the manifest so the deployed
        /// copy's install_meta has the same upstream metadata as the
        /// original (drives the Extensions detail panel's version chip).
        source_revision: Option<String>,
        source_branch: Option<String>,
    },
    Config {
        agent: String,
        category: ConfigCategory,
    },
}

/// Compute the per-item install plan for a Kit zip into a project under an agent.
pub fn compute_kit_install_plan(
    adapters: &[Box<dyn AgentAdapter>],
    zip_path: &Path,
    project_path: &str,
    agent_name: &str,
) -> Result<Vec<PlanItem>, HkError> {
    let adapter = adapters
        .iter()
        .find(|a| a.name() == agent_name)
        .ok_or_else(|| HkError::NotFound(format!("Agent '{agent_name}' not found")))?;
    let manifest = read_manifest_from_zip(zip_path)?;
    let project_root = PathBuf::from(project_path);
    let mut items = Vec::new();

    for ext in manifest.extensions.iter() {
        let target_dir = match ext.kind {
            ExtensionKind::Skill => adapter
                .project_skill_dirs()
                .into_iter()
                .find(|p| !p.contains('*') && !p.ends_with('/'))
                .ok_or_else(|| HkError::Internal(format!(
                    "Agent '{agent_name}' has no project-level skill dir"
                )))?,
            ExtensionKind::Mcp => adapter
                .project_mcp_config_relpath()
                .ok_or_else(|| HkError::Validation(format!(
                    "Agent '{agent_name}' doesn't support project-level MCP servers. \
                     Pick a different agent, or remove MCP entries from this Kit."
                )))?,
            ExtensionKind::Hook => adapter
                .project_hook_config_relpath()
                .ok_or_else(|| HkError::Validation(format!(
                    "Agent '{agent_name}' doesn't support project-level hooks. \
                     Pick a different agent, or remove hook entries from this Kit."
                )))?,
            ExtensionKind::Cli => adapter
                .project_skill_dirs()
                .into_iter()
                .next()
                .ok_or_else(|| HkError::Internal(format!(
                    "Agent '{agent_name}' has no install dir for CLI assets"
                )))?,
            ExtensionKind::Plugin => {
                return Err(HkError::Internal(
                    "plugin kind is not Kit-able in v1".into(),
                ));
            }
        };
        // Skills/CLI are folder assets: the target dir is `<project_root>/<target_dir>/<asset_name>/`.
        // MCP/Hook are single-file assets: the target file is `<project_root>/<target_dir>`.
        let target = match ext.kind {
            ExtensionKind::Skill | ExtensionKind::Cli => {
                project_root.join(&target_dir).join(&ext.name)
            }
            ExtensionKind::Mcp | ExtensionKind::Hook => project_root.join(&target_dir),
            ExtensionKind::Plugin => unreachable!(),
        };
        // For MCP entries the right conflict question is "does this server name
        // already exist in mcpServers?" not "does the config file exist?".
        // Hook is deferred to v2 (pack time returns an error), so no conflict
        // detection is needed — set false as a safe fallback.
        let conflicts_with_existing = match ext.kind {
            ExtensionKind::Mcp => mcp_entry_exists(&target, &ext.name, adapter.mcp_format()),
            ExtensionKind::Hook => false,
            _ => target.exists(),
        };
        items.push(PlanItem {
            asset_kind: PlanItemKind::Extension {
                source_extension_id: ext.source_extension_id.clone(),
                kind: ext.kind,
                asset_name: ext.name.clone(),
                source_url: ext.source_url.clone(),
                source_revision: ext.source_revision.clone(),
                source_branch: ext.source_branch.clone(),
            },
            zip_entry_path: ext.asset_path.clone(),
            target_path: target,
            conflicts_with_existing,
        });
    }

    for cfg in manifest.config_files.iter() {
        if cfg.agent != agent_name {
            // Multi-agent Kit can hold config files for several agents; we install
            // only those targeting the chosen agent for this run.
            continue;
        }
        // Try the adapter's deterministic relpath first. If the adapter has
        // multiple patterns (e.g. Claude has CLAUDE.md + .claude/CLAUDE.md +
        // .claude/rules/*.md) — pick_unique_concrete returns None — fall back to
        // placing the file at project_root/<cfg.filename> (root-level convention).
        let relpath = match cfg.category {
            ConfigCategory::Rules => adapter
                .project_rules_target_relpath()
                .or_else(|| Some(cfg.filename.clone())),
            ConfigCategory::Memory => adapter
                .project_memory_target_relpath()
                .or_else(|| Some(cfg.filename.clone())),
            _ => Some(cfg.filename.clone()),
        }
        .ok_or_else(|| HkError::Internal(format!(
            "Agent '{agent_name}' has no deterministic relpath for {:?}",
            cfg.category
        )))?;
        let target = project_root.join(&relpath);
        let conflicts_with_existing = target.exists();
        items.push(PlanItem {
            asset_kind: PlanItemKind::Config {
                agent: cfg.agent.clone(),
                category: cfg.category,
            },
            zip_entry_path: cfg.asset_path.clone(),
            target_path: target,
            conflicts_with_existing,
        });
    }

    Ok(items)
}
