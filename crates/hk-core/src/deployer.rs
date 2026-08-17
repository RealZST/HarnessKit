use crate::HkError;
use crate::adapter::{HookEntry, HookFormat, McpFormat, McpServerEntry, McpTransport, RemoteMcpSchema};
use fs2::FileExt;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

pub fn deploy_skill(source_path: &Path, target_skill_dir: &Path) -> Result<String, HkError> {
    std::fs::create_dir_all(target_skill_dir)?;
    if source_path.is_dir() {
        let dir_name = source_path
            .file_name()
            .ok_or_else(|| HkError::Validation("Invalid source path".into()))?
            .to_string_lossy()
            .to_string();
        let dest = target_skill_dir.join(&dir_name);
        copy_dir_recursive(source_path, &dest)?;
        Ok(dir_name)
    } else {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| HkError::Validation("Invalid source path".into()))?
            .to_string_lossy()
            .to_string();
        let dest = target_skill_dir.join(&file_name);
        std::fs::copy(source_path, &dest)?;
        Ok(file_name)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), HkError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        // TOCTOU-safe symlink check: use symlink_metadata (lstat) instead of
        // following symlinks. Re-check right before the copy to close the race
        // window between readdir and the actual file operation.
        let meta = match std::fs::symlink_metadata(&src_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "[hk] warning: cannot read metadata for {}: {e}",
                    src_path.display()
                );
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            eprintln!("[hk] warning: skipping symlink: {}", src_path.display());
            continue;
        }
        if meta.file_type().is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Sanitize an MCP server name to contain only `[a-zA-Z0-9_-]`.
///
/// Codex requires server names to match `^[a-zA-Z0-9_-]+$`, and TOML bare keys
/// also cannot contain characters like `/`. This replaces any disallowed character
/// with `-` so that names like `microsoft/markitdown` become `microsoft-markitdown`.
pub fn sanitize_mcp_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Resolve a command name to its absolute path using `which`.
///
/// GUI-based agents (e.g. Antigravity) do not inherit the user's shell `$PATH`,
/// so bare command names like `npx` or `uvx` fail with ENOENT. This resolves the
/// command to an absolute path (e.g. `/Users/zoe/.local/bin/uvx`) at deploy time.
/// Returns the original command unchanged if resolution fails.
pub fn resolve_command_path(command: &str) -> String {
    // Already absolute — nothing to do.
    // Unix: starts with '/'
    // Windows: starts with drive letter like 'C:\'
    if command.starts_with('/') || crate::sanitize::is_windows_abs_path(command) {
        return command.to_string();
    }
    crate::scanner::run_which(command).unwrap_or_else(|| command.to_string())
}

/// Build a PATH value that includes the directory of the resolved command.
///
/// GUI-based agents don't inherit the user's shell PATH, so scripts like `npx`
/// (which use `#!/usr/bin/env node`) fail because `node` isn't found.
/// This constructs a PATH containing the command's directory plus essential
/// system directories, ensuring sibling binaries (e.g. `node` next to `npx`)
/// are discoverable.
pub fn build_path_for_command(resolved_command: &str) -> Option<String> {
    let parent = std::path::Path::new(resolved_command).parent()?;
    let parent_str = parent.to_str()?;
    if parent_str.is_empty() {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        Some(format!(r"{};C:\Windows\System32;C:\Windows", parent_str))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(format!("{}:/usr/local/bin:/usr/bin:/bin", parent_str))
    }
}

/// For agents that don't reliably inherit shell `$PATH` (see
/// `AgentAdapter::needs_path_injection`), resolve the entry's command to an
/// absolute path and inject `PATH` into env so scripts with `#!/usr/bin/env node`
/// shebangs can find sibling binaries.
///
/// Idempotent and non-destructive: existing `PATH` in env is preserved (or_insert),
/// so a user's manual override is never overwritten. To re-compute PATH (e.g. when
/// repairing dirty data), remove the existing key first then call this function.
pub fn ensure_path_injection(entry: &mut crate::adapter::McpServerEntry) {
    // Remote entries launch no subprocess — nothing to resolve or inject.
    if entry.transport != McpTransport::Stdio {
        return;
    }
    entry.command = resolve_command_path(&entry.command);
    if let Some(path_val) = build_path_for_command(&entry.command) {
        entry.env.entry("PATH".to_string()).or_insert(path_val);
    }
}

/// Top-level JSON key under which each JSON-based MCP format stores server
/// entries. The format → key mapping is the only thing that varies between
/// JSON-format agents in the remove/restore/read paths, so centralizing it
/// here keeps that knowledge in one place and forces explicit handling of
/// every JSON variant via the compiler-checked match.
///
/// Toml and Opencode are excluded — both formats route to dedicated
/// functions (`*_toml` / `*_opencode`) before this helper is reached.
/// Centralizing the format → key map this way forces every variant to be
/// considered when a new MCP-supporting agent is added.
fn json_top_key(format: McpFormat) -> &'static str {
    match format {
        McpFormat::McpServers => "mcpServers",
        McpFormat::Servers => "servers",
        McpFormat::Toml => unreachable!("Toml format uses a separate TOML code path"),
        McpFormat::Opencode => {
            unreachable!("Opencode format routes through dedicated CST helpers")
        }
        McpFormat::HermesYaml => {
            unreachable!("HermesYaml format routes through dedicated YAML helpers")
        }
        McpFormat::DshCordis => {
            unreachable!(
                "DshCordis never reaches the JSON writers — install/remove \
                 route through the dedicated cordis writers \
                 (deploy_mcp_server_dsh_cordis / remove_mcp_server_dsh_cordis); \
                 toggling uses the native patch-layer path (set_dsh_mcp_enabled)"
            )
        }
    }
}

/// Deploy an MCP server config entry into the target agent's config file.
/// Format varies by agent — see `McpFormat`. Remote (HTTP/SSE) entries are
/// validated against the target's `RemoteMcpSchema` first: a target that
/// can't express the entry's transport gets a hard error instead of a
/// broken config (issue #105's failure mode was writing `command = ""`).
/// The UI prevents these combinations up front via `AgentCapabilities`;
/// this guard covers direct API callers.
pub fn deploy_mcp_server(
    config_path: &Path,
    entry: &McpServerEntry,
    adapter: &dyn crate::adapter::AgentAdapter,
) -> Result<(), HkError> {
    let remote_schema = adapter.remote_mcp_schema();
    if entry.transport != McpTransport::Stdio {
        validate_remote_mcp_target(entry, adapter.name(), remote_schema)?;
    }
    match adapter.mcp_format() {
        McpFormat::McpServers => {
            deploy_mcp_server_json(config_path, entry, "mcpServers", remote_schema)
        }
        McpFormat::Servers => deploy_mcp_server_json(config_path, entry, "servers", remote_schema),
        McpFormat::Toml => deploy_mcp_server_toml(config_path, entry),
        McpFormat::Opencode => deploy_mcp_server_opencode(config_path, entry),
        McpFormat::HermesYaml => deploy_mcp_server_hermes_yaml(config_path, entry),
        McpFormat::DshCordis => deploy_mcp_server_dsh_cordis(config_path, entry),
    }
}

/// Refuse remote entries the target agent cannot load.
fn validate_remote_mcp_target(
    entry: &McpServerEntry,
    agent_name: &str,
    schema: RemoteMcpSchema,
) -> Result<(), HkError> {
    if entry.url.is_none() {
        return Err(HkError::ConfigCorrupted(format!(
            "Remote MCP server '{}' has no url",
            entry.name
        )));
    }
    match schema {
        RemoteMcpSchema::Unsupported => Err(HkError::Validation(format!(
            "{agent_name} does not support remote (HTTP/SSE) MCP servers"
        ))),
        RemoteMcpSchema::Toml if entry.transport == McpTransport::Sse => {
            Err(HkError::Validation(format!(
                "{agent_name} supports Streamable HTTP MCP servers only, not SSE"
            )))
        }
        _ => Ok(()),
    }
}

/// The JSON object for one server entry, in the target agent's spelling.
fn build_mcp_json_value(
    entry: &McpServerEntry,
    remote: RemoteMcpSchema,
) -> Result<serde_json::Value, HkError> {
    if entry.transport == McpTransport::Stdio {
        return Ok(serde_json::json!({
            "command": entry.command,
            "args": entry.args,
            "env": entry.env,
        }));
    }
    let url = entry.url.clone().unwrap_or_default();
    let mut obj = serde_json::Map::new();
    match remote {
        RemoteMcpSchema::TypeAndUrl => {
            let type_str = if entry.transport == McpTransport::Sse {
                "sse"
            } else {
                "http"
            };
            obj.insert("type".into(), type_str.into());
            obj.insert("url".into(), url.into());
        }
        RemoteMcpSchema::PlainUrl => {
            obj.insert("url".into(), url.into());
        }
        RemoteMcpSchema::GeminiSplit => {
            let key = if entry.transport == McpTransport::Sse {
                "url"
            } else {
                "httpUrl"
            };
            obj.insert(key.into(), url.into());
        }
        RemoteMcpSchema::ServerUrl => {
            obj.insert("serverUrl".into(), url.into());
        }
        // Non-JSON formats have their own writers; validation rejects
        // Unsupported before this point. Reaching here means an adapter's
        // mcp_format() and remote_mcp_schema() disagree — surface it as an
        // error instead of a panic.
        RemoteMcpSchema::Toml
        | RemoteMcpSchema::OpencodeRemote
        | RemoteMcpSchema::HermesUrl
        | RemoteMcpSchema::Unsupported => {
            return Err(HkError::Internal(format!(
                "remote JSON value requested for non-JSON schema {remote:?}"
            )));
        }
    }
    if !entry.headers.is_empty() {
        obj.insert(
            "headers".into(),
            serde_json::Value::Object(
                entry
                    .headers
                    .iter()
                    .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                    .collect(),
            ),
        );
    }
    Ok(serde_json::Value::Object(obj))
}

/// JSON-based MCP deploy (Claude, Gemini, Cursor, Antigravity, Copilot,
/// Windsurf, Kiro, omp). `top_key` is "mcpServers" or "servers" depending
/// on the agent; `remote` picks the agent's remote-entry spelling.
fn deploy_mcp_server_json(
    config_path: &Path,
    entry: &McpServerEntry,
    top_key: &str,
    remote: RemoteMcpSchema,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let servers = config
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
            .entry(top_key)
            .or_insert_with(|| serde_json::json!({}));
        servers
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted(format!("{} is not an object", top_key)))?
            .insert(entry.name.clone(), build_mcp_json_value(entry, remote)?);
        Ok(())
    })
}

/// TOML-based MCP deploy (Codex: ~/.codex/config.toml with [mcp_servers.<name>]).
fn deploy_mcp_server_toml(config_path: &Path, entry: &McpServerEntry) -> Result<(), HkError> {
    // Build server entry table. Remote entries use url/http_headers
    // (Codex's Streamable HTTP schema); stdio entries use command/args/env.
    // Dispatch on transport (like the JSON writers); validation guarantees
    // remote entries carry a url by the time a writer runs.
    let mut server_table = toml::Table::new();
    if entry.transport != McpTransport::Stdio {
        let url = entry.url.clone().unwrap_or_default();
        server_table.insert("url".into(), toml::Value::String(url));
        if !entry.headers.is_empty() {
            let mut headers_table = toml::Table::new();
            for (k, v) in &entry.headers {
                headers_table.insert(k.clone(), toml::Value::String(v.clone()));
            }
            server_table.insert("http_headers".into(), toml::Value::Table(headers_table));
        }
    } else {
        server_table.insert("command".into(), toml::Value::String(entry.command.clone()));
        if !entry.args.is_empty() {
            server_table.insert(
                "args".into(),
                toml::Value::Array(
                    entry
                        .args
                        .iter()
                        .map(|a| toml::Value::String(a.clone()))
                        .collect(),
                ),
            );
        }
        if !entry.env.is_empty() {
            let mut env_table = toml::Table::new();
            for (k, v) in &entry.env {
                env_table.insert(k.clone(), toml::Value::String(v.clone()));
            }
            server_table.insert("env".into(), toml::Value::Table(env_table));
        }
    }

    upsert_mcp_server_toml(config_path, &entry.name, toml::Value::Table(server_table))
}

/// Insert/replace `[mcp_servers.<name>]` in a TOML config, preserving the
/// rest of the file. Shared by deploy (freshly built table) and restore
/// (snapshot transcoded wholesale).
///
/// Codex requires names to match ^[a-zA-Z0-9_-]+$; sanitize before inserting.
/// The original name is stored as `_hk_name` so the scanner can recover it
/// for consistent grouping with other agents that use the unsanitized name.
fn upsert_mcp_server_toml(
    config_path: &Path,
    name: &str,
    mut server_val: toml::Value,
) -> Result<(), HkError> {
    let parent = config_path
        .parent()
        .ok_or_else(|| HkError::Validation("Invalid config path".into()))?;
    std::fs::create_dir_all(parent)?;

    // Read existing TOML or start fresh
    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut doc: toml::Table = if existing.is_empty() {
        toml::Table::new()
    } else {
        existing
            .parse::<toml::Table>()
            .map_err(|e| HkError::ConfigCorrupted(format!("Failed to parse TOML config: {e}")))?
    };

    // Get or create [mcp_servers] table
    let mcp_servers = doc
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| HkError::ConfigCorrupted("mcp_servers is not a table".into()))?;

    let safe_name = sanitize_mcp_name(name);
    if safe_name != name {
        server_val
            .as_table_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("MCP server entry is not a table".into()))?
            .insert("_hk_name".into(), toml::Value::String(name.to_string()));
    }
    mcp_servers.insert(safe_name, server_val);

    // Write back atomically
    atomic_write(
        config_path,
        &toml::to_string_pretty(&doc).map_err(|e| HkError::Internal(e.to_string()))?,
    )?;

    Ok(())
}

/// JSON-based MCP deploy for OpenCode (`~/.config/opencode/opencode.json[c]`).
/// Schema reference: https://opencode.ai/config.json (McpLocalConfig).
///
/// Differs from `mcpServers`/`servers` agents in four ways:
///   - top-level key is `"mcp"`
///   - `command` is a single array merging the binary + its args
///   - env block is named `"environment"` (not `"env"`)
///   - entry must declare `"type": "local"` (the schema also defines a
///     `"remote"` variant that HarnessKit does not deploy)
///
/// `additionalProperties: false` upstream means we must not emit any
/// extra fields (e.g. no separate `args`/`env`).
///
/// Goes through `locked_modify_jsonc` so existing user comments and
/// formatting in opencode.jsonc (or opencode.json — OpenCode's loader
/// runs both through jsonc-parser) survive a deploy. Replaces an
/// existing same-named entry in place rather than re-appending.
fn deploy_mcp_server_opencode(config_path: &Path, entry: &McpServerEntry) -> Result<(), HkError> {
    let value = build_opencode_mcp_value(entry);
    locked_modify_jsonc(config_path, |root| {
        let mcp = root.object_value_or_set("mcp");
        let cst_value = to_cst_input(&value);
        if let Some(existing) = mcp.get(&entry.name) {
            existing.set_value(cst_value);
        } else {
            mcp.append(&entry.name, cst_value);
        }
        Ok(())
    })
}

/// Load config.yaml as a mutable root mapping (empty mapping if absent/blank),
/// run `f`, then atomically write it back. The single primitive every Hermes
/// YAML writer (MCP, hooks, plugins) routes through.
///
/// Note: CREATES the file (and parent dirs) even on a no-op `f`; remove-style
/// callers that must not create an absent file should pre-check existence.
fn modify_hermes_yaml(
    config_path: &Path,
    f: impl FnOnce(&mut serde_yaml::Mapping) -> Result<(), HkError>,
) -> Result<(), HkError> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut doc: serde_yaml::Value = if existing.trim().is_empty() {
        serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
    } else {
        serde_yaml::from_str(&existing).map_err(|e| {
            HkError::ConfigCorrupted(format!("Failed to parse Hermes config.yaml: {e}"))
        })?
    };
    let root = doc
        .as_mapping_mut()
        .ok_or_else(|| HkError::ConfigCorrupted("config.yaml root is not a mapping".into()))?;
    f(root)?;
    let output = serde_yaml::to_string(&doc).map_err(|e| HkError::Internal(e.to_string()))?;
    atomic_write(config_path, &output)?;
    Ok(())
}

/// YAML-based MCP deploy for Hermes (`~/.hermes/config.yaml`, "mcp_servers" key).
///
/// Reads the full config.yaml, upserts the server entry under `mcp_servers.<name>`,
/// and writes the file back. Command-based entries use `command`/`args`/`env` keys;
/// URL-based entries (where `entry.command` starts with "http") use a `url` key.
/// The rest of config.yaml is preserved through serde_yaml round-trip.
fn deploy_mcp_server_hermes_yaml(
    config_path: &Path,
    entry: &McpServerEntry,
) -> Result<(), HkError> {
    modify_hermes_yaml(config_path, |root| {
        let servers = root
            .entry("mcp_servers".into())
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("mcp_servers is not a mapping".into()))?;
        let mut server = serde_yaml::Mapping::new();
        if entry.transport != crate::adapter::McpTransport::Stdio {
            // Remote: {url, headers?, transport: sse?} — Streamable HTTP
            // is Hermes' default, so only SSE needs the transport key.
            let url = entry.url.clone().unwrap_or_default();
            server.insert("url".into(), url.into());
            if entry.transport == crate::adapter::McpTransport::Sse {
                server.insert("transport".into(), "sse".into());
            }
            if !entry.headers.is_empty() {
                let mut headers = serde_yaml::Mapping::new();
                for (k, v) in &entry.headers {
                    headers.insert(k.clone().into(), v.clone().into());
                }
                server.insert("headers".into(), serde_yaml::Value::Mapping(headers));
            }
        } else {
            server.insert("command".into(), entry.command.clone().into());
            if !entry.args.is_empty() {
                let args: Vec<serde_yaml::Value> = entry
                    .args
                    .iter()
                    .cloned()
                    .map(serde_yaml::Value::String)
                    .collect();
                server.insert("args".into(), serde_yaml::Value::Sequence(args));
            }
            if !entry.env.is_empty() {
                let mut env = serde_yaml::Mapping::new();
                for (k, v) in &entry.env {
                    env.insert(k.clone().into(), v.clone().into());
                }
                server.insert("env".into(), serde_yaml::Value::Mapping(env));
            }
        }
        server.insert("enabled".into(), serde_yaml::Value::Bool(true));
        servers.insert(
            entry.name.clone().into(),
            serde_yaml::Value::Mapping(server),
        );
        Ok(())
    })
}

/// Add/remove a plugin name under `plugins.enabled` in Hermes config.yaml.
/// Hermes plugins are disabled by default; presence in the list = enabled.
pub fn set_hermes_plugin_enabled(
    config_path: &Path,
    name: &str,
    enabled: bool,
) -> Result<(), HkError> {
    modify_hermes_yaml(config_path, |root| {
        let plugins = root
            .entry("plugins".into())
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("plugins is not a mapping".into()))?;
        let list = plugins
            .entry("enabled".into())
            .or_insert_with(|| serde_yaml::Value::Sequence(vec![]))
            .as_sequence_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("plugins.enabled is not a sequence".into()))?;
        let present = list.iter().any(|v| v.as_str() == Some(name));
        if enabled && !present {
            list.push(serde_yaml::Value::String(name.to_string()));
        } else if !enabled && present {
            list.retain(|v| v.as_str() != Some(name));
        }
        Ok(())
    })
}

/// Flip a Hermes MCP server's native `enabled` field IN PLACE (true/false),
/// leaving the rest of the entry (command/args/env/tools/…) untouched. This is
/// the in-place "disable" Hermes itself uses: the config stays put and only
/// `enabled` toggles — unlike HarnessKit's generic MCP disable, it never removes
/// the entry, snapshots it, or redacts secrets.
///
/// Hermes supports a per-server `enabled: bool` (default `true`). A server with
/// `enabled: false` is skipped entirely — no connection, discovery, or tool
/// registration — while its config is retained for later reuse.
///   Docs:   https://hermes-agent.nousresearch.com/docs/reference/mcp-config-reference
///   Source: https://github.com/NousResearch/hermes-agent/blob/main/hermes_cli/mcp_config.py
pub fn set_hermes_mcp_enabled(
    config_path: &Path,
    name: &str,
    enabled: bool,
) -> Result<(), HkError> {
    modify_hermes_yaml(config_path, |root| {
        let servers = root
            .get_mut("mcp_servers")
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| HkError::ConfigCorrupted("mcp_servers is not a mapping".into()))?;
        let server = servers
            .get_mut(name)
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| HkError::NotFound(format!("MCP server '{name}' not found in config")))?;
        server.insert("enabled".into(), serde_yaml::Value::Bool(enabled));
        Ok(())
    })
}

/// Flip a Kiro MCP server's native `disabled` flag in place.
pub fn set_kiro_mcp_enabled(
    config_path: &Path,
    server_name: &str,
    enabled: bool,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let servers = config
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| HkError::NotFound("No mcpServers block found".into()))?;
        let server = servers
            .get_mut(server_name)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| HkError::NotFound(format!("MCP server '{server_name}' not found")))?;
        if enabled {
            server.remove("disabled");
        } else {
            server.insert("disabled".into(), serde_json::Value::Bool(true));
        }
        Ok(())
    })
}

