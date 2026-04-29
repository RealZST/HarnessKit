use crate::HkError;
use crate::adapter::{HookEntry, HookFormat, McpFormat, McpServerEntry};
use fs2::FileExt;
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::Path;

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
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '-' })
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
    entry.command = resolve_command_path(&entry.command);
    if let Some(path_val) = build_path_for_command(&entry.command) {
        entry.env.entry("PATH".to_string()).or_insert(path_val);
    }
}

/// Deploy an MCP server config entry into the target agent's config file.
/// Format varies by agent — see `McpFormat`.
pub fn deploy_mcp_server(
    config_path: &Path,
    entry: &McpServerEntry,
    format: McpFormat,
) -> Result<(), HkError> {
    match format {
        McpFormat::McpServers => deploy_mcp_server_json(config_path, entry, "mcpServers"),
        McpFormat::Servers => deploy_mcp_server_json(config_path, entry, "servers"),
        McpFormat::Toml => deploy_mcp_server_toml(config_path, entry),
    }
}

/// JSON-based MCP deploy (Claude, Gemini, Cursor, Antigravity, Copilot).
/// `top_key` is "mcpServers" or "servers" depending on the agent.
fn deploy_mcp_server_json(
    config_path: &Path,
    entry: &McpServerEntry,
    top_key: &str,
) -> Result<(), HkError> {
    locked_modify_json(config_path, |config| {
        let servers = config
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted("Config is not an object".into()))?
            .entry(top_key)
            .or_insert_with(|| serde_json::json!({}));
        let server_val = serde_json::json!({
            "command": entry.command,
            "args": entry.args,
            "env": entry.env,
        });
        servers
            .as_object_mut()
            .ok_or_else(|| HkError::ConfigCorrupted(format!("{} is not an object", top_key)))?
            .insert(entry.name.clone(), server_val);
        Ok(())
    })
}

/// TOML-based MCP deploy (Codex: ~/.codex/config.toml with [mcp_servers.<name>]).
fn deploy_mcp_server_toml(config_path: &Path, entry: &McpServerEntry) -> Result<(), HkError> {
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

    // Build server entry table
    let mut server_table = toml::Table::new();
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

    // Codex requires names to match ^[a-zA-Z0-9_-]+$; sanitize before inserting.
    // Store the original name as `_hk_name` so the scanner can recover it for
    // consistent grouping with other agents that use the unsanitized name.
    let safe_name = sanitize_mcp_name(&entry.name);
    if safe_name != entry.name {
        server_table.insert(
            "_hk_name".into(),
            toml::Value::String(entry.name.clone()),
        );
    }
    mcp_servers.insert(safe_name, toml::Value::Table(server_table));

    // Write back atomically
    atomic_write(
        config_path,
        &toml::to_string_pretty(&doc).map_err(|e| HkError::Internal(e.to_string()))?,
    )?;

    Ok(())
}