/// Flip an omp MCP server's native per-entry `enabled` flag in place, then
/// scrub the user-level name list that would override the flag: on disable
/// the name is removed from `enabledServers` (the force-enable allowlist
/// overrides `enabled: false`), on enable from `disabledServers` (the
/// denylist overrides everything). Both lists live only in the *user*
/// mcp.json but gate servers from every source (omp mcp/config.ts), so
/// `user_config_path` differs from `entry_config_path` for project-scoped
/// servers.
///
/// The entry flag — not the denylist — carries the toggle so it stays scoped
/// to this one entry: `disabledServers` matches by NAME across all sources
/// and would knock out same-named servers in other projects.
pub fn set_omp_mcp_enabled(
    entry_config_path: &Path,
    user_config_path: &Path,
    server_name: &str,
    enabled: bool,
) -> Result<(), HkError> {
    locked_modify_json(entry_config_path, |config| {
        let servers = config
            .get_mut("mcpServers")
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| HkError::NotFound("No mcpServers block found".into()))?;
        let server = servers
            .get_mut(server_name)
            .and_then(|v| v.as_object_mut())
            .ok_or_else(|| HkError::NotFound(format!("MCP server '{server_name}' not found")))?;
        if enabled {
            // Absent means enabled (mcp-config.md: "skip when false").
            server.remove("enabled");
        } else {
            server.insert("enabled".into(), serde_json::Value::Bool(false));
        }
        Ok(())
    })?;
    // A missing user file can't contain the name — don't create one just to
    // scrub it.
    if !user_config_path.exists() {
        return Ok(());
    }
    let list_key = if enabled { "disabledServers" } else { "enabledServers" };
    locked_modify_json(user_config_path, |config| {
        if let Some(list) = config.get_mut(list_key).and_then(|v| v.as_array_mut()) {
            list.retain(|v| v.as_str() != Some(server_name));
        }
        Ok(())
    })
}

const DSH_BLOCK_BEGIN: &str = "# >>> managed by HarnessKit — do not edit this block >>>";
const DSH_BLOCK_END: &str = "# <<< managed by HarnessKit <<<";

/// Structured model of the HK-owned managed block at the end of the home
/// `cordis.patch.yml`. ONE engine serves the MCP toggle, the MCP insert
/// writer, and the plugin toggle — no second block format, no second
/// marker pair.
#[derive(Debug, Default)]
struct DshManagedBlock {
    /// Id-targeted `{id, disabled}` override entries. BTreeMap keeps the
    /// render order deterministic (sorted by row id).
    toggles: std::collections::BTreeMap<String, bool>,
    /// Full HK-authored insert ROWS in insertion order. Each renders as its
    /// own `- insert:` group holding exactly one row mapping, and each row
    /// is `{id, name, config}` (+ optional `disabled`) per the mcp-client
    /// schema. Toggling an HK-inserted server edits the `disabled` field of
    /// its own row — never a separate override entry.
    inserts: Vec<serde_yaml::Mapping>,
}

impl DshManagedBlock {
    fn is_empty(&self) -> bool {
        self.toggles.is_empty() && self.inserts.is_empty()
    }

    /// serverName of an insert row, but ONLY for mcp-client plugin rows —
    /// the same gate the reader applies, so every block-side matcher
    /// (find/remove/list) agrees with the reader's definition of an MCP row
    /// and can never match a non-MCP plugin insert.
    fn insert_server_name(row: &serde_yaml::Mapping) -> Option<&str> {
        if row.get("name")?.as_str()? != crate::adapter::dsh::MCP_CLIENT_PLUGIN {
            return None;
        }
        row.get("config")?.get("serverName")?.as_str()
    }

    fn find_insert_mut(&mut self, server_name: &str) -> Option<&mut serde_yaml::Mapping> {
        self.inserts
            .iter_mut()
            .find(|row| Self::insert_server_name(row) == Some(server_name))
    }

    fn remove_insert(&mut self, server_name: &str) -> bool {
        let before = self.inserts.len();
        self.inserts
            .retain(|row| Self::insert_server_name(row) != Some(server_name));
        self.inserts.len() != before
    }

    fn insert_row_ids(&self) -> Vec<String> {
        self.inserts
            .iter()
            .filter_map(|row| row.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect()
    }

    fn insert_server_names(&self) -> Vec<String> {
        self.inserts
            .iter()
            .filter_map(|row| Self::insert_server_name(row).map(String::from))
            .collect()
    }
}

/// Flip a dsh MCP server via the official patch-layer mechanism: an
/// id-targeted `disabled:` override inside an HK-owned marked block at the
/// END of the home-level `cordis.patch.yml` (the last always-applied user
/// layer — later entries win in dsh's single ordered apply).
///
/// Hard rules (upstream-verified):
/// - Only ever writes `home_patch` — NEVER `<profileDir>/cordis.yml` (dsh
///   overwrites that on boot) and never any profile's patch file.
/// - User bytes outside the markers are preserved; the sole structural edits
///   involve the `[]` empty-list placeholder (see render_dsh_patch).
/// - The edited text must re-parse as a YAML sequence, else nothing is
///   written (a broken file would make dsh keep last-good config and
///   silently ignore all future edits).
pub fn set_dsh_mcp_enabled(
    home_patch: &Path,
    server_name: &str,
    enabled: bool,
) -> Result<(), HkError> {
    use crate::adapter::dsh::DshAdapter;

    // The install writer stores the SANITIZED serverName; match it on lookup.
    let server_name = &normalize_dsh_server_name(server_name);

    let (user_text, mut block) = read_and_split_home_patch(home_patch)?;

    // An HK-inserted server (Task-8 install writer) is toggled by editing the
    // `disabled` field of its OWN insert row — no separate override entry.
    if let Some(row) = block.find_insert_mut(server_name) {
        if enabled {
            row.remove("disabled");
        } else {
            row.insert(serde_yaml::Value::from("disabled"), serde_yaml::Value::from(true));
        }
        return write_dsh_patch(home_patch, &user_text, &block);
    }

    let row_id = DshAdapter::mcp_row_id_in_text(&user_text, server_name).ok_or_else(|| {
        HkError::NotFound(format!(
            "MCP server '{server_name}' not found in {}",
            home_patch.display()
        ))
    })?;

    // Base state = the file WITHOUT our block.
    let base_enabled = DshAdapter::mcp_enabled_in_text(&user_text)
        .get(server_name)
        .copied()
        .unwrap_or(true);

    if base_enabled == enabled {
        block.toggles.remove(&row_id);
    } else {
        block.toggles.insert(row_id, !enabled); // value = disabled flag
    }

    write_dsh_patch(home_patch, &user_text, &block)
}

/// Flip a dsh PLUGIN row via the same official patch-layer mechanism as the
/// MCP toggle: an id-targeted `disabled:` override inside the HK block at
/// the end of the home `cordis.patch.yml`. The home layer is applied after
/// every profile's layer, so the WRITE is machine-global — the one override
/// affects that row id in every profile that contains it (upstream
/// precedent: dsh's own web-app bundle disables base rows exactly this way;
/// hot-reload applies it live). Accepted side effect: disabling a row that
/// exists only in profile A leaves the override "dangling" from profile B's
/// perspective — dsh warn-skips it per boot; upstream cosmetic noise, not
/// surfaced by HK.
///
/// The BASE state, by contrast, is per-profile: dsh boots ONE profile at a
/// time and composes only the layers of THAT profile plus the home patch
/// (upstream composeProfile), so a sibling profile's file is never loaded
/// alongside it and must never be folded in. `base_layers` is that profile's
/// ordered chain below the home patch — each mounted bundle's own patch file
/// in `bundles` order, then the profile's `cordis.patch.yml` — exactly what
/// the UI row carried as `PluginEntry::base_layers`. The fold is
/// `base_layers ++ [home user text]`, our own managed block stripped.
///
/// The chain, not just the defining layer: `hmr` is DEFINED by
/// `@deepseek-ai/dsh-base` (enabled) and DISABLED by
/// `@deepseek-ai/dsh-web-app` two layers later. Folding only the definition
/// would read `hmr` as enabled, so "enable" would match the base state, drop
/// the override, and silently leave the row disabled.
///
/// Bundle patch files are read-only inputs here — HK never writes any layer
/// but the home patch, and never `<profileDir>/cordis.yml` (dsh overwrites
/// that on boot).
///
/// `home_patch` is taken as a path, not derived from a dsh home, so that it
/// is the SAME value the adapter hands out in `base_layers` — the
/// `layer == home_patch` test below must compare like with like. A re-derived
/// path (trailing slash, symlinked `$DSH_HOME`) would compare unequal for a
/// home-defined row, send us down the re-read branch, and fold HK's own
/// managed block back in as base state — every toggle a silent no-op.
pub fn set_dsh_plugin_enabled(
    home_patch: &Path,
    row_id: &str,
    enabled: bool,
    base_layers: &[PathBuf],
) -> Result<(), HkError> {
    use crate::adapter::dsh::DshAdapter;

    let (user_text, mut block) = read_and_split_home_patch(home_patch)?;

    // Fold every layer below the home patch, then the home user text. The
    // home patch is read through read_and_split_home_patch above (block
    // stripped), so never re-read it here: folding our own managed block
    // back in would make every toggle look like the base state.
    let mut texts: Vec<String> = base_layers
        .iter()
        .filter(|layer| layer.as_path() != home_patch)
        .map(|layer| std::fs::read_to_string(layer).unwrap_or_default())
        .collect();
    texts.push(user_text.clone());
    let mut defined = false;
    let mut disabled_state: Option<bool> = None;
    for text in &texts {
        let (layer_defined, layer_state) = DshAdapter::plugin_row_state_in_text(text, row_id);
        defined |= layer_defined;
        if let Some(d) = layer_state {
            disabled_state = Some(d);
        }
    }
    if !defined {
        let layers = base_layers
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(HkError::NotFound(format!(
            "plugin row '{row_id}' is not defined by any of [{layers}] or {}",
            home_patch.display()
        )));
    }
    let base_enabled = !disabled_state.unwrap_or(false);

    if base_enabled == enabled {
        block.toggles.remove(row_id);
    } else {
        block.toggles.insert(row_id.to_string(), !enabled);
    }
    write_dsh_patch(home_patch, &user_text, &block)
}

/// serverName must satisfy mcp-client's `/^[A-Za-z0-9_-]{1,32}$/`
/// (source-verified: packages/mcp/mcp-client/src/index.ts). Map every other
/// char to `-`, cap at 32; a name with no valid alphanumeric at all errors.
fn sanitize_dsh_server_name(name: &str) -> Result<String, HkError> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .take(32)
        .collect();
    if !cleaned.chars().any(|c| c.is_ascii_alphanumeric()) {
        return Err(HkError::Validation(format!(
            "cannot derive a valid dsh serverName from '{name}' \
             (needs at least one of A-Za-z0-9; pattern [A-Za-z0-9_-], max 32 chars)"
        )));
    }
    Ok(cleaned)
}

/// Lookup-side normalization: identity for valid names; an unsanitizable
/// name is kept raw — it can't match any written row, so callers fall
/// through to "not found"/"no conflict". The install writer deliberately
/// does NOT use this (it must error on unsanitizable input).
pub(crate) fn normalize_dsh_server_name(name: &str) -> String {
    sanitize_dsh_server_name(name).unwrap_or_else(|_| name.to_string())
}

/// One full mcp-client insert row per the source-verified Config schema:
/// always an explicit `transport` discriminant and `serverName`; optional
/// keys only when non-empty (the schema defaults them). This is the ONLY
/// site that builds insert rows, so the key order (id, name, config) is
/// fixed here once (serde_yaml Mapping preserves insertion order — the
/// rendered byte format depends on it); env/header keys are sorted for the
/// same determinism.
///
/// Name round-trip: `serverName` must match mcp-client's
/// `/^[A-Za-z0-9_-]{1,32}$/`, so `microsoft/markitdown` is stored as
/// `microsoft-markitdown`. When sanitizing changed the name, the ORIGINAL is
/// recorded as `_hk_name` right after it so the reader can hand the scanner
/// the unsanitized name (`adapter::dsh::mcp_entries_in_text`). Without it the
/// scanner reads the row back under a different name and HK models it as a
/// SECOND extension — the ghost-duplicate-row bug. Same conditional as
/// Codex's `upsert_mcp_server_toml`: written ONLY when the name changed, so
/// already-valid names keep their exact previous bytes.
fn build_dsh_insert_row(
    row_id: &str,
    server_name: &str,
    entry: &McpServerEntry,
) -> serde_yaml::Mapping {
    use crate::adapter::dsh::HK_NAME_CONFIG_KEY;
    use serde_yaml::{Mapping, Value};
    // Placed immediately after `serverName` in both transport branches — the
    // key it qualifies.
    let insert_hk_name = |config: &mut Mapping| {
        if server_name != entry.name {
            config.insert(
                Value::from(HK_NAME_CONFIG_KEY),
                Value::from(entry.name.clone()),
            );
        }
    };
    let mut config = Mapping::new();
    if entry.transport == McpTransport::Stdio {
        config.insert(Value::from("transport"), Value::from("stdio"));
        config.insert(Value::from("serverName"), Value::from(server_name));
        insert_hk_name(&mut config);
        config.insert(Value::from("command"), Value::from(entry.command.clone()));
        if !entry.args.is_empty() {
            config.insert(
                Value::from("args"),
                Value::Sequence(entry.args.iter().map(|a| Value::from(a.clone())).collect()),
            );
        }
        if !entry.env.is_empty() {
            let mut env = Mapping::new();
            let mut keys: Vec<&String> = entry.env.keys().collect();
            keys.sort();
            for k in keys {
                env.insert(Value::from(k.clone()), Value::from(entry.env[k].clone()));
            }
            config.insert(Value::from("env"), Value::Mapping(env));
        }
    } else {
        // Both Http and (schema-rejected upstream of this fn) Sse spell the
        // written transport as streamable-http — dsh ships no SSE transport,
        // and validate_remote_mcp_target refuses Sse before this point once
        // the Task-9 remote schema lands (until then it refuses all remotes).
        config.insert(Value::from("transport"), Value::from("streamable-http"));
        config.insert(Value::from("serverName"), Value::from(server_name));
        insert_hk_name(&mut config);
        config.insert(
            Value::from("url"),
            Value::from(entry.url.clone().unwrap_or_default()),
        );
        if !entry.headers.is_empty() {
            let mut headers = Mapping::new();
            let mut keys: Vec<&String> = entry.headers.keys().collect();
            keys.sort();
            for k in keys {
                headers.insert(Value::from(k.clone()), Value::from(entry.headers[k].clone()));
            }
            config.insert(Value::from("headers"), Value::Mapping(headers));
        }
    }
    let mut row = Mapping::new();
    row.insert(Value::from("id"), Value::from(row_id));
    row.insert(
        Value::from("name"),
        Value::from(crate::adapter::dsh::MCP_CLIENT_PLUGIN),
    );
    row.insert(Value::from("config"), Value::Mapping(config));
    row
}

/// dsh MCP install: append a full `insert:` row (an mcp-client plugin row)
/// inside the HK managed block of the home `cordis.patch.yml`. Global scope
/// only — `mcp_config_path_for(Project)` stays `None` for dsh. User text
/// outside the block is byte-preserved, exactly as in the P0 toggle.
fn deploy_mcp_server_dsh_cordis(
    config_path: &Path,
    entry: &McpServerEntry,
) -> Result<(), HkError> {
    use crate::adapter::dsh::DshAdapter;

    let (user_text, mut block) = read_and_split_home_patch(config_path)?;

    let server_name = sanitize_dsh_server_name(&entry.name)?;
    // Collision domain is the STORED `serverName`, so re-installing the same
    // ORIGINAL name sanitizes to the same key and is caught here — one row,
    // never a silent second one (the `_hk_name` round-trip only affects what
    // the READER reports, never how rows are keyed).
    if DshAdapter::mcp_enabled_in_text(&user_text).contains_key(&server_name)
        || block.insert_server_names().contains(&server_name)
    {
        // Name the original input too when sanitizing changed it — the
        // caller may otherwise not recognize the colliding name as theirs.
        let from = if server_name == entry.name {
            String::new()
        } else {
            format!(" (from '{}')", entry.name)
        };
        return Err(HkError::Validation(format!(
            "dsh already has an MCP server named '{server_name}'{from} in {}",
            config_path.display()
        )));
    }
    // Generated row id: mcp-<server-name>, kebab. Collision with ANY existing
    // row id is an error — a duplicate id would make dsh treat the second
    // definition as a malformed collision. The id namespace spans every
    // layer dsh composes, not just this file: profile patches are applied
    // BEFORE the home patch, so a profile row with the same id collides just
    // as hard. Checked here, in the ONE place that generates ids.
    let row_id = format!("mcp-{}", server_name.to_lowercase().replace('_', "-"));
    let profile_row_ids: std::collections::HashSet<String> = config_path
        .parent()
        .map(DshAdapter::profile_patch_texts)
        .unwrap_or_default()
        .iter()
        .flat_map(|text| DshAdapter::row_ids_in_text(text))
        .collect();
    if DshAdapter::row_ids_in_text(&user_text).contains(&row_id)
        || block.toggles.contains_key(&row_id)
        || block.insert_row_ids().contains(&row_id)
        || profile_row_ids.contains(&row_id)
    {
        return Err(HkError::Validation(format!(
            "row id '{row_id}' already exists in the dsh patch layers of {} — \
             rename the server or the existing row",
            config_path.display()
        )));
    }
    block.inserts.push(build_dsh_insert_row(&row_id, &server_name, entry));
    write_dsh_patch(config_path, &user_text, &block)
}

/// dsh MCP removal: HK-inserted rows (inside the managed block) are removed;
/// user-authored rows keep the Validation refusal — HK never rewrites user
/// YAML. An absent name is a no-op, matching every other format.
fn remove_mcp_server_dsh_cordis(config_path: &Path, server_name: &str) -> Result<(), HkError> {
    use crate::adapter::dsh::DshAdapter;
    // The block stores the SANITIZED serverName; removal must map to it.
    let server_name = &normalize_dsh_server_name(server_name);
    let (user_text, mut block) = read_and_split_home_patch(config_path)?;
    if block.remove_insert(server_name) {
        return write_dsh_patch(config_path, &user_text, &block);
    }
    if DshAdapter::mcp_enabled_in_text(&user_text).contains_key(server_name) {
        return Err(HkError::Validation(format!(
            "'{server_name}' is a user-authored row in cordis.patch.yml; \
             HarnessKit never rewrites user YAML — remove the row in the file itself"
        )));
    }
    Ok(())
}

/// Shared prologue of every dsh home-patch writer: read the file, split off
/// the HK managed block, and name the file in any block-corruption error.
///
/// - Absent file = dsh has not seeded its template yet; start from the valid
///   empty form (`[]`). Any other IO error must surface, not be mistaken for
///   an empty file. The toggles never write this synthesized text — an empty
///   patch has no rows, so their row lookup fails first with "not found" —
///   while the install writer proceeds and creates the file, which is exactly
///   the desired first-install behavior. The remove writer relies on the
///   same synthesis for its idempotent no-op: an absent file has no rows, so
///   removal finds nothing and returns Ok instead of an IO error.
/// - The block parser is path-agnostic, so this call site owns naming the
///   file and the remediation hint on `ConfigCorrupted`.
fn read_and_split_home_patch(home_patch: &Path) -> Result<(String, DshManagedBlock), HkError> {
    let original = match std::fs::read_to_string(home_patch) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => "[]\n".to_string(),
        Err(e) => return Err(e.into()),
    };
    split_dsh_managed_block(&original).map_err(|e| match e {
        HkError::ConfigCorrupted(msg) => HkError::ConfigCorrupted(format!(
            "{msg} (in {}; fix or remove the content between the \
             '>>> managed by HarnessKit' markers)",
            home_patch.display()
        )),
        other => other,
    })
}

/// Split file text into (user text without the managed block, structured
/// block model). The block body is parsed as YAML: HK owns every byte
/// inside the markers, so content it would not itself render is a hard
/// `ConfigCorrupted` — refusing to write beats silently discarding block
/// entries. `split_inclusive` keeps user lines byte-exact (including CRLF).
///
/// Unbalanced markers are a hard `ConfigCorrupted` error: a BEGIN without a
/// matching END would otherwise swallow every user line to EOF (and the
/// rewritten file could still parse as a valid YAML sequence, so the
/// caller's post-edit guard would not catch the loss).
fn split_dsh_managed_block(text: &str) -> Result<(String, DshManagedBlock), HkError> {
    let mut user = String::new();
    let mut body = String::new();
    let mut in_block = false;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim();
        if line == DSH_BLOCK_BEGIN {
            if in_block {
                return Err(HkError::ConfigCorrupted(
                    "unbalanced HarnessKit managed-block markers: \
                     nested BEGIN marker inside the managed block"
                        .into(),
                ));
            }
            in_block = true;
            continue;
        }
        if line == DSH_BLOCK_END {
            if !in_block {
                return Err(HkError::ConfigCorrupted(
                    "unbalanced HarnessKit managed-block markers: \
                     END marker without a preceding BEGIN"
                        .into(),
                ));
            }
            in_block = false;
            continue;
        }
        if in_block {
            body.push_str(raw);
        } else {
            user.push_str(raw);
        }
    }
    if in_block {
        return Err(HkError::ConfigCorrupted(
            "unbalanced HarnessKit managed-block markers: \
             BEGIN marker without a matching END"
                .into(),
        ));
    }
    Ok((user, parse_dsh_block_body(&body)?))
}

/// Parse the marker-stripped block body into the structured model.
fn parse_dsh_block_body(body: &str) -> Result<DshManagedBlock, HkError> {
    let mut block = DshManagedBlock::default();
    if body.trim().is_empty() {
        return Ok(block);
    }
    let corrupted = |detail: String| {
        HkError::ConfigCorrupted(format!(
            "HarnessKit managed block in cordis.patch.yml is not valid: {detail}"
        ))
    };
    let doc: serde_yaml::Value =
        serde_yaml::from_str(body).map_err(|e| corrupted(e.to_string()))?;
    let Some(items) = doc.as_sequence() else {
        return Err(corrupted("block body is not a YAML list".into()));
    };
    // Entries must carry EXACTLY the keys HK itself renders. Extra keys in an
    // id-targeted entry are LIVE dsh patch semantics (they would patch the
    // target row), so silently dropping them on re-render would alter the
    // user's effective config — hard error instead.
    let extra_keys = |map: &serde_yaml::Mapping, allowed: &[&str]| -> String {
        map.keys()
            .map(|k| k.as_str().unwrap_or("<non-string key>").to_string())
            .filter(|k| !allowed.contains(&k.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for item in items {
        let Some(map) = item.as_mapping() else {
            return Err(corrupted("block entry is not a mapping".into()));
        };
        if let Some(rows) = map.get("insert").and_then(|v| v.as_sequence()) {
            if map.len() != 1 {
                return Err(corrupted(format!(
                    "insert group has keys besides insert: {}",
                    extra_keys(map, &["insert"])
                )));
            }
            for row in rows {
                let Some(rm) = row.as_mapping() else {
                    return Err(corrupted("insert row is not a mapping".into()));
                };
                block.inserts.push(rm.clone());
            }
            continue;
        }
        // `as_bool()` rejects `disabled: null`, which the READER
        // (adapter::dsh::yaml_disabled) accepts as `false` per upstream: HK
        // owns every byte between the markers and only ever writes literal
        // booleans, so a null in here means the block was hand-edited or
        // corrupted — refuse it rather than guess.
        match (
            map.get("id").and_then(|v| v.as_str()),
            map.get("disabled").and_then(|v| v.as_bool()),
        ) {
            (Some(id), Some(disabled)) => {
                if map.len() != 2 {
                    return Err(corrupted(format!(
                        "toggle entry has keys besides id/disabled: {}",
                        extra_keys(map, &["id", "disabled"])
                    )));
                }
                block.toggles.insert(id.to_string(), disabled);
            }
            _ => {
                return Err(corrupted(
                    "block entry is neither an {id, disabled} toggle nor an insert group".into(),
                ))
            }
        }
    }
    Ok(block)
}

/// Reassemble user text + managed block. Structural rules (unchanged from P0):
/// - Block present → any lone `[]` placeholder line is dropped (it can't
///   coexist with block-style entries in one document).
/// - Block absent → if the remaining text has no non-comment content, append
///   `[]` (an empty/comment-only patch file is a dsh boot error).
///
/// Rendering is deterministic: toggles sorted by id (BTreeMap) in the P0
/// byte format, then insert groups in insertion order with fixed key order
/// (serde_yaml Mapping preserves insertion order).
fn render_dsh_patch(user_text: &str, block: &DshManagedBlock) -> String {
    if block.is_empty() {
        let has_content = user_text
            .lines()
            .any(|l| !l.trim().is_empty() && !l.trim().starts_with('#') && l.trim() != "[]");
        let has_placeholder = user_text.lines().any(|l| l.trim() == "[]");
        if has_content || has_placeholder {
            return user_text.to_string();
        }
        let mut out = user_text.to_string();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[]\n");
        return out;
    }

    let mut body = String::new();
    for (id, disabled) in &block.toggles {
        body.push_str(&format!("- id: {id}\n  disabled: {disabled}\n"));
    }
    for row in &block.inserts {
        let mut group = serde_yaml::Mapping::new();
        group.insert(
            serde_yaml::Value::from("insert"),
            serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(row.clone())]),
        );
        let rendered = serde_yaml::to_string(&serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::Mapping(group),
        ]))
        .expect("HK-built YAML mapping always serializes");
        body.push_str(&rendered);
    }

    // Drop `[]` placeholder lines byte-preservingly (keep every other raw line).
    let mut out = String::new();
    for raw in user_text.split_inclusive('\n') {
        if raw.trim() != "[]" {
            out.push_str(raw);
        }
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(DSH_BLOCK_BEGIN);
    out.push('\n');
    out.push_str(&body);
    out.push_str(DSH_BLOCK_END);
    out.push('\n');
    out
}

/// Re-parse guard + atomic write shared by every dsh patch writer: the
/// edited text must stay a valid top-level YAML sequence, else nothing is
/// written (a broken file would make dsh keep last-good config and silently
/// ignore all future edits).
fn write_dsh_patch(
    path: &Path,
    user_text: &str,
    block: &DshManagedBlock,
) -> Result<(), HkError> {
    let new_text = render_dsh_patch(user_text, block);
    let parsed: Result<serde_yaml::Value, _> = serde_yaml::from_str(&new_text);
    if !matches!(parsed, Ok(serde_yaml::Value::Sequence(_))) {
        return Err(HkError::ConfigCorrupted(format!(
            "refusing to write {}: edited content is not a YAML list",
            path.display()
        )));
    }
    atomic_write(path, &new_text)
}

/// Flip a Kiro IDE hook's native `enabled` flag in place, keeping the entry
/// in the file — mirrors Kiro's own panel toggle ("skip without deleting").
pub fn set_kiro_hook_enabled(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    enabled: bool,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let hooks = config
            .get_mut("hooks")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| HkError::NotFound("No hooks array found".into()))?;
        let hook = hooks
            .iter_mut()
            .find(|h| kiro_hook_matches(h, event, matcher, command))
            .ok_or_else(|| HkError::NotFound(format!("Hook for '{event}' not found in config")))?;
        let obj = hook
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("hook is not an object".into()))?;
        obj.insert("enabled".into(), serde_json::Value::Bool(enabled));
        Ok(())
    })
}

fn kiro_hook_matches(
    hook: &serde_json::Value,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> bool {
    hook.get("trigger").and_then(|v| v.as_str()) == Some(event)
        && hook.get("matcher").and_then(|v| v.as_str()) == matcher
        && hook
            .get("action")
            .and_then(|v| v.get("type"))
            .and_then(|v| v.as_str())
            == Some("command")
        && hook
            .get("action")
            .and_then(|v| v.get("command"))
            .and_then(|v| v.as_str())
            == Some(command)
}

fn kiro_hook_value(entry: &HookEntry) -> serde_json::Value {
    let mut hook = serde_json::json!({
        "name": format!("{} {}", entry.event, entry.command),
        "trigger": entry.event,
        "action": { "type": "command", "command": entry.command },
    });
    if let Some(matcher) = &entry.matcher
        && let Some(obj) = hook.as_object_mut()
    {
        obj.insert("matcher".into(), serde_json::Value::String(matcher.clone()));
    }
    hook
}

/// True if a hooks-list item matches (matcher, command).
fn hermes_hook_item_matches(
    item: &serde_yaml::Value,
    matcher: Option<&str>,
    command: &str,
) -> bool {
    let item_cmd = item.get("command").and_then(|v| v.as_str());
    let item_matcher = item.get("matcher").and_then(|v| v.as_str());
    item_cmd == Some(command) && item_matcher == matcher
}

/// YAML-based hook deploy for Hermes (`~/.hermes/config.yaml`, root "hooks" key).
/// Upserts `{matcher?, command}` under `hooks.<event>` (a list), preserving the
/// rest of config.yaml. Deduplicates on (matcher, command).
fn deploy_hook_hermes_yaml(config_path: &Path, entry: &HookEntry) -> Result<(), HkError> {
    modify_hermes_yaml(config_path, |root| {
        let hooks = root
            .entry("hooks".into())
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("hooks is not a mapping".into()))?;
        let list = hooks
            .entry(entry.event.clone().into())
            .or_insert_with(|| serde_yaml::Value::Sequence(vec![]))
            .as_sequence_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("hook event is not a sequence".into()))?;
        if list
            .iter()
            .any(|i| hermes_hook_item_matches(i, entry.matcher.as_deref(), &entry.command))
        {
            return Ok(()); // dedup
        }
        let mut item = serde_yaml::Mapping::new();
        if let Some(m) = &entry.matcher {
            item.insert("matcher".into(), m.clone().into());
        }
        item.insert("command".into(), entry.command.clone().into());
        list.push(serde_yaml::Value::Mapping(item));
        Ok(())
    })
}

/// YAML-based hook remove for Hermes. Drops the matching `{matcher?, command}`
/// item from `hooks.<event>`; removes the event key entirely if it becomes empty.
fn remove_hook_hermes_yaml(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> Result<(), HkError> {
    if !config_path.exists() {
        return Ok(());
    }
    modify_hermes_yaml(config_path, |root| {
        let Some(hooks) = root.get_mut("hooks").and_then(|v| v.as_mapping_mut()) else {
            return Ok(());
        };
        if let Some(list) = hooks.get_mut(event).and_then(|v| v.as_sequence_mut()) {
            list.retain(|i| !hermes_hook_item_matches(i, matcher, command));
            if list.is_empty() {
                hooks.remove(event);
            }
        }
        Ok(())
    })
}

/// YAML-based hook restore for Hermes. Pushes the previously-saved entry (stored
/// as a `serde_json::Value` by `read_hook_config_hermes_yaml`) back under
/// `hooks.<event>`.
fn restore_hook_hermes_yaml(
    config_path: &Path,
    event: &str,
    entry: &serde_json::Value,
) -> Result<(), HkError> {
    let yaml_item: serde_yaml::Value =
        serde_yaml::to_value(entry).map_err(|e| HkError::Internal(e.to_string()))?;
    modify_hermes_yaml(config_path, |root| {
        let hooks = root
            .entry("hooks".into())
            .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()))
            .as_mapping_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("hooks is not a mapping".into()))?;
        let list = hooks
            .entry(event.to_string().into())
            .or_insert_with(|| serde_yaml::Value::Sequence(vec![]))
            .as_sequence_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("hook event is not a sequence".into()))?;
        list.push(yaml_item);
        Ok(())
    })
}

/// YAML-based hook read for Hermes. Returns the matching `hooks.<event>` item
/// converted to a `serde_json::Value` (mirrors the JSON formats' saved-entry type).
fn read_hook_config_hermes_yaml(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
) -> Result<Option<serde_json::Value>, HkError> {
    if !config_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(config_path)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
        HkError::ConfigCorrupted(format!("Failed to parse Hermes config.yaml: {e}"))
    })?;
    let Some(item) = doc
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|v| v.as_sequence())
        .and_then(|seq| {
            seq.iter()
                .find(|i| hermes_hook_item_matches(i, matcher, command))
        })
    else {
        return Ok(None);
    };
    let json_str = serde_json::to_string(item).map_err(|e| HkError::Internal(e.to_string()))?;
    let json_val = serde_json::from_str(&json_str).map_err(|e| HkError::Internal(e.to_string()))?;
    Ok(Some(json_val))
}

/// Build the `serde_json::Value` shape OpenCode's `McpLocalConfig` schema
/// expects for one server entry. Shared by `deploy_mcp_server_opencode`
/// (cross-agent install path) and intentionally also reachable as the
/// "regenerate from McpServerEntry" reference. Schema invariants are
/// documented at the parent function — keep them in sync.
fn build_opencode_mcp_value(entry: &McpServerEntry) -> serde_json::Value {
    let mut server_obj = serde_json::Map::new();
    if entry.transport != McpTransport::Stdio {
        // McpRemoteConfig: {type: "remote", url, headers?}
        let url = entry.url.clone().unwrap_or_default();
        server_obj.insert("type".into(), serde_json::Value::String("remote".into()));
        server_obj.insert("url".into(), serde_json::Value::String(url));
        if !entry.headers.is_empty() {
            server_obj.insert(
                "headers".into(),
                serde_json::Value::Object(
                    entry
                        .headers
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect(),
                ),
            );
        }
    } else {
        let mut command_array = vec![serde_json::Value::String(entry.command.clone())];
        command_array.extend(entry.args.iter().cloned().map(serde_json::Value::String));
        server_obj.insert("type".into(), serde_json::Value::String("local".into()));
        server_obj.insert("command".into(), serde_json::Value::Array(command_array));
        if !entry.env.is_empty() {
            server_obj.insert(
                "environment".into(),
                serde_json::Value::Object(
                    entry
                        .env
                        .iter()
                        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
                        .collect(),
                ),
            );
        }
    }
    serde_json::Value::Object(server_obj)
}

/// Deploy a hook config entry into the target agent's config file.
/// Reads the existing JSON, appends the hook under "hooks" -> event, writes back.
pub fn deploy_hook(
    config_path: &Path,
    entry: &HookEntry,
    format: HookFormat,
) -> Result<(), HkError> {
    if format == HookFormat::HermesYaml {
        return deploy_hook_hermes_yaml(config_path, entry);
    }
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                let hooks = config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(&entry.event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hook event is not an array".into()))?;

                let matcher_val = entry.matcher.as_deref().map(serde_json::Value::from);
                let group = arr.iter_mut().find(|h| {
                    h.get("matcher").and_then(|v| v.as_str()).map(String::from) == entry.matcher
                });
                // Use object format {"type":"command","command":"..."} — accepted by Claude, required by Codex/Gemini
                let cmd_obj = serde_json::json!({ "type": "command", "command": entry.command });
                if let Some(group) = group {
                    let cmds = group.as_object_mut().and_then(|o| {
                        o.entry("hooks")
                            .or_insert_with(|| serde_json::json!([]))
                            .as_array_mut()
                    });
                    if let Some(cmds) = cmds
                        && !cmds.iter().any(|c| {
                            c.get("command").and_then(|v| v.as_str()) == Some(&entry.command)
                        })
                    {
                        cmds.push(cmd_obj);
                    }
                } else {
                    let mut group = serde_json::json!({ "hooks": [cmd_obj] });
                    if let Some(m) = &matcher_val {
                        group
                            .as_object_mut()
                            .unwrap()
                            .insert("matcher".into(), m.clone());
                    }
                    arr.push(group);
                }
            }
            HookFormat::Cursor => {
                config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("version")
                    .or_insert(serde_json::json!(1));
                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(&entry.event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("event is not an array".into()))?;
                let hook_val = serde_json::json!({ "command": entry.command });
                if !arr.contains(&hook_val) {
                    arr.push(hook_val);
                }
            }
            HookFormat::Windsurf => {
                let hooks = config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(&entry.event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("event is not an array".into()))?;
                let hook_val = serde_json::json!({ "command": entry.command });
                if !arr.contains(&hook_val) {
                    arr.push(hook_val);
                }
            }
            HookFormat::Copilot => {
                config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("version")
                    .or_insert(serde_json::json!(1));
                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(&entry.event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("event is not an array".into()))?;
                let hook_val = serde_json::json!({ "type": "command", "command": entry.command });
                if !arr.contains(&hook_val) {
                    arr.push(hook_val);
                }
            }
            HookFormat::HermesYaml => {
                // Handled by the early return above; YAML is not JSON.
                unreachable!("HermesYaml handled before locked_modify_json")
            }
            HookFormat::KiroIde => {
                config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("version")
                    .or_insert(serde_json::json!("v1"));
                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!([]));
                let arr = hooks
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an array".into()))?;
                if !arr.iter().any(|h| {
                    kiro_hook_matches(h, &entry.event, entry.matcher.as_deref(), &entry.command)
                }) {
                    arr.push(kiro_hook_value(entry));
                }
            }
            HookFormat::None => {
                return Err(HkError::Internal("Agent does not support hooks".into()));
            }
        }
        Ok(())
    })
}

/// Remove an MCP server entry from a config file by name.
pub fn remove_mcp_server(
    config_path: &Path,
    server_name: &str,
    format: McpFormat,
) -> Result<(), HkError> {
    if !config_path.exists() {
        return Ok(());
    }
    match format {
        McpFormat::Toml => {
            let content = std::fs::read_to_string(config_path)?;
            let mut doc: toml::Table = content
                .parse::<toml::Table>()
                .map_err(|e| HkError::ConfigCorrupted(e.to_string()))?;
            if let Some(servers) = doc.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
                // Try original name first, then sanitized TOML key.
                if servers.remove(server_name).is_none() {
                    servers.remove(&sanitize_mcp_name(server_name));
                }
            }
            atomic_write(
                config_path,
                &toml::to_string_pretty(&doc).map_err(|e| HkError::Internal(e.to_string()))?,
            )?;
            Ok(())
        }
        McpFormat::Opencode => remove_mcp_server_opencode(config_path, server_name),
        McpFormat::HermesYaml => modify_hermes_yaml(config_path, |root| {
            if let Some(servers) = root.get_mut("mcp_servers").and_then(|v| v.as_mapping_mut()) {
                servers.remove(server_name);
            }
            Ok(())
        }),
        McpFormat::DshCordis => remove_mcp_server_dsh_cordis(config_path, server_name),
        _ => locked_modify_json(config_path, |config| {
            let key = json_top_key(format);
            if let Some(servers) = config.get_mut(key).and_then(|v| v.as_object_mut()) {
                servers.remove(server_name);
            }
            Ok(())
        }),
    }
}

/// Remove `server_name` from OpenCode's `mcp` block while preserving the
/// rest of the file verbatim (comments, formatting, sibling entries).
/// No-op if the server isn't present. Per the design decision in this PR,
/// any leading user-comments next to the removed entry stay in place — HK
/// never edits user comment text, only its own data entries.
fn remove_mcp_server_opencode(config_path: &Path, server_name: &str) -> Result<(), HkError> {
    locked_modify_jsonc(config_path, |root| {
        if let Some(mcp) = root.object_value("mcp")
            && let Some(prop) = mcp.get(server_name)
        {
            prop.remove();
        }
        Ok(())
    })
}

/// Remove a specific hook command from a config file by event, matcher, and command.
/// Only removes the given command from the group's hooks array.
/// If the hooks array becomes empty, removes the group.
/// If the event array becomes empty, removes the event key.
pub fn remove_hook(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    format: HookFormat,
) -> Result<(), HkError> {
    if format == HookFormat::HermesYaml {
        return remove_hook_hermes_yaml(config_path, event, matcher, command);
    }
    if !config_path.exists() {
        return Ok(());
    }
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut())
                {
                    for group in event_arr.iter_mut() {
                        let group_matcher = group.get("matcher").and_then(|v| v.as_str());
                        if group_matcher != matcher {
                            continue;
                        }
                        if let Some(cmds) = group.get_mut("hooks").and_then(|v| v.as_array_mut()) {
                            // Match both string format "cmd" and object format {"type":"command","command":"cmd"}
                            cmds.retain(|c| {
                                if c.as_str() == Some(command) {
                                    return false;
                                }
                                if c.get("command").and_then(|v| v.as_str()) == Some(command) {
                                    return false;
                                }
                                true
                            });
                        }
                    }
                    event_arr.retain(|h| {
                        h.get("hooks")
                            .and_then(|v| v.as_array())
                            .map(|a| !a.is_empty())
                            .unwrap_or(true)
                    });
                    if event_arr.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            HookFormat::Cursor => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut())
                {
                    let cmd_val = serde_json::json!({ "command": command });
                    event_arr.retain(|h| h != &cmd_val);
                    if event_arr.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            HookFormat::Windsurf => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut())
                {
                    event_arr.retain(|h| {
                        h.get("command").and_then(|v| v.as_str()) != Some(command)
                            && h.get("powershell").and_then(|v| v.as_str()) != Some(command)
                    });
                    if event_arr.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            HookFormat::Copilot => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut())
                {
                    event_arr
                        .retain(|h| h.get("command").and_then(|v| v.as_str()) != Some(command));
                    if event_arr.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            HookFormat::HermesYaml => {
                // Handled by the early return above; YAML is not JSON.
                unreachable!("HermesYaml handled before locked_modify_json")
            }
            HookFormat::KiroIde => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_array_mut()) {
                    hooks.retain(|h| !kiro_hook_matches(h, event, matcher, command));
                }
            }
            HookFormat::None => {
                return Err(HkError::Internal("Agent does not support hooks".into()));
            }
        }
        Ok(())
    })
}

/// Remove a plugin entry from a config file's enabledPlugins object by key.
pub fn remove_plugin_entry(config_path: &Path, plugin_key: &str) -> Result<(), HkError> {
    if !config_path.exists() {
        return Ok(());
    }
    locked_modify_json(config_path, |config| {
        if let Some(plugins) = config
            .get_mut("enabledPlugins")
            .and_then(|v| v.as_object_mut())
        {
            plugins.remove(plugin_key);
        }
        Ok(())
    })
}