/// Deploy a hook config entry into the target agent's config file.
/// Reads the existing JSON, appends the hook under "hooks" -> event, writes back.
pub fn deploy_hook(
    config_path: &Path,
    entry: &HookEntry,
    format: HookFormat,
) -> Result<(), HkError> {
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
        _ => locked_modify_json(config_path, |config| {
            let key = match format {
                McpFormat::Servers => "servers",
                _ => "mcpServers",
            };
            if let Some(servers) = config.get_mut(key).and_then(|v| v.as_object_mut()) {
                servers.remove(server_name);
            }
            Ok(())
        }),
    }
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
            // Convert saved JSON entry back to TOML and write
            let mcp_entry = McpServerEntry {
                name: server_name.to_string(),
                command: entry
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into(),
                args: entry
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                env: entry
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            deploy_mcp_server_toml(config_path, &mcp_entry)
        }
        _ => {
            let key = match format {
                McpFormat::Servers => "servers",
                _ => "mcpServers",
            };
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

/// Restore a previously disabled hook entry into the config file.
pub fn restore_hook(
    config_path: &Path,
    event: &str,
    entry: &serde_json::Value,
    format: HookFormat,
) -> Result<(), HkError> {
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
        content.parse::<toml::Table>().map_err(|e| HkError::ConfigCorrupted(e.to_string()))?
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
pub fn remove_codex_plugin_entry(
    config_path: &Path,
    plugin_key: &str,
) -> Result<(), HkError> {
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
    let db_path = vscode_user_dir
        .join("globalStorage")
        .join("state.vscdb");
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

    let mut entries: Vec<(String, bool)> =
        serde_json::from_str(&current).unwrap_or_default();

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

    let new_value = serde_json::to_string(&entries)
        .map_err(|e| HkError::Internal(e.to_string()))?;

    conn.execute(
        "INSERT INTO ItemTable (key, value) VALUES ('agentPlugins.enablement', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![new_value],
    )
    .map_err(|e| HkError::Internal(format!("Failed to update VS Code state DB: {}", e)))?;

    Ok(())
}

/// Remove a plugin entry from VS Code's state.vscdb enablement array.
pub fn remove_vscode_plugin_entry(
    vscode_user_dir: &Path,
    plugin_uri: &str,
) -> Result<(), HkError> {
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

    let mut entries: Vec<(String, bool)> =
        serde_json::from_str(&current).unwrap_or_default();

    entries.retain(|e| e.0 != plugin_uri);

    let new_value = serde_json::to_string(&entries)
        .map_err(|e| HkError::Internal(e.to_string()))?;

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

    let output = serde_json::to_string_pretty(&config)
        .map_err(|e| HkError::Internal(e.to_string()))?;
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

/// Ensure Codex hooks feature is enabled in config.toml.
/// Codex requires `[features] codex_hooks = true` to activate hook support.
pub fn ensure_codex_hooks_enabled(codex_base_dir: &Path) -> Result<(), HkError> {
    let config_toml = codex_base_dir.join("config.toml");
    let content = if config_toml.exists() {
        std::fs::read_to_string(&config_toml)?
    } else {
        String::new()
    };
    // Check if already enabled
    if content.contains("codex_hooks") {
        return Ok(());
    }
    // Append the feature flag
    let mut new_content = content;
    if !new_content.ends_with('\n') && !new_content.is_empty() {
        new_content.push('\n');
    }
    new_content.push_str("\n[features]\ncodex_hooks = true\n");
    atomic_write(&config_toml, &new_content)?;
    Ok(())
}

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
        _ => {
            let config = read_or_create_json(config_path)?;
            let key = match format {
                McpFormat::Servers => "servers",
                _ => "mcpServers",
            };
            Ok(config.get(key).and_then(|v| v.get(server_name)).cloned())
        }
    }
}

/// Read a hook entry's full JSON value from a config file.
pub fn read_hook_config(
    config_path: &Path,
    event: &str,
    matcher: Option<&str>,
    command: &str,
    format: HookFormat,
) -> Result<Option<serde_json::Value>, HkError> {
    if !config_path.exists() {
        return Ok(None);
    }
    let config = read_or_create_json(config_path)?;
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
        };
        deploy_mcp_server(&config, &entry, McpFormat::McpServers).unwrap();

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
        };
        deploy_mcp_server(&config, &entry, McpFormat::McpServers).unwrap();

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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Servers).unwrap();

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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

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
    fn test_sanitize_mcp_name_replaces_slash() {
        assert_eq!(sanitize_mcp_name("microsoft/markitdown"), "microsoft-markitdown");
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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

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
        assert!(resolved.starts_with('/'), "expected absolute path, got: {resolved}");
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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

        // Read using the original (unsanitized) name
        let result = read_mcp_server_config(&config, "microsoft/markitdown", McpFormat::Toml)
            .unwrap();
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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

        // Remove using the original name
        remove_mcp_server(&config, "microsoft/markitdown", McpFormat::Toml).unwrap();

        // Verify it's gone
        let result = read_mcp_server_config(&config, "microsoft/markitdown", McpFormat::Toml)
            .unwrap();
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
        };
        deploy_mcp_server(&config, &entry, McpFormat::Toml).unwrap();

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
        };
        deploy_hook(&config, &entry, HookFormat::Windsurf).unwrap();

        let content: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(content.get("version").is_none());
        assert_eq!(content["hooks"]["pre_user_prompt"][0]["command"], "echo hi");
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
        let hooks = content["hooks"]["post_cascade_response"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "echo other");
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
        ).unwrap();
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
        ).unwrap();
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
        ).unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        ).unwrap();
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
        ).unwrap();

        set_gemini_extension_enabled(&ext_dir, "my-ext", false, home).unwrap();

        let content: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(ext_dir.join("extension-enablement.json")).unwrap(),
        ).unwrap();
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
        ).unwrap();
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

        let content: toml::Table = std::fs::read_to_string(&config)
            .unwrap()
            .parse()
            .unwrap();
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
        ).unwrap();

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