/// Restore a previously disabled MCP server entry into the config file.
pub fn restore_mcp_server(
    config_path: &Path,
    server_name: &str,
    entry: &serde_json::Value,
    format: McpFormat,
) -> Result<(), HkError> {
    match format {
        McpFormat::Toml => {
            // Transcode the saved JSON snapshot back to TOML wholesale. The
            // snapshot is the raw on-disk table (read_mcp_server_config), so a
            // generic conversion preserves every key — url, http_headers, and
            // anything Codex adds later — where a field-by-field copy through
            // McpServerEntry silently dropped unknown ones.
            let toml_val: toml::Value = serde_json::from_value(entry.clone())
                .map_err(|e| HkError::ConfigCorrupted(format!("saved MCP snapshot: {e}")))?;
            upsert_mcp_server_toml(config_path, server_name, toml_val)
        }
        McpFormat::Opencode => restore_mcp_server_opencode(config_path, server_name, entry),
        McpFormat::HermesYaml => unreachable!(
            "Hermes MCP uses native in-place enable/disable (set_hermes_mcp_enabled); \
             the remove+snapshot+restore path is never reached for Hermes"
        ),
        McpFormat::DshCordis => unreachable!(
            "dsh MCP uses native in-place enable/disable (set_dsh_mcp_enabled); \
             the remove+snapshot+restore path is never reached for dsh"
        ),
        _ => {
            let key = json_top_key(format);
            locked_modify_json(config_path, |config| {
                let servers = config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry(key)
                    .or_insert_with(|| serde_json::json!({}));
                servers
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted(format!("{key} is not an object")))?
                    .insert(server_name.to_string(), entry.clone());
                Ok(())
            })
        }
    }
}

/// Restore a previously-saved entry into OpenCode's `mcp` block while
/// preserving the rest of the file verbatim. Creates the `mcp` block if
/// absent. Replaces an existing entry with the same name in place (rare
/// but possible if the user re-enables an entry that's also been
/// re-installed by another path).
fn restore_mcp_server_opencode(
    config_path: &Path,
    server_name: &str,
    entry: &serde_json::Value,
) -> Result<(), HkError> {
    locked_modify_jsonc(config_path, |root| {
        let mcp = root.object_value_or_set("mcp");
        let cst_value = to_cst_input(entry);
        if let Some(existing) = mcp.get(server_name) {
            existing.set_value(cst_value);
        } else {
            mcp.append(server_name, cst_value);
        }
        Ok(())
    })
}

/// Restore a previously disabled hook entry into the config file.
pub fn restore_hook(
    config_path: &Path,
    event: &str,
    entry: &serde_json::Value,
    format: HookFormat,
) -> Result<(), HkError> {
    if format == HookFormat::HermesYaml {
        return restore_hook_hermes_yaml(config_path, event, entry);
    }
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                let hooks = config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hook event is not an array".into()))?;
                arr.push(entry.clone());
            }
            HookFormat::Cursor | HookFormat::Copilot => {
                config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("version")
                    .or_insert(serde_json::json!(1));
                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hook event is not an array".into()))?;
                arr.push(entry.clone());
            }
            HookFormat::Windsurf => {
                let hooks = config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an object".into()))?
                    .entry(event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hook event is not an array".into()))?;
                arr.push(entry.clone());
            }
            HookFormat::HermesYaml => {
                // Handled by the early return above; YAML is not JSON.
                unreachable!("HermesYaml handled before locked_modify_json")
            }
            HookFormat::KiroIde => {
                config
                    .as_object_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
                    .entry("version")
                    .or_insert(serde_json::json!("v1"));
                let hooks = config
                    .as_object_mut()
                    .unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!([]));
                let arr = hooks
                    .as_array_mut()
                    .ok_or_else(|| HkError::ConfigCorrupted("hooks is not an array".into()))?;
                // Same (event, matcher, command) identity as deploy_hook, so a
                // double-restore doesn't duplicate the entry.
                let matcher = entry.get("matcher").and_then(|v| v.as_str());
                let command = entry
                    .get("action")
                    .and_then(|a| a.get("command"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if !arr
                    .iter()
                    .any(|h| kiro_hook_matches(h, event, matcher, command))
                {
                    arr.push(entry.clone());
                }
            }
            HookFormat::None => {
                return Err(HkError::Internal("Agent does not support hooks".into()));
            }
        }
        Ok(())
    })
}

/// Set enabledPlugins[plugin_key] to true or false (Claude native toggle).
pub fn set_plugin_enabled(
    config_path: &Path,
    plugin_key: &str,
    enabled: bool,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let plugins = config
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
            .entry("enabledPlugins")
            .or_insert_with(|| serde_json::json!({}));
        plugins
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("enabledPlugins is not an object".into()))?
            .insert(plugin_key.to_string(), serde_json::Value::Bool(enabled));
        Ok(())
    })
}

/// Set [plugins."plugin_key"] enabled = true/false in Codex config.toml.
/// Uses file locking to prevent concurrent read-modify-write races.
pub fn set_codex_plugin_enabled(
    config_path: &Path,
    plugin_key: &str,
    enabled: bool,
) -> Result<(), HkError> {
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(config_path)?;
    file.lock_exclusive()?;

    let mut content = String::new();
    (&file).read_to_string(&mut content)?;
    let mut doc: toml::Table = if content.is_empty() {
        toml::Table::new()
    } else {
        content
            .parse::<toml::Table>()
            .map_err(|e| HkError::ConfigCorrupted(e.to_string()))?
    };
    let plugins = doc
        .entry("plugins")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| HkError::ConfigCorrupted("plugins is not a table".into()))?;
    let entry = plugins
        .entry(plugin_key)
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| HkError::ConfigCorrupted("plugin entry is not a table".into()))?;
    entry.insert("enabled".into(), toml::Value::Boolean(enabled));

    let output = toml::to_string_pretty(&doc).map_err(|e| HkError::Internal(e.to_string()))?;
    (&file).seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    (&file).write_all(output.as_bytes())?;
    (&file).flush()?;

    file.unlock()?;
    Ok(())
}

/// Remove a [plugins."plugin_key"] entry from Codex config.toml.
pub fn remove_codex_plugin_entry(config_path: &Path, plugin_key: &str) -> Result<(), HkError> {
    if !config_path.exists() {
        return Ok(());
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(config_path)?;
    file.lock_exclusive()?;

    let mut content = String::new();
    (&file).read_to_string(&mut content)?;
    let mut doc: toml::Table = content
        .parse::<toml::Table>()
        .map_err(|e| HkError::ConfigCorrupted(e.to_string()))?;

    if let Some(plugins) = doc.get_mut("plugins").and_then(|v| v.as_table_mut()) {
        plugins.remove(plugin_key);
    }

    let output = toml::to_string_pretty(&doc).map_err(|e| HkError::Internal(e.to_string()))?;
    (&file).seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    (&file).write_all(output.as_bytes())?;
    (&file).flush()?;

    file.unlock()?;
    Ok(())
}

/// Set VS Code agent plugin enablement in state.vscdb.
/// Reads the current `agentPlugins.enablement` array, updates the entry for the
/// given plugin URI, and writes it back. Creates the entry if it doesn't exist.
pub fn set_vscode_plugin_enabled(
    vscode_user_dir: &Path,
    plugin_uri: &str,
    enabled: bool,
) -> Result<(), HkError> {
    let db_path = vscode_user_dir.join("globalStorage").join("state.vscdb");
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| HkError::Internal(format!("Failed to open VS Code state DB: {}", e)))?;

    // Read current enablement array
    let current: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());

    let mut entries: Vec<(String, bool)> = serde_json::from_str(&current).unwrap_or_default();

    // Update or insert the entry
    let mut found = false;
    for entry in &mut entries {
        if entry.0 == plugin_uri {
            entry.1 = enabled;
            found = true;
            break;
        }
    }
    if !found {
        entries.push((plugin_uri.to_string(), enabled));
    }

    let new_value =
        serde_json::to_string(&entries).map_err(|e| HkError::Internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO ItemTable (key, value) VALUES ('agentPlugins.enablement', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![new_value],
    )
    .map_err(|e| HkError::Internal(format!("Failed to update VS Code state DB: {}", e)))?;

    Ok(())
}

/// Remove a plugin entry from VS Code's state.vscdb enablement array.
pub fn remove_vscode_plugin_entry(vscode_user_dir: &Path, plugin_uri: &str) -> Result<(), HkError> {
    let db_path = vscode_user_dir.join("globalStorage").join("state.vscdb");
    if !db_path.exists() {
        return Ok(());
    }
    let conn = rusqlite::Connection::open(&db_path)
        .map_err(|e| HkError::Internal(format!("Failed to open VS Code state DB: {}", e)))?;

    let current: String = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());

    let mut entries: Vec<(String, bool)> = serde_json::from_str(&current).unwrap_or_default();

    entries.retain(|e| e.0 != plugin_uri);

    let new_value =
        serde_json::to_string(&entries).map_err(|e| HkError::Internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO ItemTable (key, value) VALUES ('agentPlugins.enablement', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![new_value],
    )
    .map_err(|e| HkError::Internal(format!("Failed to update VS Code state DB: {}", e)))?;

    Ok(())
}

/// Set Gemini extension enablement in extension-enablement.json.
/// Updates only the user-scope rule (`{homedir}/*`) and preserves workspace-scope rules.
pub fn set_gemini_extension_enabled(
    extensions_dir: &Path,
    extension_name: &str,
    enabled: bool,
    home: &Path,
) -> Result<(), HkError> {
    let home_str = home.to_string_lossy();
    let enable_rule = format!("{}/*", home_str);
    let disable_rule = format!("!{}/*", home_str);

    modify_gemini_enablement(extensions_dir, |config| {
        let entry = config
            .entry(extension_name.to_string())
            .or_insert_with(|| serde_json::json!({"overrides": []}));
        let overrides = entry
            .get_mut("overrides")
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| HkError::ConfigCorrupted("overrides is not an array".into()))?;

        // Remove existing user-scope rules (both enable and disable)
        overrides.retain(|v| {
            let s = v.as_str().unwrap_or("");
            s != enable_rule && s != disable_rule
        });

        // Add the new rule
        let rule = if enabled { &enable_rule } else { &disable_rule };
        overrides.push(serde_json::Value::String(rule.to_string()));
        Ok(())
    })
}

/// Remove an extension entry from Gemini's extension-enablement.json.
pub fn remove_gemini_extension_entry(
    extensions_dir: &Path,
    extension_name: &str,
) -> Result<(), HkError> {
    let enablement_path = extensions_dir.join("extension-enablement.json");
    if !enablement_path.exists() {
        return Ok(());
    }
    modify_gemini_enablement(extensions_dir, |config| {
        config.remove(extension_name);
        Ok(())
    })
}

/// Locked read-modify-write for extension-enablement.json.
fn modify_gemini_enablement(
    extensions_dir: &Path,
    modify: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>) -> Result<(), HkError>,
) -> Result<(), HkError> {
    let enablement_path = extensions_dir.join("extension-enablement.json");
    if let Some(parent) = enablement_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&enablement_path)?;
    file.lock_exclusive()?;

    let mut content = String::new();
    (&file).read_to_string(&mut content)?;
    let mut config: serde_json::Map<String, serde_json::Value> = if content.is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str(&content)
            .map_err(|e| HkError::ConfigCorrupted(format!("extension-enablement.json: {}", e)))?
    };

    modify(&mut config)?;

    let output =
        serde_json::to_string_pretty(&config).map_err(|e| HkError::Internal(e.to_string()))?;
    (&file).seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    (&file).write_all(output.as_bytes())?;
    (&file).flush()?;

    file.unlock()?;
    Ok(())
}

/// Restore a previously disabled plugin entry into enabledPlugins.
pub fn restore_plugin_entry(
    config_path: &Path,
    plugin_key: &str,
    value: &serde_json::Value,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let plugins = config
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
            .entry("enabledPlugins")
            .or_insert_with(|| serde_json::json!({}));
        plugins
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("enabledPlugins is not an object".into()))?
            .insert(plugin_key.to_string(), value.clone());
        Ok(())
    })
}

// NOTE: `ensure_codex_hooks_enabled` used to live here, writing
// `[features] hooks = true` into ~/.codex/config.toml on hook deploy. It was
// removed: Codex hooks are enabled by default (the flag is a DISABLE switch,
// per https://developers.openai.com/codex/hooks), so the write was redundant
// and would trample an explicit user `hooks = false` opt-out. Hook execution
// is gated by project trust + per-hook `/hooks` review on the Codex side.

/// Read an MCP server entry's full JSON value from a config file.
pub fn read_mcp_server_config(
    config_path: &Path,
    server_name: &str,
    format: McpFormat,
) -> Result<Option<serde_json::Value>, HkError> {
    if !config_path.exists() {
        return Ok(None);
    }
    match format {
        McpFormat::Toml => {
            let content = std::fs::read_to_string(config_path)?;
            let doc: toml::Table = content
                .parse::<toml::Table>()
                .map_err(|e| HkError::ConfigCorrupted(e.to_string()))?;
            // Try the original name first, then the sanitized TOML key.
            // The scanner uses `_hk_name` to recover the original name, so
            // callers pass the original while the TOML key is sanitized.
            let safe_name = sanitize_mcp_name(server_name);
            let server = doc
                .get("mcp_servers")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get(server_name).or_else(|| t.get(&safe_name)));
            // Convert TOML value to JSON for uniform storage in DB
            match server {
                Some(val) => {
                    let json_str = serde_json::to_string(&val)?;
                    let json_val: serde_json::Value = serde_json::from_str(&json_str)?;
                    Ok(Some(json_val))
                }
                None => Ok(None),
            }
        }
        McpFormat::Opencode => read_mcp_server_config_opencode(config_path, server_name),
        McpFormat::HermesYaml => unreachable!(
            "Hermes MCP uses native in-place enable/disable (set_hermes_mcp_enabled); \
             the read-config-for-snapshot path is never reached for Hermes"
        ),
        McpFormat::DshCordis => unreachable!(
            "dsh MCP uses native in-place enable/disable (set_dsh_mcp_enabled); \
             the read-config-for-snapshot path is never reached for dsh"
        ),
        _ => {
            let config = read_or_create_json(config_path)?;
            let key = json_top_key(format);
            Ok(config.get(key).and_then(|v| v.get(server_name)).cloned())
        }
    }
}

/// Read a single OpenCode MCP entry's value as `serde_json::Value`. Tolerant
/// of jsonc syntax (`//` comments, trailing commas) since OpenCode's loader
/// accepts the same superset for both `opencode.json` and `opencode.jsonc`.
/// Returns `None` if the file lacks `mcp` or that specific entry. Read-only,
/// no advisory lock — locks would only matter if we were modifying.
fn read_mcp_server_config_opencode(
    config_path: &Path,
    server_name: &str,
) -> Result<Option<serde_json::Value>, HkError> {
    use jsonc_parser::cst::CstRootNode;
    let content = std::fs::read_to_string(config_path)?;
    if content.is_empty() {
        return Ok(None);
    }
    let cst = CstRootNode::parse(&content, &Default::default())
        .map_err(|e| HkError::ConfigCorrupted(format!("Failed to parse jsonc: {e}")))?;
    let Some(root) = cst.object_value() else {
        return Ok(None);
    };
    let Some(prop) = root
        .object_value("mcp")
        .and_then(|mcp| mcp.get(server_name))
    else {
        return Ok(None);
    };
    Ok(prop.to_serde_value())
}

/// Read a hook entry's full JSON value from a config file.
pub fn read_hook_config(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    format: HookFormat,
) -> Result<Option<serde_json::Value>, HkError> {
    if format == HookFormat::HermesYaml {
        return read_hook_config_hermes_yaml(config_path, event, matcher, command);
    }
    if !config_path.exists() {
        return Ok(None);
    }
    let config = read_or_create_json(config_path)?;
    if format == HookFormat::KiroIde {
        let Some(hooks) = config.get("hooks").and_then(|v| v.as_array()) else {
            return Ok(None);
        };
        return Ok(hooks
            .iter()
            .find(|entry| kiro_hook_matches(entry, event, matcher, command))
            .cloned());
    }
    let hooks = config.get("hooks").and_then(|v| v.as_object());
    let Some(hooks) = hooks else {
        return Ok(None);
    };
    let Some(event_arr) = hooks.get(event).and_then(|v| v.as_array()) else {
        return Ok(None);
    };
    match format {
        HookFormat::ClaudeLike => {
            for group in event_arr {
                let group_matcher = group.get("matcher").and_then(|v| v.as_str());
                if group_matcher != matcher {
                    continue;
                }
                if let Some(cmds) = group.get("hooks").and_then(|v| v.as_array())
                    && cmds.iter().any(|c| {
                        // Match both string format "cmd" and object format {"command":"cmd"}
                        c.as_str() == Some(command)
                            || c.get("command").and_then(|v| v.as_str()) == Some(command)
                    })
                {
                    return Ok(Some(group.clone()));
                }
            }
            Ok(None)
        }
        HookFormat::Cursor => {
            let cmd_val = serde_json::json!({ "command": command });
            for entry in event_arr {
                if entry == &cmd_val {
                    return Ok(Some(entry.clone()));
                }
            }
            Ok(None)
        }
        HookFormat::Windsurf => {
            for entry in event_arr {
                if entry.get("command").and_then(|v| v.as_str()) == Some(command)
                    || entry.get("powershell").and_then(|v| v.as_str()) == Some(command)
                {
                    return Ok(Some(entry.clone()));
                }
            }
            Ok(None)
        }
        HookFormat::Copilot => {
            for entry in event_arr {
                if entry.get("command").and_then(|v| v.as_str()) == Some(command) {
                    return Ok(Some(entry.clone()));
                }
            }
            Ok(None)
        }
        HookFormat::KiroIde => Ok(None),
        // Handled by the early return above; YAML is not JSON.
        HookFormat::HermesYaml => Ok(None),
        HookFormat::None => Ok(None),
    }
}

/// Read a plugin entry's value from enabledPlugins in a config file.
pub fn read_plugin_config(
    config_path: &Path,
    plugin_key: &str,
) -> Result<Option<serde_json::Value>, HkError> {
    if !config_path.exists() {
        return Ok(None);
    }
    let config = read_or_create_json(config_path)?;
    Ok(config
        .get("enabledPlugins")
        .and_then(|v| v.get(plugin_key))
        .cloned())
}

fn read_or_create_json(path: &Path) -> Result<serde_json::Value, HkError> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(serde_json::json!({}))
    }
}

#[allow(dead_code)]
fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), HkError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

/// Write content to a file atomically: write to a temp file, then rename.
fn atomic_write(path: &Path, content: &str) -> Result<(), HkError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Convert a `serde_json::Value` into the `CstInputValue` shape that
/// `jsonc-parser`'s CST mutation API expects. Used by OpenCode write paths
/// to feed existing serde-shaped entries (read off McpServerEntry, restored
/// off SQLite undo log, etc.) through CST `append` / `set_value`.
///
/// Note on key ordering: `serde_json::Value::Object` maps to
/// `serde_json::Map`, which is alphabetically sorted unless the
/// `preserve_order` feature is enabled (it isn't, here). New entries
/// therefore land with alphabetized keys — the same behavior as the
/// existing `to_string_pretty` path, so this isn't a regression.
fn to_cst_input(v: &serde_json::Value) -> jsonc_parser::cst::CstInputValue {
    use jsonc_parser::cst::CstInputValue;
    match v {
        serde_json::Value::Null => CstInputValue::Null,
        serde_json::Value::Bool(b) => CstInputValue::Bool(*b),
        serde_json::Value::Number(n) => CstInputValue::Number(n.to_string()),
        serde_json::Value::String(s) => CstInputValue::String(s.clone()),
        serde_json::Value::Array(arr) => {
            CstInputValue::Array(arr.iter().map(to_cst_input).collect())
        }
        serde_json::Value::Object(obj) => CstInputValue::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), to_cst_input(v)))
                .collect(),
        ),
    }
}

/// Read-modify-write a jsonc-flavored config file with an exclusive advisory
/// file lock, preserving comments and formatting outside the modified area.
///
/// Mirrors `locked_modify_json`'s lock-and-rewrite semantics (no rename, so
/// the advisory lock isn't dropped mid-write), but parses with the CST API
/// instead of `serde_json::Value`. The closure receives the root `CstObject`
/// and operates on it via `get` / `append` / `object_value_or_set` / etc.
/// Comments and whitespace surrounding unmodified entries are kept verbatim.
///
/// Used today only by OpenCode write paths — both `opencode.json` and
/// `opencode.jsonc` flow through here. Other agents' formats stay on
/// `locked_modify_json` (strict JSON), so the CST dependency only loads
/// when a McpFormat::Opencode dispatch lands here.
fn locked_modify_jsonc<F>(path: &Path, modify: F) -> Result<(), HkError>
where
    F: FnOnce(&jsonc_parser::cst::CstObject) -> Result<(), HkError>,
{
    use jsonc_parser::cst::CstRootNode;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock_exclusive()?;

    let mut content = String::new();
    (&file).read_to_string(&mut content)?;
    // Empty file → seed with "{}" so CstRootNode::parse always sees an
    // object root. Avoids bailing on a freshly-created config file whose
    // first write would otherwise be the root entry itself.
    let seed = if content.is_empty() {
        "{}"
    } else {
        content.as_str()
    };

    let cst = CstRootNode::parse(seed, &Default::default())
        .map_err(|e| HkError::ConfigCorrupted(format!("Failed to parse jsonc: {e}")))?;
    // Fail fast if root is non-object (e.g. user wrote `[1,2,3]` at top
    // level). `object_value_or_set` would silently destroy the array — we
    // refuse to do that.
    let root_obj = cst
        .object_value()
        .ok_or_else(|| HkError::ConfigCorrupted("Config root is not an object".into()))?;

    modify(&root_obj)?;

    let output = cst.to_string();
    (&file).seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    (&file).write_all(output.as_bytes())?;
    (&file).flush()?;

    file.unlock()?;
    Ok(())
}

/// Read-modify-write a JSON config file with an exclusive advisory file lock.
fn locked_modify_json<F>(path: &Path, modify: F) -> Result<(), HkError>
where
    F: FnOnce(&mut serde_json::Value) -> Result<(), HkError>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.lock_exclusive()?;

    let mut content = String::new();
    (&file).read_to_string(&mut content)?;
    let mut config: serde_json::Value = if content.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(&content)?
    };

    modify(&mut config)?;

    let output = serde_json::to_string_pretty(&config)?;
    (&file).seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    (&file).write_all(output.as_bytes())?;
    (&file).flush()?;

    file.unlock()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// A representative adapter for each MCP format, so deploy tests can
    /// exercise `deploy_mcp_server`'s real dispatch (format + remote schema).
    fn test_adapter(format: McpFormat) -> Box<dyn crate::adapter::AgentAdapter> {
        use crate::adapter::*;
        let home = std::path::PathBuf::from("/nonexistent");
        match format {
            McpFormat::McpServers => Box::new(claude::ClaudeAdapter::with_home(home)),
            McpFormat::Servers => Box::new(copilot::CopilotAdapter::with_home(home)),
            McpFormat::Toml => Box::new(codex::CodexAdapter::with_home(home)),
            McpFormat::Opencode => Box::new(opencode::OpencodeAdapter::with_home(home)),
            McpFormat::HermesYaml => Box::new(hermes::HermesAdapter::with_home(home)),
            McpFormat::DshCordis => Box::new(dsh::DshAdapter::with_home(home)),
        }
    }

    fn remote_entry(transport: McpTransport) -> McpServerEntry {
        McpServerEntry {
            name: "linear".into(),
            transport,
            url: Some("https://mcp.linear.app/mcp".into()),
            headers: [("Authorization".to_string(), "Bearer tok".to_string())].into(),
            ..Default::default()
        }
    }

    #[test]
    fn build_mcp_json_value_spells_each_remote_schema() {
        let http = remote_entry(McpTransport::Http);
        let sse = remote_entry(McpTransport::Sse);

        let v = build_mcp_json_value(&http, RemoteMcpSchema::TypeAndUrl).unwrap();
        assert_eq!(v["type"], "http");
        assert_eq!(v["url"], "https://mcp.linear.app/mcp");
        assert_eq!(v["headers"]["Authorization"], "Bearer tok");
        assert!(v.get("command").is_none());
        assert_eq!(
            build_mcp_json_value(&sse, RemoteMcpSchema::TypeAndUrl).unwrap()["type"],
            "sse"
        );

        let v = build_mcp_json_value(&http, RemoteMcpSchema::PlainUrl).unwrap();
        assert_eq!(v["url"], "https://mcp.linear.app/mcp");
        assert!(v.get("type").is_none());

        // Gemini spells the transport through the key itself.
        let v = build_mcp_json_value(&http, RemoteMcpSchema::GeminiSplit).unwrap();
        assert_eq!(v["httpUrl"], "https://mcp.linear.app/mcp");
        assert!(v.get("url").is_none());
        let v = build_mcp_json_value(&sse, RemoteMcpSchema::GeminiSplit).unwrap();
        assert_eq!(v["url"], "https://mcp.linear.app/mcp");
        assert!(v.get("httpUrl").is_none());

        let v = build_mcp_json_value(&http, RemoteMcpSchema::ServerUrl).unwrap();
        assert_eq!(v["serverUrl"], "https://mcp.linear.app/mcp");
    }

    #[test]
    fn validate_remote_mcp_target_rejects_unsupported_combinations() {
        let http = remote_entry(McpTransport::Http);
        let sse = remote_entry(McpTransport::Sse);

        let err = validate_remote_mcp_target(&http, "someagent", RemoteMcpSchema::Unsupported)
            .unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("someagent")));

        // Codex (TOML) is HTTP-only.
        let err = validate_remote_mcp_target(&sse, "codex", RemoteMcpSchema::Toml).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("not SSE")));
        validate_remote_mcp_target(&http, "codex", RemoteMcpSchema::Toml).unwrap();

        // Remote without url is corrupt regardless of target.
        let mut broken = remote_entry(McpTransport::Http);
        broken.url = None;
        let err = validate_remote_mcp_target(&broken, "claude", RemoteMcpSchema::TypeAndUrl)
            .unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
    }

    #[test]
    fn deploy_remote_mcp_json_end_to_end_per_agent_spelling() {
        // Full deploy_mcp_server dispatch (validate + format + remote schema)
        // for the JSON-family agents, not just the value builder.
        let dir = TempDir::new().unwrap();

        // Claude (TypeAndUrl): {type, url, headers} under mcpServers.
        let config = dir.path().join("claude.json");
        let entry = remote_entry(McpTransport::Http);
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::McpServers)).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &doc["mcpServers"]["linear"];
        assert_eq!(server["type"], "http");
        assert_eq!(server["url"], "https://mcp.linear.app/mcp");
        assert_eq!(server["headers"]["Authorization"], "Bearer tok");
        assert!(server.get("command").is_none());

        // Gemini (GeminiSplit): SSE entries land under `url`, not `httpUrl`.
        let config = dir.path().join("gemini.json");
        let sse = remote_entry(McpTransport::Sse);
        let gemini = crate::adapter::gemini::GeminiAdapter::with_home("/nonexistent".into());
        deploy_mcp_server(&config, &sse, &gemini).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &doc["mcpServers"]["linear"];
        assert_eq!(server["url"], "https://mcp.linear.app/mcp");
        assert!(server.get("httpUrl").is_none());
        assert!(server.get("type").is_none());

        // Windsurf (ServerUrl): single serverUrl key regardless of protocol.
        let config = dir.path().join("windsurf.json");
        let windsurf = crate::adapter::windsurf::WindsurfAdapter::with_home("/nonexistent".into());
        deploy_mcp_server(&config, &entry, &windsurf).unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &doc["mcpServers"]["linear"];
        assert_eq!(server["serverUrl"], "https://mcp.linear.app/mcp");
        assert!(server.get("url").is_none());
    }

    #[test]
    fn deploy_remote_mcp_hermes_yaml_writes_url_headers_and_transport() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.yaml");
        let mut entry = remote_entry(McpTransport::Sse);
        entry.name = "stripe".into();
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::HermesYaml)).unwrap();

        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &doc["mcp_servers"]["stripe"];
        assert_eq!(server["url"].as_str(), Some("https://mcp.linear.app/mcp"));
        assert_eq!(server["transport"].as_str(), Some("sse"));
        assert_eq!(
            server["headers"]["Authorization"].as_str(),
            Some("Bearer tok")
        );
        assert!(server.get("command").is_none());
    }

    #[test]
    fn deploy_remote_mcp_opencode_writes_remote_type() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        std::fs::write(&config, "{}").unwrap();
        let entry = remote_entry(McpTransport::Http);
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Opencode)).unwrap();

        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &doc["mcp"]["linear"];
        assert_eq!(server["type"], "remote");
        assert_eq!(server["url"], "https://mcp.linear.app/mcp");
        assert_eq!(server["headers"]["Authorization"], "Bearer tok");
        assert!(server.get("command").is_none());
    }

    #[test]
    fn toml_disable_enable_roundtrip_preserves_remote_fields() {
        // The old restore path narrowed snapshots to command/args/env,
        // destroying url/http_headers on re-enable.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let entry = remote_entry(McpTransport::Http);
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        let snapshot = read_mcp_server_config(&config, "linear", McpFormat::Toml)
            .unwrap()
            .unwrap();
        remove_mcp_server(&config, "linear", McpFormat::Toml).unwrap();
        restore_mcp_server(&config, "linear", &snapshot, McpFormat::Toml).unwrap();

        let doc: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let restored = doc["mcp_servers"]["linear"].as_table().unwrap();
        assert_eq!(
            restored["url"].as_str(),
            Some("https://mcp.linear.app/mcp"),
            "url must survive disable→enable"
        );
        assert_eq!(
            restored["http_headers"]["Authorization"].as_str(),
            Some("Bearer tok")
        );
        assert!(!restored.contains_key("command"));
    }

    // ----- jsonc / OpenCode-specific tests -----
    // Helper tests pin `locked_modify_jsonc` round-trip and edge cases.
    // End-to-end tests pin the public MCP API (deploy/remove/restore)
    // through `McpFormat::Opencode` for the comment-preservation contract.

    #[test]
    fn locked_modify_jsonc_round_trip_preserves_comments() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("opencode.jsonc");
        let original = "{\n  // hello\n  \"a\": 1, // trailing line comment\n  \"b\": [1, 2,], /* block */\n}\n";
        std::fs::write(&path, original).unwrap();

        locked_modify_jsonc(&path, |_root| Ok(())).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }

    #[test]
    fn locked_modify_jsonc_appends_into_mcp_keeping_neighbor_comments() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("opencode.jsonc");
        std::fs::write(
            &path,
            "{\n  // top note\n  \"model\": \"x\",\n  \"mcp\": {\n    // about github\n    \"github\": {\"type\": \"local\", \"command\": [\"a\"]}\n  }\n}\n",
        )
        .unwrap();

        locked_modify_jsonc(&path, |root| {
            let mcp = root.object_value_or_set("mcp");
            mcp.append(
                "filesystem",
                to_cst_input(&serde_json::json!({"type": "local", "command": ["b"]})),
            );
            Ok(())
        })
        .unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("// top note"), "top-level comment dropped");
        assert!(
            written.contains("// about github"),
            "mcp child comment dropped"
        );
        assert!(written.contains("\"github\""), "existing entry lost");
        assert!(written.contains("\"filesystem\""), "appended entry missing");
    }

    #[test]
    fn locked_modify_jsonc_rejects_non_object_root() {
        // Refuse to silently overwrite a top-level array — better to error
        // than to destroy data. Mirrors locked_modify_json's behavior.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("weird.jsonc");
        std::fs::write(&path, "[1, 2, 3]").unwrap();

        let err = locked_modify_jsonc(&path, |_| Ok(()));
        assert!(matches!(err, Err(HkError::ConfigCorrupted(_))));
        // File untouched.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[1, 2, 3]");
    }

    #[test]
    fn locked_modify_jsonc_seeds_empty_file_with_object() {
        // First-time write to an empty/non-existent file: seed with `{}`
        // so the helper has a valid object root to operate on.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("fresh.jsonc");

        locked_modify_jsonc(&path, |root| {
            root.append("mcp", to_cst_input(&serde_json::json!({})));
            Ok(())
        })
        .unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("\"mcp\""));
        let _: serde_json::Value = serde_json::from_str(&written).unwrap();
    }

    #[test]
    fn test_remove_mcp_server_opencode_preserves_comments() {
        // End-to-end: remove only the targeted entry; surrounding user
        // comments and sibling entries stay verbatim. The comment that
        // was directly above the removed entry stays as an "orphan" by
        // design — HK never edits user comment text.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.jsonc");
        std::fs::write(
            &config,
            "{\n  // top note\n  \"model\": \"x\",\n  \"mcp\": {\n    // about github\n    \"github\": {\"type\": \"local\", \"command\": [\"a\"]},\n    // about filesystem\n    \"filesystem\": {\"type\": \"local\", \"command\": [\"b\"]}\n  }\n}\n",
        )
        .unwrap();

        remove_mcp_server(&config, "github", McpFormat::Opencode).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(written.contains("// top note"));
        assert!(
            written.contains("// about filesystem"),
            "sibling comment dropped"
        );
        assert!(written.contains("\"filesystem\""), "sibling entry lost");
        assert!(!written.contains("\"github\""), "target entry not removed");
    }

    #[test]
    fn test_restore_mcp_server_opencode_preserves_comments() {
        // End-to-end: restoring a previously-saved entry into mcp keeps
        // every other comment, formatting, and sibling intact. Mirrors
        // the HK toggle flow (disable → restore).
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.jsonc");
        std::fs::write(
            &config,
            "{\n  // top note\n  \"model\": \"x\",\n  \"mcp\": {\n    // about github\n    \"github\": {\"type\": \"local\", \"command\": [\"a\"]}\n  }\n}\n",
        )
        .unwrap();

        let saved = serde_json::json!({"type": "local", "command": ["b"]});
        restore_mcp_server(&config, "filesystem", &saved, McpFormat::Opencode).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(written.contains("// top note"));
        assert!(written.contains("// about github"));
        assert!(written.contains("\"github\""), "existing entry lost");
        assert!(written.contains("\"filesystem\""), "restored entry missing");
    }

    #[test]
    fn test_deploy_mcp_server_opencode_preserves_comments() {
        // End-to-end guarantee for the deploy path (cross-agent install
        // into OpenCode): existing user comments and formatting outside
        // the touched mcp entry survive intact.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.jsonc");
        std::fs::write(
            &config,
            "{\n  // top note kept\n  \"model\": \"claude-opus-4\",\n  \"mcp\": {\n    // about github\n    \"github\": {\"type\": \"local\", \"command\": [\"existing\"]}\n  }\n}\n",
        )
        .unwrap();

        let entry = McpServerEntry {
            name: "filesystem".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@mcp/fs".into()],
            env: std::collections::HashMap::new(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Opencode)).unwrap();

        let written = std::fs::read_to_string(&config).unwrap();
        assert!(
            written.contains("// top note kept"),
            "top-level comment dropped"
        );
        assert!(
            written.contains("// about github"),
            "mcp child comment dropped"
        );
        assert!(written.contains("\"github\""), "existing entry lost");
        assert!(written.contains("\"filesystem\""), "deployed entry missing");
        assert!(
            written.contains("\"npx\""),
            "deployed entry's command missing"
        );
    }

    // ----- existing tests below -----

    #[test]
    fn test_deploy_skill_directory() {
        let src_dir = TempDir::new().unwrap();
        let skill_dir = src_dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill").unwrap();
        std::fs::write(skill_dir.join("helper.py"), "print('hello')").unwrap();

        let target_dir = TempDir::new().unwrap();
        let name = deploy_skill(&skill_dir, target_dir.path()).unwrap();
        assert_eq!(name, "my-skill");
        assert!(target_dir.path().join("my-skill").join("SKILL.md").exists());
        assert!(
            target_dir
                .path()
                .join("my-skill")
                .join("helper.py")
                .exists()
        );
    }

    #[test]
    fn test_deploy_skill_file() {
        let src_dir = TempDir::new().unwrap();
        let skill_file = src_dir.path().join("solo-skill.md");
        std::fs::write(&skill_file, "# Solo Skill").unwrap();

        let target_dir = TempDir::new().unwrap();
        let name = deploy_skill(&skill_file, target_dir.path()).unwrap();
        assert_eq!(name, "solo-skill.md");
        assert!(target_dir.path().join("solo-skill.md").exists());
    }

    #[test]
    fn test_deploy_skill_skips_git_dir() {
        let src_dir = TempDir::new().unwrap();
        let skill_dir = src_dir.path().join("git-skill");
        std::fs::create_dir_all(skill_dir.join(".git")).unwrap();
        std::fs::write(skill_dir.join(".git").join("HEAD"), "ref: refs/heads/main").unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Git Skill").unwrap();

        let target_dir = TempDir::new().unwrap();
        deploy_skill(&skill_dir, target_dir.path()).unwrap();
        assert!(
            target_dir
                .path()
                .join("git-skill")
                .join("SKILL.md")
                .exists()
        );
        assert!(!target_dir.path().join("git-skill").join(".git").exists());
    }

    #[test]
    fn test_deploy_mcp_server_new_file() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("mcp.json");
        let entry = McpServerEntry {
            name: "github".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            env: [("GITHUB_TOKEN".into(), "ghp_test".into())].into(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::McpServers)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &content["mcpServers"]["github"];
        assert_eq!(server["command"], "npx");
        assert_eq!(server["args"][0], "-y");
        assert_eq!(server["env"]["GITHUB_TOKEN"], "ghp_test");
    }

    #[test]
    fn test_deploy_mcp_server_existing_file() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(
            &config,
            r#"{"theme":"dark","mcpServers":{"existing":{"command":"node"}}}"#,
        )
        .unwrap();

        let entry = McpServerEntry {
            name: "new-server".into(),
            command: "python".into(),
            args: vec!["server.py".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::McpServers)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["theme"], "dark"); // preserved
        assert_eq!(content["mcpServers"]["existing"]["command"], "node"); // preserved
        assert_eq!(content["mcpServers"]["new-server"]["command"], "python"); // added
    }

    #[test]
    fn test_deploy_mcp_server_servers_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("mcp.json");
        let entry = McpServerEntry {
            name: "memory".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Servers)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(
            content.get("mcpServers").is_none(),
            "should not use mcpServers key"
        );
        let server = &content["servers"]["memory"];
        assert_eq!(server["command"], "npx");
    }

    #[test]
    fn test_deploy_mcp_server_toml_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        // Existing TOML content to preserve
        std::fs::write(&config, "model = \"o4-mini\"\n").unwrap();

        let entry = McpServerEntry {
            name: "context7".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@upstash/context7-mcp".into()],
            env: [("MY_KEY".into(), "val".into())].into(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        let content = std::fs::read_to_string(&config).unwrap();
        let doc: toml::Table = content.parse().unwrap();
        assert_eq!(doc["model"].as_str().unwrap(), "o4-mini"); // preserved
        let server = doc["mcp_servers"]["context7"].as_table().unwrap();
        assert_eq!(server["command"].as_str().unwrap(), "npx");
        assert_eq!(
            server["args"].as_array().unwrap()[0].as_str().unwrap(),
            "-y"
        );
        assert_eq!(server["env"]["MY_KEY"].as_str().unwrap(), "val");
    }

    #[test]
    fn test_deploy_mcp_server_opencode_format() {
        // OpenCode schema (https://opencode.ai/config.json):
        //   - top-level key "mcp"
        //   - entry must declare type: "local"
        //   - command is a single array merging the binary + its args
        //   - env block is named "environment"
        //   - additionalProperties: false → no separate "args"/"env" fields
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        let entry = McpServerEntry {
            name: "github".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            env: [("GITHUB_TOKEN".into(), "ghp_test".into())].into(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Opencode)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();

        assert!(
            content.get("mcpServers").is_none(),
            "must not use the Claude-style mcpServers key"
        );
        let server = &content["mcp"]["github"];
        assert_eq!(server["type"], "local");
        assert_eq!(server["command"][0], "npx");
        assert_eq!(server["command"][1], "-y");
        assert_eq!(server["command"][2], "@modelcontextprotocol/server-github");
        assert_eq!(server["environment"]["GITHUB_TOKEN"], "ghp_test");
        // additionalProperties: false is enforced upstream — verify we honor it.
        assert!(
            server.get("args").is_none(),
            "must not emit a separate args field"
        );
        assert!(
            server.get("env").is_none(),
            "must use 'environment', not 'env'"
        );
    }

    #[test]
    fn test_deploy_mcp_server_opencode_omits_environment_when_empty() {
        // Schema marks `environment` optional. Emitting `"environment": {}` is
        // legal but noisy; we omit the field entirely when the source has no
        // env vars to keep the on-disk config minimal.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        let entry = McpServerEntry {
            name: "memory".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-memory".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Opencode)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &content["mcp"]["memory"];
        assert_eq!(server["type"], "local");
        assert!(server["command"].is_array());
        assert!(
            server.get("environment").is_none(),
            "should omit environment field when source has no env vars"
        );
    }

    #[test]
    fn test_deploy_mcp_server_opencode_preserves_existing_keys() {
        // OpenCode's opencode.json holds many top-level keys (model, agent,
        // skills, etc.). Deploy must merge into the existing "mcp" object
        // without clobbering siblings or sibling-format settings.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        std::fs::write(
            &config,
            r#"{"model":"claude-sonnet-4-6","mcp":{"existing":{"type":"local","command":["node","s.js"]}}}"#,
        )
        .unwrap();

        let entry = McpServerEntry {
            name: "added".into(),
            command: "python".into(),
            args: vec!["server.py".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Opencode)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["model"], "claude-sonnet-4-6"); // sibling preserved
        assert_eq!(content["mcp"]["existing"]["command"][0], "node"); // sibling entry preserved
        assert_eq!(content["mcp"]["added"]["command"][0], "python"); // new entry added
    }

    #[test]
    fn test_opencode_remove_restore_and_read_uses_mcp_key() {
        // Exercise the three json_top_key code paths (remove/restore/read) for
        // McpFormat::Opencode in one round-trip. Regression guard: an earlier
        // implementation routed Opencode through the wildcard arm and silently
        // operated on "mcpServers" instead of "mcp".
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        std::fs::write(
            &config,
            r#"{"mcp":{"github":{"type":"local","command":["npx","server-github"],"environment":{"TOKEN":"abc"}}}}"#,
        )
        .unwrap();

        // read
        let saved = read_mcp_server_config(&config, "github", McpFormat::Opencode).unwrap();
        assert!(
            saved.is_some(),
            "read must find entry under 'mcp', not 'mcpServers'"
        );
        let saved = saved.unwrap();
        assert_eq!(saved["environment"]["TOKEN"], "abc");

        // remove
        remove_mcp_server(&config, "github", McpFormat::Opencode).unwrap();
        let after_remove = read_mcp_server_config(&config, "github", McpFormat::Opencode).unwrap();
        assert!(after_remove.is_none(), "remove must delete from 'mcp' key");

        // restore
        restore_mcp_server(&config, "github", &saved, McpFormat::Opencode).unwrap();
        let restored = read_mcp_server_config(&config, "github", McpFormat::Opencode).unwrap();
        assert_eq!(
            restored.unwrap(),
            saved,
            "restored entry must match what was saved (bit-perfect round-trip)"
        );

        // Confirm the entry actually lives under "mcp" on disk.
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(content.get("mcp").is_some());
        assert!(
            content.get("mcpServers").is_none(),
            "must not have leaked into mcpServers via fallback"
        );
    }

    #[test]
    fn test_opencode_deploy_then_adapter_read_roundtrip() {
        // Cross-module integration: bytes deployer writes must be exactly what
        // the OpencodeAdapter's parser reads back — i.e. a McpServerEntry
        // survives a full write→read loop with command/args/env intact.
        use crate::adapter::AgentAdapter;
        use crate::adapter::opencode::OpencodeAdapter;

        let dir = TempDir::new().unwrap();
        let config = dir.path().join("opencode.json");
        let original = McpServerEntry {
            name: "context7".into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@upstash/context7-mcp".into()],
            env: [("API_KEY".into(), "k1".into())].into(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &original, &*test_adapter(McpFormat::Opencode)).unwrap();

        let adapter = OpencodeAdapter::with_home(dir.path().to_path_buf());
        let entries = adapter.read_mcp_servers_from(&config);
        assert_eq!(entries.len(), 1);
        let read_back = &entries[0];
        assert_eq!(read_back.name, original.name);
        assert_eq!(read_back.command, original.command);
        assert_eq!(read_back.args, original.args);
        assert_eq!(read_back.env, original.env);
    }

    #[test]
    fn test_sanitize_mcp_name_replaces_slash() {
        assert_eq!(
            sanitize_mcp_name("microsoft/markitdown"),
            "microsoft-markitdown"
        );
    }

    #[test]
    fn test_sanitize_mcp_name_preserves_valid_chars() {
        assert_eq!(sanitize_mcp_name("my_server-1"), "my_server-1");
    }

    #[test]
    fn test_deploy_mcp_server_toml_sanitizes_name_and_preserves_original() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let entry = McpServerEntry {
            name: "microsoft/markitdown".into(),
            command: "uvx".into(),
            args: vec!["markitdown-mcp@0.0.1a4".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        let doc: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let servers = doc["mcp_servers"].as_table().unwrap();
        // TOML key should be sanitized: "/" → "-"
        assert!(servers.contains_key("microsoft-markitdown"));
        assert!(!servers.contains_key("microsoft/markitdown"));
        // Original name preserved in _hk_name for scanner round-trip
        let server = servers["microsoft-markitdown"].as_table().unwrap();
        assert_eq!(server["_hk_name"].as_str().unwrap(), "microsoft/markitdown");
    }

    #[test]
    fn test_deploy_mcp_server_toml_no_hk_name_when_unchanged() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let entry = McpServerEntry {
            name: "context7".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        let doc: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let server = doc["mcp_servers"]["context7"].as_table().unwrap();
        // No _hk_name needed when name didn't require sanitization
        assert!(!server.contains_key("_hk_name"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_resolve_command_path_absolute_passthrough() {
        // Already absolute paths should be returned unchanged.
        assert_eq!(resolve_command_path("/usr/bin/env"), "/usr/bin/env");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_resolve_command_path_resolves_known_command() {
        // "ls" should resolve to an absolute path on any Unix system.
        let resolved = resolve_command_path("ls");
        assert!(
            resolved.starts_with('/'),
            "expected absolute path, got: {resolved}"
        );
    }

    #[test]
    fn test_resolve_command_path_unknown_fallback() {
        // Non-existent command should return the original string.
        assert_eq!(
            resolve_command_path("__nonexistent_cmd_12345__"),
            "__nonexistent_cmd_12345__"
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_build_path_for_command_includes_parent_dir() {
        let path = build_path_for_command("/Users/zoe/.nvm/versions/node/v24.13.0/bin/npx");
        assert_eq!(
            path.unwrap(),
            "/Users/zoe/.nvm/versions/node/v24.13.0/bin:/usr/local/bin:/usr/bin:/bin"
        );
    }

    #[test]
    fn test_build_path_for_command_bare_name_returns_none() {
        // Bare command name (no directory) should return None.
        assert!(build_path_for_command("npx").is_none());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_resolve_command_path_absolute_passthrough_windows() {
        assert_eq!(
            resolve_command_path(r"C:\Windows\System32\cmd.exe"),
            r"C:\Windows\System32\cmd.exe"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_resolve_command_path_resolves_known_command_windows() {
        let resolved = resolve_command_path("cmd");
        assert!(
            crate::sanitize::is_windows_abs_path(&resolved),
            "expected absolute path, got: {resolved}"
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_build_path_for_command_includes_parent_dir_windows() {
        let path = build_path_for_command(r"C:\Users\test\AppData\Local\Programs\node\npx.exe");
        assert_eq!(
            path.unwrap(),
            r"C:\Users\test\AppData\Local\Programs\node;C:\Windows\System32;C:\Windows"
        );
    }

    #[test]
    fn test_read_mcp_server_config_toml_finds_sanitized_key() {
        // When the TOML key is sanitized ("microsoft-markitdown") but the caller
        // uses the original name ("microsoft/markitdown"), the lookup should still work.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let entry = McpServerEntry {
            name: "microsoft/markitdown".into(),
            command: "uvx".into(),
            args: vec!["markitdown-mcp@0.0.1a4".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        // Read using the original (unsanitized) name
        let result =
            read_mcp_server_config(&config, "microsoft/markitdown", McpFormat::Toml).unwrap();
        assert!(result.is_some(), "should find entry via original name");
        assert_eq!(result.unwrap()["command"], "uvx");
    }

    #[test]
    fn test_remove_mcp_server_toml_removes_sanitized_key() {
        // remove_mcp_server should find and remove the sanitized TOML key
        // when called with the original name.
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let entry = McpServerEntry {
            name: "microsoft/markitdown".into(),
            command: "uvx".into(),
            args: vec!["markitdown-mcp@0.0.1a4".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        // Remove using the original name
        remove_mcp_server(&config, "microsoft/markitdown", McpFormat::Toml).unwrap();

        // Verify it's gone
        let result =
            read_mcp_server_config(&config, "microsoft/markitdown", McpFormat::Toml).unwrap();
        assert!(result.is_none(), "entry should be removed");
    }

    #[test]
    fn test_mcp_toml_disable_enable_roundtrip_with_sanitized_name() {
        // Full roundtrip: deploy → read → remove (disable) → restore (enable)
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");
        let original_name = "microsoft/markitdown";

        // 1. Deploy with a name that needs sanitization
        let entry = McpServerEntry {
            name: original_name.into(),
            command: "uvx".into(),
            args: vec!["markitdown-mcp@0.0.1a4".into()],
            env: Default::default(),
            enabled: true,
            ..Default::default()
        };
        deploy_mcp_server(&config, &entry, &*test_adapter(McpFormat::Toml)).unwrap();

        // 2. Read (for saving before disable) — using original name
        let saved = read_mcp_server_config(&config, original_name, McpFormat::Toml)
            .unwrap()
            .expect("should read entry");

        // 3. Remove (disable) — using original name
        remove_mcp_server(&config, original_name, McpFormat::Toml).unwrap();
        assert!(
            read_mcp_server_config(&config, original_name, McpFormat::Toml)
                .unwrap()
                .is_none(),
            "entry should be gone after disable"
        );

        // 4. Restore (enable) — using original name
        restore_mcp_server(&config, original_name, &saved, McpFormat::Toml).unwrap();
        let restored = read_mcp_server_config(&config, original_name, McpFormat::Toml)
            .unwrap()
            .expect("should be restored");
        assert_eq!(restored["command"], "uvx");
    }

    #[test]
    fn test_deploy_hook_new_file() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        let entry = HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("Bash".into()),
            command: "echo test".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::ClaudeLike).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hook = &content["hooks"]["PreToolUse"][0];
        assert_eq!(hook["matcher"], "Bash");
        // Now writes object format: {"type":"command","command":"echo test"}
        assert_eq!(hook["hooks"][0]["type"], "command");
        assert_eq!(hook["hooks"][0]["command"], "echo test");
    }

    #[test]
    fn test_deploy_hook_appends_to_existing_group() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        // Existing hook in old string format
        std::fs::write(
            &config,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo first"]}]}}"#,
        )
        .unwrap();

        let entry = HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("Bash".into()),
            command: "echo second".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::ClaudeLike).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"][0]["hooks"]
            .as_array()
            .unwrap();
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0], "echo first"); // old string entry preserved
        assert_eq!(hooks[1]["command"], "echo second"); // new entry in object format
    }

    #[test]
    fn test_deploy_hook_no_duplicate_command() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        // Existing hook in object format
        std::fs::write(&config, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo test"}]}]}}"#).unwrap();

        let entry = HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("Bash".into()),
            command: "echo test".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::ClaudeLike).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"][0]["hooks"]
            .as_array()
            .unwrap();
        assert_eq!(hooks.len(), 1); // not duplicated
    }

    #[test]
    fn test_restore_mcp_server() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"mcpServers":{}}"#).unwrap();

        let entry_json = r#"{"command":"npx","args":["-y","@mcp/github"],"env":{"TOKEN":"abc"}}"#;
        let entry: serde_json::Value = serde_json::from_str(entry_json).unwrap();
        restore_mcp_server(&config, "github", &entry, McpFormat::McpServers).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["mcpServers"]["github"]["command"], "npx");
        assert_eq!(content["mcpServers"]["github"]["env"]["TOKEN"], "abc");
    }

    #[test]
    fn test_restore_hook() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"hooks":{}}"#).unwrap();

        let entry = serde_json::json!({"matcher": "Bash", "hooks": ["echo test"]});
        restore_hook(&config, "PreToolUse", &entry, HookFormat::ClaudeLike).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["hooks"]["PreToolUse"][0]["matcher"], "Bash");
        assert_eq!(content["hooks"]["PreToolUse"][0]["hooks"][0], "echo test");
    }

    #[test]
    fn test_restore_plugin_entry() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"enabledPlugins":{}}"#).unwrap();

        restore_plugin_entry(&config, "my-plugin@source", &serde_json::json!(true)).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["enabledPlugins"]["my-plugin@source"], true);
    }

    #[test]
    fn test_read_mcp_server_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y"]}}}"#,
        )
        .unwrap();

        let entry = read_mcp_server_config(&config, "github", McpFormat::McpServers).unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["command"], "npx");

        let missing =
            read_mcp_server_config(&config, "nonexistent", McpFormat::McpServers).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_read_hook_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(
            &config,
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo test"]}]}}"#,
        )
        .unwrap();

        let entry = read_hook_config(
            &config,
            "PreToolUse",
            Some("Bash"),
            "echo test",
            HookFormat::ClaudeLike,
        )
        .unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["matcher"], "Bash");

        let missing = read_hook_config(
            &config,
            "PreToolUse",
            Some("Bash"),
            "nonexistent",
            HookFormat::ClaudeLike,
        )
        .unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_read_hook_config_windsurf_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(
            &config,
            r#"{"hooks":{"post_cascade_response":[{"powershell":"python C:\\hooks\\log.py"}]}}"#,
        )
        .unwrap();

        let entry = read_hook_config(
            &config,
            "post_cascade_response",
            None,
            "python C:\\hooks\\log.py",
            HookFormat::Windsurf,
        )
        .unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["powershell"], "python C:\\hooks\\log.py");
    }

    #[test]
    fn test_read_plugin_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"enabledPlugins":{"my-plugin@source":true}}"#).unwrap();

        let entry = read_plugin_config(&config, "my-plugin@source").unwrap();
        assert_eq!(entry.unwrap(), serde_json::json!(true));
    }

    #[test]
    fn test_remove_and_restore_mcp_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{"github":{"command":"npx","args":["-y"],"env":{}}}}"#,
        )
        .unwrap();

        // Read, remove, restore
        let saved = read_mcp_server_config(&config, "github", McpFormat::McpServers)
            .unwrap()
            .unwrap();
        remove_mcp_server(&config, "github", McpFormat::McpServers).unwrap();

        let after_remove: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(after_remove["mcpServers"].get("github").is_none());

        restore_mcp_server(&config, "github", &saved, McpFormat::McpServers).unwrap();
        let after_restore: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after_restore["mcpServers"]["github"]["command"], "npx");
    }

    #[test]
    fn test_deploy_hook_cursor_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        let entry = HookEntry {
            event: "stop".into(),
            matcher: None,
            command: "echo done".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::Cursor).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["version"], 1);
        assert_eq!(content["hooks"]["stop"][0]["command"], "echo done");
        // Should NOT have matcher or nested hooks array
        assert!(content["hooks"]["stop"][0].get("matcher").is_none());
        assert!(content["hooks"]["stop"][0].get("hooks").is_none());
    }

    #[test]
    fn test_deploy_hook_copilot_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        let entry = HookEntry {
            event: "PreToolUse".into(),
            matcher: None,
            command: "./check.sh".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::Copilot).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["version"], 1);
        assert_eq!(content["hooks"]["PreToolUse"][0]["type"], "command");
        assert_eq!(content["hooks"]["PreToolUse"][0]["command"], "./check.sh");
    }

    #[test]
    fn test_deploy_hook_windsurf_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        let entry = HookEntry {
            event: "pre_user_prompt".into(),
            matcher: None,
            command: "echo hi".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::Windsurf).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(content.get("version").is_none());
        assert_eq!(content["hooks"]["pre_user_prompt"][0]["command"], "echo hi");
    }

    #[test]
    fn test_kiro_ide_hook_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("lint.json");
        let entry = HookEntry {
            event: "PostFileSave".into(),
            matcher: Some("\\.ts$".into()),
            command: "npm run lint".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::KiroIde).unwrap();
        let deployed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        // Kiro only documents "v1" (https://kiro.dev/docs/hooks/); other values
        // may make Kiro skip the file entirely.
        assert_eq!(deployed["version"], "v1");
        let saved = read_hook_config(
            &config,
            "PostFileSave",
            Some("\\.ts$"),
            "npm run lint",
            HookFormat::KiroIde,
        )
        .unwrap()
        .expect("Kiro hook should be readable");
        assert_eq!(saved["action"]["type"], "command");
        assert_eq!(saved["action"]["command"], "npm run lint");

        remove_hook(
            &config,
            "PostFileSave",
            Some("\\.ts$"),
            "npm run lint",
            HookFormat::KiroIde,
        )
        .unwrap();
        let removed: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(removed["hooks"].as_array().unwrap().len(), 0);

        restore_hook(&config, "PostFileSave", &saved, HookFormat::KiroIde).unwrap();
        let restored = read_hook_config(
            &config,
            "PostFileSave",
            Some("\\.ts$"),
            "npm run lint",
            HookFormat::KiroIde,
        )
        .unwrap();
        assert!(restored.is_some());
    }

    #[test]
    fn test_kiro_ide_restore_hook_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("lint.json");
        let entry = serde_json::json!({
            "name": "lint-on-save",
            "trigger": "PostFileSave",
            "matcher": "\\.ts$",
            "action": { "type": "command", "command": "npm run lint" },
        });
        restore_hook(&config, "PostFileSave", &entry, HookFormat::KiroIde).unwrap();
        restore_hook(&config, "PostFileSave", &entry, HookFormat::KiroIde).unwrap();
        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(
            content["hooks"].as_array().unwrap().len(),
            1,
            "double restore must not duplicate the hook"
        );
    }

    #[test]
    fn test_set_kiro_mcp_enabled_flips_disabled() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("mcp.json");
        std::fs::write(
            &config,
            r#"{"mcpServers":{"github":{"command":"npx","args":["server"]}}}"#,
        )
        .unwrap();
        set_kiro_mcp_enabled(&config, "github", false).unwrap();
        let disabled: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(disabled["mcpServers"]["github"]["disabled"], true);

        set_kiro_mcp_enabled(&config, "github", true).unwrap();
        let enabled: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(enabled["mcpServers"]["github"].get("disabled").is_none());
    }

    #[test]
    fn test_set_omp_mcp_enabled_flips_flag_and_scrubs_lists() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("mcp.json");
        // Entry currently force-enabled via the allowlist; a stale denylist
        // entry for another server must survive untouched.
        std::fs::write(
            &config,
            r#"{
              "mcpServers": {"github": {"type": "http", "url": "https://example.com/mcp", "enabled": false}},
              "enabledServers": ["github"],
              "disabledServers": ["other"]
            }"#,
        )
        .unwrap();

        // Disable: entry flag set, name scrubbed from the allowlist (which
        // would otherwise override enabled:false), entry keys preserved.
        set_omp_mcp_enabled(&config, &config, "github", false).unwrap();
        let disabled: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(disabled["mcpServers"]["github"]["enabled"], false);
        assert_eq!(disabled["mcpServers"]["github"]["type"], "http");
        assert_eq!(disabled["mcpServers"]["github"]["url"], "https://example.com/mcp");
        assert!(disabled["enabledServers"].as_array().unwrap().is_empty());
        assert_eq!(disabled["disabledServers"][0], "other");

        // Enable: flag removed (absent means enabled).
        set_omp_mcp_enabled(&config, &config, "github", true).unwrap();
        let enabled: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(enabled["mcpServers"]["github"].get("enabled").is_none());
        assert_eq!(enabled["mcpServers"]["github"]["url"], "https://example.com/mcp");
    }

    #[test]
    fn test_set_omp_mcp_enabled_project_entry_scrubs_user_denylist() {
        let dir = TempDir::new().unwrap();
        // Project entry file and user file are distinct for project scope.
        let project = dir.path().join("project-mcp.json");
        let user = dir.path().join("user-mcp.json");
        std::fs::write(
            &project,
            r#"{"mcpServers": {"srv": {"command": "echo", "enabled": false}}}"#,
        )
        .unwrap();
        std::fs::write(&user, r#"{"mcpServers": {}, "disabledServers": ["srv"]}"#).unwrap();

        set_omp_mcp_enabled(&project, &user, "srv", true).unwrap();
        let p: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&project).unwrap()).unwrap();
        let u: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&user).unwrap()).unwrap();
        assert!(p["mcpServers"]["srv"].get("enabled").is_none());
        // Denylist would override the entry flag — must be scrubbed.
        assert!(u["disabledServers"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_set_omp_mcp_enabled_missing_user_file_ok() {
        let dir = TempDir::new().unwrap();
        let project = dir.path().join("project-mcp.json");
        std::fs::write(&project, r#"{"mcpServers": {"srv": {"command": "echo"}}}"#).unwrap();
        let user = dir.path().join("does-not-exist.json");

        set_omp_mcp_enabled(&project, &user, "srv", false).unwrap();
        let p: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&project).unwrap()).unwrap();
        assert_eq!(p["mcpServers"]["srv"]["enabled"], false);
        // No user file must not be created just to scrub a list.
        assert!(!user.exists());
    }

    #[test]
    fn test_set_kiro_hook_enabled_flips_in_place() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("lint.json");
        let entry = HookEntry {
            event: "PostFileSave".into(),
            matcher: Some("\\.ts$".into()),
            command: "npm run lint".into(),
            enabled: true,
        };
        deploy_hook(&config, &entry, HookFormat::KiroIde).unwrap();

        set_kiro_hook_enabled(
            &config,
            "PostFileSave",
            Some("\\.ts$"),
            "npm run lint",
            false,
        )
        .unwrap();
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(doc["hooks"].as_array().unwrap().len(), 1, "entry kept");
        assert_eq!(doc["hooks"][0]["enabled"], false);

        set_kiro_hook_enabled(
            &config,
            "PostFileSave",
            Some("\\.ts$"),
            "npm run lint",
            true,
        )
        .unwrap();
        let doc2: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(doc2["hooks"][0]["enabled"], true);

        // Unknown hook → NotFound, so callers can fall through to other files.
        let err = set_kiro_hook_enabled(&config, "Stop", None, "missing", false).unwrap_err();
        assert!(matches!(err, HkError::NotFound(_)));
    }

    #[test]
    fn test_remove_hook_cursor_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(
            &config,
            r#"{"version":1,"hooks":{"stop":[{"command":"echo done"},{"command":"echo other"}]}}"#,
        )
        .unwrap();

        remove_hook(&config, "stop", None, "echo done", HookFormat::Cursor).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let stops = content["hooks"]["stop"].as_array().unwrap();
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0]["command"], "echo other");
    }

    #[test]
    fn test_remove_hook_copilot_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(&config, r#"{"version":1,"hooks":{"PreToolUse":[{"type":"command","command":"./check.sh"},{"type":"command","command":"./other.sh"}]}}"#).unwrap();

        remove_hook(
            &config,
            "PreToolUse",
            None,
            "./check.sh",
            HookFormat::Copilot,
        )
        .unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "./other.sh");
    }

    #[test]
    fn test_remove_hook_windsurf_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(
            &config,
            r#"{"hooks":{"post_cascade_response":[{"powershell":"python C:\\hooks\\log.py"},{"command":"echo other"}]}}"#,
        )
        .unwrap();

        remove_hook(
            &config,
            "post_cascade_response",
            None,
            "python C:\\hooks\\log.py",
            HookFormat::Windsurf,
        )
        .unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["post_cascade_response"]
            .as_array()
            .unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "echo other");
    }

    #[test]
    fn test_hermes_yaml_hook_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "model:\n  default: x\n").unwrap();
        let entry = HookEntry {
            event: "pre_tool_call".into(),
            matcher: Some("terminal".into()),
            command: "~/.hermes/agent-hooks/block.sh".into(),
            enabled: true,
        };
        deploy_hook(&cfg, &entry, HookFormat::HermesYaml).unwrap();
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(
            doc.get("model")
                .and_then(|m| m.get("default"))
                .and_then(|v| v.as_str()),
            Some("x")
        );
        let saved = read_hook_config(
            &cfg,
            "pre_tool_call",
            Some("terminal"),
            "~/.hermes/agent-hooks/block.sh",
            HookFormat::HermesYaml,
        )
        .unwrap();
        assert!(saved.is_some());
        remove_hook(
            &cfg,
            "pre_tool_call",
            Some("terminal"),
            "~/.hermes/agent-hooks/block.sh",
            HookFormat::HermesYaml,
        )
        .unwrap();
        let after: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert!(
            after
                .get("hooks")
                .and_then(|h| h.get("pre_tool_call"))
                .is_none()
        );
        restore_hook(
            &cfg,
            "pre_tool_call",
            &saved.unwrap(),
            HookFormat::HermesYaml,
        )
        .unwrap();
        let restored = read_hook_config(
            &cfg,
            "pre_tool_call",
            Some("terminal"),
            "~/.hermes/agent-hooks/block.sh",
            HookFormat::HermesYaml,
        )
        .unwrap();
        assert!(restored.is_some());
    }

    #[test]
    fn test_hermes_yaml_hook_deploy_dedup() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "model:\n  default: x\n").unwrap();
        let entry = HookEntry {
            event: "pre_tool_call".into(),
            matcher: Some("terminal".into()),
            command: "~/.hermes/agent-hooks/block.sh".into(),
            enabled: true,
        };
        // Deploying the identical hook twice must not duplicate the list item.
        deploy_hook(&cfg, &entry, HookFormat::HermesYaml).unwrap();
        deploy_hook(&cfg, &entry, HookFormat::HermesYaml).unwrap();
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let seq = doc
            .get("hooks")
            .and_then(|h| h.get("pre_tool_call"))
            .and_then(|v| v.as_sequence())
            .expect("pre_tool_call should be a sequence");
        assert_eq!(seq.len(), 1, "duplicate deploy should be deduped");
    }

    #[test]
    fn test_hermes_yaml_hook_matcherless_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "model:\n  default: x\n").unwrap();
        let entry = HookEntry {
            event: "on_session_start".into(),
            matcher: None,
            command: "~/.hermes/agent-hooks/log.sh".into(),
            enabled: true,
        };
        deploy_hook(&cfg, &entry, HookFormat::HermesYaml).unwrap();

        // read_hook_config with matcher=None finds the matcher-less entry.
        let saved = read_hook_config(
            &cfg,
            "on_session_start",
            None,
            "~/.hermes/agent-hooks/log.sh",
            HookFormat::HermesYaml,
        )
        .unwrap();
        assert!(saved.is_some());

        // The written item must carry no `matcher` key.
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let item = doc
            .get("hooks")
            .and_then(|h| h.get("on_session_start"))
            .and_then(|v| v.as_sequence())
            .and_then(|seq| seq.first())
            .expect("on_session_start should have one item");
        assert!(
            item.get("matcher").is_none(),
            "matcher-less hook must not write a matcher key"
        );
        assert_eq!(
            item.get("command").and_then(|v| v.as_str()),
            Some("~/.hermes/agent-hooks/log.sh")
        );
    }

    #[test]
    fn test_set_hermes_plugin_enabled_toggles_list() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "plugins:\n  enabled:\n    - calculator\n").unwrap();
        set_hermes_plugin_enabled(&cfg, "weather", true).unwrap();
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let list: Vec<&str> = doc["plugins"]["enabled"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(list.contains(&"calculator") && list.contains(&"weather"));
        set_hermes_plugin_enabled(&cfg, "calculator", false).unwrap();
        let doc2: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let list2: Vec<&str> = doc2["plugins"]["enabled"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(!list2.contains(&"calculator") && list2.contains(&"weather"));
    }

    #[test]
    fn test_set_hermes_plugin_enabled_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "plugins:\n  enabled:\n    - calculator\n").unwrap();

        // Enabling an already-enabled plugin must not duplicate it.
        set_hermes_plugin_enabled(&cfg, "calculator", true).unwrap();
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let list: Vec<&str> = doc["plugins"]["enabled"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(list, vec!["calculator"], "no duplicate on re-enable");

        // Disabling an absent plugin must be a clean no-op.
        set_hermes_plugin_enabled(&cfg, "ghost", false).unwrap();
        let doc2: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let list2: Vec<&str> = doc2["plugins"]["enabled"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(
            list2,
            vec!["calculator"],
            "disabling absent plugin is a no-op"
        );
    }

    #[test]
    fn test_set_hermes_mcp_enabled_flips_in_place_preserving_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(
            &cfg,
            "mcp_servers:\n  github:\n    command: npx\n    args:\n    - -y\n    env:\n      TOKEN: secret123\n    tools:\n      include:\n      - a\n      - b\n    enabled: true\n  time:\n    command: uvx\n",
        )
        .unwrap();
        // disable github in place
        set_hermes_mcp_enabled(&cfg, "github", false).unwrap();
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        let gh = doc
            .get("mcp_servers")
            .and_then(|m| m.get("github"))
            .unwrap();
        assert_eq!(gh.get("enabled").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            gh.get("env")
                .and_then(|e| e.get("TOKEN"))
                .and_then(|v| v.as_str()),
            Some("secret123")
        );
        let include: Vec<&str> = gh["tools"]["include"]
            .as_sequence()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(include, vec!["a", "b"]);
        assert!(doc.get("mcp_servers").and_then(|m| m.get("time")).is_some());
        // re-enable
        set_hermes_mcp_enabled(&cfg, "github", true).unwrap();
        let doc2: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(
            doc2["mcp_servers"]["github"]["enabled"].as_bool(),
            Some(true)
        );
        // `time` has no `enabled` key on disk; disabling must INSERT enabled:false.
        set_hermes_mcp_enabled(&cfg, "time", false).unwrap();
        let doc3: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        assert_eq!(
            doc3["mcp_servers"]["time"]["enabled"].as_bool(),
            Some(false)
        );
        // and `time` keeps its command (entry not rebuilt)
        assert_eq!(doc3["mcp_servers"]["time"]["command"].as_str(), Some("uvx"));
    }

    #[test]
    fn test_set_hermes_mcp_enabled_missing_server_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = tmp.path().join("config.yaml");
        std::fs::write(&cfg, "mcp_servers:\n  time:\n    command: uvx\n").unwrap();
        assert!(set_hermes_mcp_enabled(&cfg, "ghost", false).is_err());
    }

    #[test]
    fn test_copy_dir_recursive_skips_symlinks() {
        let src_dir = TempDir::new().unwrap();
        let skill_dir = src_dir.path().join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# My Skill").unwrap();

        // Create a symlink to a file outside the skill directory
        let secret = src_dir.path().join("secret.txt");
        std::fs::write(&secret, "TOP SECRET").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&secret, skill_dir.join("link-to-secret")).unwrap();

        let target_dir = TempDir::new().unwrap();
        deploy_skill(&skill_dir, target_dir.path()).unwrap();

        assert!(target_dir.path().join("my-skill").join("SKILL.md").exists());
        // Symlink should NOT have been followed/copied
        #[cfg(unix)]
        assert!(
            !target_dir
                .path()
                .join("my-skill")
                .join("link-to-secret")
                .exists()
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_dir_recursive_uses_symlink_metadata_recheck() {
        // Verify that copy_dir_recursive uses symlink_metadata to avoid following
        // symlinks even if a TOCTOU race replaces a file with a symlink between
        // the readdir check and the copy. We test by creating a symlinked directory
        // and verifying it's not traversed.
        let src_dir = TempDir::new().unwrap();
        let skill_dir = src_dir.path().join("skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Skill").unwrap();

        // Create a symlinked subdirectory pointing outside
        let outside = TempDir::new().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "SECRET DATA").unwrap();
        std::os::unix::fs::symlink(outside.path(), skill_dir.join("evil-link")).unwrap();

        let dst = TempDir::new().unwrap();
        let dst_dir = dst.path().join("skill");
        copy_dir_recursive(&skill_dir, &dst_dir).unwrap();

        assert!(dst_dir.join("SKILL.md").exists());
        // The symlinked directory should be skipped entirely
        assert!(!dst_dir.join("evil-link").exists());
    }

    #[test]
    fn test_set_gemini_extension_enabled_disable() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let ext_dir = home.join(".gemini").join("extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        )
        .unwrap();
        let overrides = content["my-ext"]["overrides"].as_array().unwrap();
        assert_eq!(overrides.len(), 1);
        let expected = format!("!{}/*", home.to_string_lossy());
        assert_eq!(overrides[0].as_str().unwrap(), expected);
    }

    #[test]
    fn test_set_gemini_extension_enabled_enable() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let ext_dir = home.join(".gemini").join("extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();
        set_gemini_extension_enabled(&ext_dir, "my-ext", true, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        )
        .unwrap();
        let overrides = content["my-ext"]["overrides"].as_array().unwrap();
        assert_eq!(overrides.len(), 1);
        let expected = format!("{}/*", home.to_string_lossy());
        assert_eq!(overrides[0].as_str().unwrap(), expected);
    }

    #[test]
    fn test_set_gemini_extension_enabled_preserves_other_extensions() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let ext_dir = home.join(".gemini").join("extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();

        std::fs::write(
            ext_dir.join("extension-enablement.json"),
            r#"{"other-ext": {"overrides": ["!/some/workspace/*"]}}"#,
        )
        .unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        )
        .unwrap();
        assert!(content["other-ext"]["overrides"].as_array().unwrap().len() == 1);
        assert!(content["my-ext"]["overrides"].as_array().unwrap().len() == 1);
    }

    #[test]
    fn test_set_gemini_extension_enabled_preserves_workspace_rules() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let ext_dir = home.join(".gemini").join("extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();

        let home_str = home.to_string_lossy();
        let initial = serde_json::json!({
            "my-ext": { "overrides": [
                format!("!/some/workspace/*"),
            ]}
        });
        std::fs::write(
            ext_dir.join("extension-enablement.json"),
            initial.to_string(),
        )
        .unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        )
        .unwrap();
        let overrides = content["my-ext"]["overrides"].as_array().unwrap();
        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides[0].as_str().unwrap(), "!/some/workspace/*");
        assert_eq!(overrides[1].as_str().unwrap(), format!("!{}/*", home_str));
    }

    #[test]
    fn test_remove_gemini_extension_entry() {
        let dir = TempDir::new().unwrap();
        let home = dir.path();
        let ext_dir = home.join(".gemini").join("extensions");
        std::fs::create_dir_all(&ext_dir).unwrap();

        // Create enablement with two extensions
        set_gemini_extension_enabled(&ext_dir, "ext-a", false, home).unwrap();
        set_gemini_extension_enabled(&ext_dir, "ext-b", false, home).unwrap();

        // Remove one
        remove_gemini_extension_entry(&ext_dir, "ext-a").unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        )
        .unwrap();
        assert!(content.get("ext-a").is_none(), "ext-a should be removed");
        assert!(content.get("ext-b").is_some(), "ext-b should remain");
    }

    #[test]
    fn test_remove_codex_plugin_entry() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("config.toml");

        // Set up two plugin entries
        set_codex_plugin_enabled(&config, "pluginA@marketplace", false).unwrap();
        set_codex_plugin_enabled(&config, "pluginB@marketplace", true).unwrap();

        // Remove one
        remove_codex_plugin_entry(&config, "pluginA@marketplace").unwrap();

        let content: toml::Table = std::fs::read_to_string(&config).unwrap().parse().unwrap();
        let plugins = content["plugins"].as_table().unwrap();
        assert!(!plugins.contains_key("pluginA@marketplace"));
        assert!(plugins.contains_key("pluginB@marketplace"));
    }

    #[test]
    fn test_remove_vscode_plugin_entry() {
        let dir = TempDir::new().unwrap();
        let gs = dir.path().join("globalStorage");
        std::fs::create_dir_all(&gs).unwrap();
        let db_path = gs.join("state.vscdb");

        // Set up state.vscdb
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS ItemTable (key TEXT UNIQUE, value TEXT)",
            [],
        )
        .unwrap();

        // Add two entries
        set_vscode_plugin_enabled(dir.path(), "file:///plugin-a", false).unwrap();
        set_vscode_plugin_enabled(dir.path(), "file:///plugin-b", true).unwrap();

        // Remove one
        remove_vscode_plugin_entry(dir.path(), "file:///plugin-a").unwrap();

        let result: String = conn
            .query_row(
                "SELECT value FROM ItemTable WHERE key = 'agentPlugins.enablement'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let entries: Vec<(String, bool)> = serde_json::from_str(&result).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "file:///plugin-b");
    }
}

#[cfg(test)]
mod dsh_toggle_tests {
    use super::*;

    const HOME_WITH_GH: &str = r#"# precious comment
- insert:
    - id: mcp-github
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: github
        transport: stdio
        command: npx
        env:
          GITHUB_TOKEN: !!js process.env.GITHUB_TOKEN
"#;

    fn patch_file(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cordis.patch.yml");
        std::fs::write(&path, text).unwrap();
        (tmp, path)
    }

    #[test]
    fn disable_appends_managed_block_and_enable_removes_it() {
        let (_tmp, path) = patch_file(HOME_WITH_GH);

        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(HOME_WITH_GH), "user bytes preserved verbatim");
        assert!(text.contains("- id: mcp-github\n  disabled: true"));
        assert!(text.contains("managed by HarnessKit"));

        // Enable: base state (the insert) is already enabled → block entry
        // removed entirely; user content restored byte-for-byte.
        set_dsh_mcp_enabled(&path, "github", true).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), HOME_WITH_GH);
    }

    #[test]
    fn enable_overrides_user_disable_with_disabled_false() {
        // User disabled it themselves → HK writes an explicit disabled: false
        // override (upstream e2e-covered semantics).
        let text = format!("{HOME_WITH_GH}- id: mcp-github\n  disabled: true\n");
        let (_tmp, path) = patch_file(&text);
        set_dsh_mcp_enabled(&path, "github", true).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.starts_with(&text), "user bytes preserved");
        assert!(out.contains("- id: mcp-github\n  disabled: false"));
    }

    #[test]
    fn template_file_toggle_errors_not_found() {
        // dsh's seeded patch template: comment header + literal []. No row
        // exists in it → toggling must error, and the file must be untouched.
        let template = "# header comment\n[]\n";
        let (_tmp, path) = patch_file(template);
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::NotFound(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), template);
    }

    #[test]
    fn roundtrip_always_leaves_valid_yaml_list() {
        // After any disable→enable cycle the file must re-parse as a YAML
        // list — an empty/comment-only patch file is a dsh boot error.
        let (_tmp, path) = patch_file(HOME_WITH_GH);
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        set_dsh_mcp_enabled(&path, "github", true).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert!(parsed.is_sequence(), "file must stay a valid YAML list");
    }

    #[test]
    fn unknown_server_errors() {
        let (_tmp, path) = patch_file(HOME_WITH_GH);
        let err = set_dsh_mcp_enabled(&path, "nope", false).unwrap_err();
        assert!(matches!(err, HkError::NotFound(_)));
    }

    #[test]
    fn toggle_is_idempotent() {
        let (_tmp, path) = patch_file(HOME_WITH_GH);
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        let once = std::fs::read_to_string(&path).unwrap();
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), once);
    }

    #[test]
    fn unbalanced_markers_error_and_leave_file_untouched() {
        let text = format!(
            "{HOME_WITH_GH}{}\n- id: mcp-github\n  disabled: true\n",
            DSH_BLOCK_BEGIN
        );
        let (_tmp, path) = patch_file(&text); // BEGIN without END
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn corrupted_block_yaml_errors_and_leaves_file_untouched() {
        // HK owns every byte inside the markers: unparseable block content is
        // a hard ConfigCorrupted, refuse to write.
        let text = format!(
            "{HOME_WITH_GH}{DSH_BLOCK_BEGIN}\n- id: [unclosed\n{DSH_BLOCK_END}\n"
        );
        let (_tmp, path) = patch_file(&text);
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn unrecognized_block_entry_errors_instead_of_silent_drop() {
        // Behavior change pinned on purpose: entries HK did not render are
        // corruption, not noise to discard.
        let text = format!(
            "{HOME_WITH_GH}{DSH_BLOCK_BEGIN}\n- surprise: true\n{DSH_BLOCK_END}\n"
        );
        let (_tmp, path) = patch_file(&text);
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn toggle_entry_with_extra_keys_errors() {
        // Extra keys on an id-targeted entry are LIVE dsh patch semantics
        // (they would patch the target row) — dropping them on re-render
        // would alter the user's effective config, so they are corruption.
        let text = format!(
            "{HOME_WITH_GH}{DSH_BLOCK_BEGIN}\n- id: mcp-github\n  disabled: true\n  command: pwned\n{DSH_BLOCK_END}\n"
        );
        let (_tmp, path) = patch_file(&text);
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn insert_group_with_extra_keys_errors() {
        let text = format!(
            "{HOME_WITH_GH}{DSH_BLOCK_BEGIN}\n- insert:\n    - id: mcp-x\n  after: mcp-github\n{DSH_BLOCK_END}\n"
        );
        let (_tmp, path) = patch_file(&text);
        let err = set_dsh_mcp_enabled(&path, "github", false).unwrap_err();
        assert!(matches!(err, HkError::ConfigCorrupted(_)));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
    }

    #[test]
    fn hk_inserted_server_toggles_via_own_row_and_render_is_byte_stable() {
        // Pins the serde_yaml insert byte format BEFORE T8 depends on it.
        let text = format!(
            "{HOME_WITH_GH}{DSH_BLOCK_BEGIN}\n- insert:\n    - id: mcp-web\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        serverName: web\n        transport: streamable-http\n        url: http://localhost:3000/mcp\n{DSH_BLOCK_END}\n"
        );
        let (_tmp, path) = patch_file(&text);
        set_dsh_mcp_enabled(&path, "web", false).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.starts_with(HOME_WITH_GH), "user bytes preserved");

        // Disable lands on the insert row's OWN `disabled` field — never a
        // separate toggle entry.
        let (user_text, block) = split_dsh_managed_block(&out).unwrap();
        assert!(block.toggles.is_empty(), "no separate toggle entry");
        assert_eq!(block.inserts.len(), 1);
        assert_eq!(
            block.inserts[0].get("disabled").and_then(|v| v.as_bool()),
            Some(true)
        );

        let parsed: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert!(parsed.is_sequence(), "file must stay a valid YAML list");

        // Byte-stable: a second split→render reproduces the file exactly.
        assert_eq!(render_dsh_patch(&user_text, &block), out);
    }

    #[test]
    fn user_content_after_block_survives_roundtrip() {
        // Documented out-vote mechanism: user lines AFTER the managed block
        // must never be lost. (They may legitimately be reordered before the
        // re-appended block on the next toggle — base-state semantics.)
        let (_tmp, path) = patch_file(HOME_WITH_GH);
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("# user note after block\n");
        std::fs::write(&path, &text).unwrap();
        set_dsh_mcp_enabled(&path, "github", true).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("# user note after block"));
        assert!(!out.contains("managed by HarnessKit"), "override no longer needed");
    }

    #[test]
    fn two_servers_share_one_managed_block() {
        let text = format!(
            "{HOME_WITH_GH}    - id: mcp-web\n      name: '@deepseek-ai/dsh-mcp-client'\n      config:\n        serverName: web\n        transport: streamable-http\n        url: http://localhost:3000/mcp\n"
        );
        let (_tmp, path) = patch_file(&text);
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        set_dsh_mcp_enabled(&path, "web", false).unwrap();
        let out = std::fs::read_to_string(&path).unwrap();
        assert_eq!(out.matches(DSH_BLOCK_BEGIN).count(), 1, "exactly one block");
        assert!(out.contains("- id: mcp-github\n  disabled: true"));
        assert!(out.contains("- id: mcp-web\n  disabled: true"));
        let parsed: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert!(parsed.is_sequence());
    }
}

#[cfg(test)]
mod dsh_plugin_toggle_tests {
    use super::*;

    const PROFILE_PATCH: &str =
        "- insert:\n    - id: tool-policy\n      name: dsh-plugin-tool\n      config:\n        mode: strict\n";

    fn dsh_home_with_profile(
        patch: &str,
        home_patch: Option<&str>,
    ) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let dsh_home = tmp.path().join(".dsh");
        std::fs::create_dir_all(dsh_home.join("profiles/web")).unwrap();
        std::fs::write(dsh_home.join("profiles/web/cordis.patch.yml"), patch).unwrap();
        if let Some(h) = home_patch {
            std::fs::write(dsh_home.join("cordis.patch.yml"), h).unwrap();
        }
        (tmp, dsh_home)
    }

    /// The layer that DEFINES the row in the `dsh_home_with_profile` fixture
    /// — what the dsh adapter puts on the entry the UI toggled.
    fn web_layer(dsh_home: &Path) -> std::path::PathBuf {
        dsh_home.join("profiles/web/cordis.patch.yml")
    }

    /// The home patch the writer edits — what `manager::toggle_plugin` passes
    /// straight through from the dsh adapter's `mcp_config_path()`.
    fn home_patch(dsh_home: &Path) -> std::path::PathBuf {
        dsh_home.join("cordis.patch.yml")
    }

    #[test]
    fn disable_profile_row_writes_home_block_and_enable_removes_it() {
        let (_tmp, dsh_home) = dsh_home_with_profile(PROFILE_PATCH, None);
        set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            false,
            &[web_layer(&dsh_home)],
        )
        .unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(home.contains("managed by HarnessKit"));
        assert!(home.contains("- id: tool-policy\n  disabled: true"));
        // ONLY the home file is written — profile patch stays byte-identical.
        assert_eq!(
            std::fs::read_to_string(dsh_home.join("profiles/web/cordis.patch.yml")).unwrap(),
            PROFILE_PATCH
        );
        set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            true,
            &[web_layer(&dsh_home)],
        )
        .unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(!home.contains("managed by HarnessKit"), "back to base → entry removed");
        let parsed: serde_yaml::Value = serde_yaml::from_str(&home).unwrap();
        assert!(parsed.is_sequence(), "file must stay a valid YAML list");
    }

    #[test]
    fn enable_user_disabled_row_writes_disabled_false_override() {
        // The row is disabled IN THE PROFILE FILE by the user; HK enable must
        // write an explicit `disabled: false` override (last layer wins).
        let patch = format!("{PROFILE_PATCH}- id: tool-policy\n  disabled: true\n");
        let (_tmp, dsh_home) = dsh_home_with_profile(&patch, None);
        set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            true,
            &[web_layer(&dsh_home)],
        )
        .unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(home.contains("- id: tool-policy\n  disabled: false"));
    }

    #[test]
    fn home_user_override_is_part_of_base_state() {
        // User already disabled the row from their home patch text: HK
        // disable is then a no-op (no block written).
        let (_tmp, dsh_home) = dsh_home_with_profile(
            PROFILE_PATCH,
            Some("- id: tool-policy\n  disabled: true\n"),
        );
        set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            false,
            &[web_layer(&dsh_home)],
        )
        .unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(!home.contains("managed by HarnessKit"), "already disabled at base");
    }

    #[test]
    fn unknown_row_errors_not_found_and_writes_nothing() {
        let (_tmp, dsh_home) = dsh_home_with_profile(PROFILE_PATCH, None);
        let err =
            set_dsh_plugin_enabled(
                &home_patch(&dsh_home),
                "nope",
                false,
                &[web_layer(&dsh_home)],
            )
            .unwrap_err();
        assert!(matches!(err, HkError::NotFound(_)));
        assert!(!dsh_home.join("cordis.patch.yml").exists(), "nothing written");
    }

    #[test]
    fn sibling_profile_override_is_not_part_of_base_state() {
        // Row DEFINED (enabled) in profile `alpha`, separately overridden
        // `disabled: true` in profile `beta`. dsh boots ONE profile at a
        // time — beta's patch is never loaded next to alpha's — so from
        // alpha's entry the base state is ENABLED and disabling it must
        // actually write an override. Folding beta in would compute
        // base=disabled and silently write nothing, leaving the plugin
        // loaded in alpha.
        let tmp = tempfile::tempdir().unwrap();
        let dsh_home = tmp.path().join(".dsh");
        std::fs::create_dir_all(dsh_home.join("profiles/alpha")).unwrap();
        std::fs::create_dir_all(dsh_home.join("profiles/beta")).unwrap();
        std::fs::write(dsh_home.join("profiles/alpha/cordis.patch.yml"), PROFILE_PATCH).unwrap();
        std::fs::write(
            dsh_home.join("profiles/beta/cordis.patch.yml"),
            "- id: tool-policy\n  disabled: true\n",
        )
        .unwrap();
        let alpha_layer = dsh_home.join("profiles/alpha/cordis.patch.yml");

        set_dsh_plugin_enabled(&home_patch(&dsh_home), "tool-policy", false, std::slice::from_ref(&alpha_layer)).unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(
            home.contains("- id: tool-policy\n  disabled: true"),
            "alpha's row is enabled at base, so disable must write: {home}"
        );

        // And back: the row returns to alpha's own base → block removed. A
        // fold that consulted beta would instead leave a gratuitous
        // `disabled: false`, overriding beta's own choice machine-wide.
        set_dsh_plugin_enabled(&home_patch(&dsh_home), "tool-policy", true, std::slice::from_ref(&alpha_layer)).unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(
            !home.contains("managed by HarnessKit"),
            "back to alpha's base → entry removed: {home}"
        );
        assert!(!home.contains("disabled: false"), "no gratuitous override: {home}");
    }

    #[test]
    fn own_layer_override_after_the_definition_is_part_of_base_state() {
        // Same row defined AND overridden inside the toggled entry's own
        // layer: that override is loaded with the definition, so it does
        // count — the per-profile rule narrows the fold, it does not drop
        // in-layer ordering.
        let patch = format!("{PROFILE_PATCH}- id: tool-policy\n  disabled: true\n");
        let (_tmp, dsh_home) = dsh_home_with_profile(&patch, None);
        set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            false,
            &[web_layer(&dsh_home)],
        )
        .unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml"))
            .unwrap_or_else(|_| String::new());
        assert!(
            !home.contains("managed by HarnessKit"),
            "the row's own layer already disables it at base: {home}"
        );
    }

    #[test]
    fn corrupted_block_error_names_the_home_patch_path() {
        // The call-site map_err must append the file path + remediation hint
        // to the path-agnostic block parser's ConfigCorrupted.
        let bad_home = format!("{DSH_BLOCK_BEGIN}\n- surprise: 1\n{DSH_BLOCK_END}\n");
        let (_tmp, dsh_home) = dsh_home_with_profile(PROFILE_PATCH, Some(&bad_home));
        let err = set_dsh_plugin_enabled(
            &home_patch(&dsh_home),
            "tool-policy",
            false,
            &[web_layer(&dsh_home)],
        )
            .unwrap_err();
        let HkError::ConfigCorrupted(msg) = err else {
            panic!("expected ConfigCorrupted, got {err:?}");
        };
        let home_path = dsh_home.join("cordis.patch.yml").display().to_string();
        assert!(msg.contains(&home_path), "message names the file: {msg}");
        assert!(msg.contains("fix or remove"), "message carries the hint: {msg}");
        // Nothing written: the corrupt file is untouched.
        assert_eq!(
            std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap(),
            bad_home
        );
    }

    #[test]
    fn home_defined_row_toggles_too() {
        // Rows defined directly in the home layer are also valid targets:
        // the owning layer IS the home patch, and the writer must then read
        // that layer only through the block-stripped user text.
        let (_tmp, dsh_home) = dsh_home_with_profile(
            "[]\n",
            Some("- insert:\n    - id: theme-row\n      name: dsh-plugin-theme\n"),
        );
        let home_layer = dsh_home.join("cordis.patch.yml");
        set_dsh_plugin_enabled(&home_patch(&dsh_home), "theme-row", false, std::slice::from_ref(&home_layer)).unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(home.contains("- id: theme-row\n  disabled: true"));
        assert!(home.starts_with("- insert:"), "user bytes preserved");
        // Re-enable with our own block already in the file: the block must
        // never be folded back in as "base", or this would look like a no-op.
        set_dsh_plugin_enabled(&home_patch(&dsh_home), "theme-row", true, std::slice::from_ref(&home_layer)).unwrap();
        let home = std::fs::read_to_string(dsh_home.join("cordis.patch.yml")).unwrap();
        assert!(!home.contains("managed by HarnessKit"), "back to base → entry removed");
    }
}

#[cfg(test)]
mod dsh_insert_writer_tests {
    use super::*;
    use crate::adapter::dsh::DshAdapter;

    const USER_GH: &str = r#"# precious comment
- insert:
    - id: mcp-github
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: github
        transport: stdio
        command: npx
"#;

    fn patch_file(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cordis.patch.yml");
        std::fs::write(&path, text).unwrap();
        (tmp, path)
    }

    fn stdio_entry(name: &str) -> McpServerEntry {
        McpServerEntry {
            name: name.into(),
            command: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-github".into()],
            env: std::collections::HashMap::from([(
                "GITHUB_TOKEN".to_string(),
                "tok".to_string(),
            )]),
            transport: McpTransport::Stdio,
            url: None,
            headers: Default::default(),
            enabled: true,
        }
    }

    #[test]
    fn stdio_install_round_trips_through_the_dsh_reader() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cordis.patch.yml");
        // Missing file: writer starts from the valid empty form.
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github2")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("managed by HarnessKit"));
        // Markers are YAML comments, so the whole file parses as one
        // document and the P0 reader sees the new row with no extra plumbing.
        assert_eq!(DshAdapter::mcp_enabled_in_text(&text).get("github2"), Some(&true));
        assert_eq!(
            DshAdapter::mcp_row_id_in_text(&text, "github2").as_deref(),
            Some("mcp-github2")
        );
        // Explicit discriminant + serverName always (mcp-client schema).
        assert!(text.contains("transport: stdio"));
        assert!(text.contains("serverName: github2"));
        assert!(text.contains("command: npx"));
        assert!(text.contains("GITHUB_TOKEN: tok"));
    }

    #[test]
    fn install_preserves_user_bytes_and_drops_placeholder() {
        let (_tmp, path) = patch_file("# my notes\n[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github2")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# my notes\n"));
        assert!(
            !text.lines().any(|l| l.trim() == "[]"),
            "placeholder can't coexist with entries"
        );
        let parsed: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
        assert!(parsed.is_sequence());
    }

    #[test]
    fn server_name_is_sanitized_to_the_mcp_client_pattern() {
        // /^[A-Za-z0-9_-]{1,32}$/ — invalid chars map to '-', 32-char cap.
        let (_tmp, path) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("My Server/rocks!")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("serverName: My-Server-rocks-"));

        // A name with no valid character at all cannot be sanitized.
        let err = deploy_mcp_server_dsh_cordis(&path, &stdio_entry("///")).unwrap_err();
        assert!(matches!(err, HkError::Validation(_)));
    }

    #[test]
    fn server_name_and_row_id_collisions_error() {
        // serverName collision with a user-authored row.
        let (_tmp, path) = patch_file(USER_GH);
        let err = deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github")).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("github")));

        // Row-id collision with an unrelated user row occupying the generated id.
        let user2 = "- insert:\n    - id: mcp-github2\n      name: dsh-plugin-tool\n      config:\n        mode: x\n";
        let (_tmp2, path2) = patch_file(user2);
        let err = deploy_mcp_server_dsh_cordis(&path2, &stdio_entry("github2")).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("mcp-github2")));

        // Double-install of the same HK server collides with its own block row.
        let (_tmp3, path3) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path3, &stdio_entry("github2")).unwrap();
        let err = deploy_mcp_server_dsh_cordis(&path3, &stdio_entry("github2")).unwrap_err();
        assert!(matches!(err, HkError::Validation(_)));
    }

    #[test]
    fn toggle_of_hk_inserted_server_edits_its_own_insert_entry() {
        let (_tmp, path) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github2")).unwrap();
        set_dsh_mcp_enabled(&path, "github2", false).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("disabled: true"));
        assert_eq!(
            text.matches("id: mcp-github2").count(),
            1,
            "no separate override row — the insert row itself carries disabled"
        );
        assert_eq!(DshAdapter::mcp_enabled_in_text(&text).get("github2"), Some(&false));

        // ENABLE path of an HK-inserted row: the disabled key is removed from
        // the row itself and the reader sees the server enabled again.
        set_dsh_mcp_enabled(&path, "github2", true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("disabled"), "re-enable removes the disabled key");
        assert_eq!(DshAdapter::mcp_enabled_in_text(&text).get("github2"), Some(&true));
    }

    #[test]
    fn user_row_toggle_back_to_base_keeps_hk_insert_rows() {
        // Spec-pinned: removing a toggle entry must NOT delete co-resident
        // HK insert rows in the same block.
        let (_tmp, path) = patch_file(USER_GH);
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github2")).unwrap();
        set_dsh_mcp_enabled(&path, "github", false).unwrap();
        set_dsh_mcp_enabled(&path, "github", true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(USER_GH), "user bytes preserved");
        assert!(text.contains("serverName: github2"), "HK insert row survives");
        assert!(
            !text.contains("- id: mcp-github\n  disabled"),
            "toggle entry for the user row is gone"
        );
    }

    #[test]
    fn remove_deletes_hk_row_refuses_user_row_ignores_absent() {
        let (_tmp, path) = patch_file(USER_GH);
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("github2")).unwrap();

        // HK-inserted row: removed; block (now empty) disappears; user bytes intact.
        remove_mcp_server(&path, "github2", McpFormat::DshCordis).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(USER_GH));
        assert!(!text.contains("managed by HarnessKit"));

        // User-authored row: Validation refusal, file untouched.
        let err = remove_mcp_server(&path, "github", McpFormat::DshCordis).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("cordis.patch.yml")));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);

        // Absent name: idempotent no-op, like every other format.
        remove_mcp_server(&path, "nope", McpFormat::DshCordis).unwrap();
    }

    #[test]
    fn crlf_user_file_survives_install_byte_for_byte() {
        let user = "# note\r\n- insert:\r\n    - id: mcp-github\r\n      name: '@deepseek-ai/dsh-mcp-client'\r\n      config:\r\n        serverName: github\r\n        transport: stdio\r\n        command: npx\r\n";
        let (_tmp, path) = patch_file(user);
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("web2")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with(user), "CRLF user bytes preserved verbatim");
        let parsed: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
        assert!(parsed.is_sequence());
    }

    #[test]
    fn deploy_mcp_server_dispatch_routes_dsh_cordis() {
        use crate::adapter::AgentAdapter;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let path = adapter.mcp_config_path();
        deploy_mcp_server(&path, &stdio_entry("github2"), &adapter).unwrap();
        assert_eq!(adapter.read_mcp_servers().len(), 1);
    }

    /// The bug this pins: installing `microsoft/markitdown` used to write a
    /// row named `microsoft-markitdown` with no record of the original, so
    /// the scanner read back a DIFFERENT extension — the source row's DSH
    /// button never turned ✓, a re-install failed on the serverName
    /// collision, and the list grew a ghost `microsoft-markitdown` row.
    #[test]
    fn install_records_the_original_name_and_the_reader_round_trips_it() {
        use crate::adapter::AgentAdapter;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let path = adapter.mcp_config_path();

        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("microsoft/markitdown")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // On disk: the sanitized name mcp-client accepts, plus the original.
        assert!(text.contains("serverName: microsoft-markitdown"), "{text}");
        assert!(text.contains("_hk_name: microsoft/markitdown"), "{text}");

        // Read back: the ORIGINAL name, so the extension groups with the
        // other agents' rows instead of forming a second one.
        let servers = adapter.read_mcp_servers();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name, "microsoft/markitdown");

        // Deployer-side lookups still key on the STORED serverName.
        assert_eq!(
            DshAdapter::mcp_row_id_in_text(&text, "microsoft-markitdown").as_deref(),
            Some("mcp-microsoft-markitdown")
        );
        assert!(DshAdapter::mcp_enabled_in_text(&text).contains_key("microsoft-markitdown"));
    }

    #[test]
    fn install_omits_hk_name_when_the_name_needs_no_sanitizing() {
        // Same conditional as Codex: unchanged names keep their exact bytes.
        let (_tmp, path) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("my_server-1")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("serverName: my_server-1"), "{text}");
        assert!(!text.contains("_hk_name"), "{text}");
    }

    #[test]
    fn toggle_and_remove_work_through_the_original_name() {
        use crate::adapter::AgentAdapter;
        // The scanner now hands the manager the ORIGINAL name, so every
        // by-name path must resolve it to the row stored under the sanitized
        // `serverName` — and must still be a no-op-free round trip.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let path = adapter.mcp_config_path();
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("microsoft/markitdown")).unwrap();

        set_dsh_mcp_enabled(&path, "microsoft/markitdown", false).unwrap();
        let disabled = adapter.read_mcp_servers();
        assert_eq!(disabled[0].name, "microsoft/markitdown");
        assert!(!disabled[0].enabled, "toggle by original name reached the row");

        set_dsh_mcp_enabled(&path, "microsoft/markitdown", true).unwrap();
        assert!(adapter.read_mcp_servers()[0].enabled);

        remove_mcp_server(&path, "microsoft/markitdown", McpFormat::DshCordis).unwrap();
        assert!(adapter.read_mcp_servers().is_empty());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("markitdown"), "row actually gone: {text}");
    }

    #[test]
    fn installing_the_same_original_name_twice_collides_and_adds_no_second_row() {
        use crate::adapter::AgentAdapter;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".dsh")).unwrap();
        let adapter = DshAdapter::with_home(tmp.path().to_path_buf());
        let path = adapter.mcp_config_path();
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("microsoft/markitdown")).unwrap();

        let err =
            deploy_mcp_server_dsh_cordis(&path, &stdio_entry("microsoft/markitdown")).unwrap_err();
        // The message names both the stored name and the original input.
        assert!(matches!(&err, HkError::Validation(m)
            if m.contains("microsoft-markitdown") && m.contains("microsoft/markitdown")));
        assert_eq!(adapter.read_mcp_servers().len(), 1, "no ghost second row");
    }

    #[test]
    fn remove_and_toggle_by_original_name_hit_the_sanitized_row() {
        // Name symmetry: deploy writes the SANITIZED serverName, so remove
        // and toggle called with the ORIGINAL input must normalize the same
        // way — otherwise `remove("My Server")` silently returns Ok while
        // the "My-Server" row stays installed.
        let (_tmp, path) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("My Server")).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("serverName: My-Server"));

        set_dsh_mcp_enabled(&path, "My Server", false).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(DshAdapter::mcp_enabled_in_text(&text).get("My-Server"), Some(&false));

        remove_mcp_server(&path, "My Server", McpFormat::DshCordis).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("My-Server"), "row actually gone: {text}");
    }

    #[test]
    fn build_dsh_insert_row_streamable_http_pins_rendered_bytes() {
        // Byte-level pin of the remote row format, now that dsh advertises
        // RemoteMcpSchema::DshTransport and installs can reach this arm.
        let entry = McpServerEntry {
            name: "web".into(),
            command: String::new(),
            args: vec![],
            env: Default::default(),
            transport: McpTransport::Http,
            url: Some("https://example.com/mcp".into()),
            headers: std::collections::HashMap::from([
                ("X-Api".to_string(), "v1".to_string()),
                ("Authorization".to_string(), "Bearer tok".to_string()),
            ]),
            enabled: true,
        };
        let mut block = DshManagedBlock::default();
        block.inserts.push(build_dsh_insert_row("mcp-web", "web", &entry));
        let out = render_dsh_patch("", &block);
        let expected = format!(
            "{DSH_BLOCK_BEGIN}\n\
             - insert:\n\
             \x20 - id: mcp-web\n\
             \x20   name: '@deepseek-ai/dsh-mcp-client'\n\
             \x20   config:\n\
             \x20     transport: streamable-http\n\
             \x20     serverName: web\n\
             \x20     url: https://example.com/mcp\n\
             \x20     headers:\n\
             \x20       Authorization: Bearer tok\n\
             \x20       X-Api: v1\n\
             {DSH_BLOCK_END}\n"
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn distinct_inputs_sanitizing_to_the_same_name_collide() {
        // "My Server" and "My/Server" both sanitize to "My-Server" — the
        // second install must error (collision), never clobber the first,
        // and the message names both the sanitized and the original form.
        let (_tmp, path) = patch_file("[]\n");
        deploy_mcp_server_dsh_cordis(&path, &stdio_entry("My Server")).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let err = deploy_mcp_server_dsh_cordis(&path, &stdio_entry("My/Server")).unwrap_err();
        assert!(
            matches!(&err, HkError::Validation(m)
                if m.contains("'My-Server'") && m.contains("(from 'My/Server')")),
            "got: {err:?}"
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "no clobber");
    }

    #[test]
    fn stale_toggle_occupying_the_generated_row_id_errors() {
        // A block toggle can outlive the user row it targeted (dsh warn-skips
        // dangling overrides). Its id still occupies the collision domain: an
        // install deriving the same row id must error, not double-define it.
        let stale = format!("{DSH_BLOCK_BEGIN}\n- id: mcp-x\n  disabled: true\n{DSH_BLOCK_END}\n");
        let (_tmp, path) = patch_file(&stale);
        let err = deploy_mcp_server_dsh_cordis(&path, &stdio_entry("x")).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("mcp-x")), "got: {err:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), stale, "file untouched");
    }

    #[test]
    fn profile_layer_row_id_occupies_the_collision_domain() {
        // Profile patches are applied BEFORE the home patch, so their row ids
        // share one namespace with it: generating `mcp-x` while a profile
        // already defines `mcp-x` would be a duplicate definition for dsh.
        let (_tmp, path) = patch_file("[]\n");
        let profile = path.parent().unwrap().join("profiles/web");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(
            profile.join("cordis.patch.yml"),
            "- insert:\n    - id: mcp-x\n      name: dsh-plugin-tool\n",
        )
        .unwrap();
        let err = deploy_mcp_server_dsh_cordis(&path, &stdio_entry("x")).unwrap_err();
        assert!(matches!(&err, HkError::Validation(m) if m.contains("mcp-x")), "got: {err:?}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]\n", "file untouched");
    }

    #[test]
    fn remove_on_a_wholly_absent_file_is_ok() {
        // Pins the writer-level idempotency (NotFound → "[]" synthesis in
        // read_and_split_home_patch) so it can't regress to an IO error —
        // independent of the dispatch-level exists() early return.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("cordis.patch.yml");
        remove_mcp_server_dsh_cordis(&path, "anything").unwrap();
        assert!(!path.exists(), "no file conjured by a no-op removal");
    }
}
