use anyhow::{Context, Result};
use std::path::Path;
use crate::adapter::{McpServerEntry, McpFormat, HookEntry, HookFormat};
use fs2::FileExt;
use std::io::{Read as _, Write as _, Seek as _, SeekFrom};

pub fn deploy_skill(source_path: &Path, target_skill_dir: &Path) -> Result<String> {
    std::fs::create_dir_all(target_skill_dir)?;
    if source_path.is_dir() {
        let dir_name = source_path.file_name().context("Invalid source path")?.to_string_lossy().to_string();
        let dest = target_skill_dir.join(&dir_name);
        copy_dir_recursive(source_path, &dest)?;
        Ok(dir_name)
    } else {
        let file_name = source_path.file_name().context("Invalid source path")?.to_string_lossy().to_string();
        let dest = target_skill_dir.join(&file_name);
        std::fs::copy(source_path, &dest)?;
        Ok(file_name)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        // Skip symlinks to prevent symlink-following attacks
        if entry.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
            eprintln!("[hk] warning: skipping symlink: {}", entry.path().display());
            continue;
        }
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        // Re-check via symlink_metadata right before copy to close the TOCTOU
        // window between the readdir check above and the actual I/O below.
        // If the file was deleted between readdir and now, skip instead of aborting.
        let meta = match std::fs::symlink_metadata(&src_path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[hk] warning: cannot read metadata for {}: {e}", src_path.display());
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            eprintln!("[hk] warning: skipping symlink: {}", src_path.display());
            continue;
        }
        if meta.file_type().is_dir() {
            if entry.file_name() == ".git" { continue; }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Deploy an MCP server config entry into the target agent's config file.
/// Format varies by agent — see `McpFormat`.
pub fn deploy_mcp_server(config_path: &Path, entry: &McpServerEntry, format: McpFormat) -> Result<()> {
    match format {
        McpFormat::McpServers => deploy_mcp_server_json(config_path, entry, "mcpServers"),
        McpFormat::Servers => deploy_mcp_server_json(config_path, entry, "servers"),
        McpFormat::Toml => deploy_mcp_server_toml(config_path, entry),
    }
}

/// JSON-based MCP deploy (Claude, Gemini, Cursor, Antigravity, Copilot).
/// `top_key` is "mcpServers" or "servers" depending on the agent.
fn deploy_mcp_server_json(config_path: &Path, entry: &McpServerEntry, top_key: &str) -> Result<()> {
    locked_modify_json(config_path, |config| {
        let servers = config.as_object_mut().context("Config is not an object")?
            .entry(top_key)
            .or_insert_with(|| serde_json::json!({}));
        let server_val = serde_json::json!({
            "command": entry.command,
            "args": entry.args,
            "env": entry.env,
        });
        servers.as_object_mut().context(format!("{} is not an object", top_key))?
            .insert(entry.name.clone(), server_val);
        Ok(())
    })
}

/// TOML-based MCP deploy (Codex: ~/.codex/config.toml with [mcp_servers.<name>]).
fn deploy_mcp_server_toml(config_path: &Path, entry: &McpServerEntry) -> Result<()> {
    let parent = config_path.parent().context("Invalid config path")?;
    std::fs::create_dir_all(parent)?;

    // Read existing TOML or start fresh
    let existing = std::fs::read_to_string(config_path).unwrap_or_default();
    let mut doc: toml::Table = if existing.is_empty() {
        toml::Table::new()
    } else {
        existing.parse::<toml::Table>().context("Failed to parse TOML config")?
    };

    // Get or create [mcp_servers] table
    let mcp_servers = doc.entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .context("mcp_servers is not a table")?;

    // Build server entry table
    let mut server_table = toml::Table::new();
    server_table.insert("command".into(), toml::Value::String(entry.command.clone()));
    if !entry.args.is_empty() {
        server_table.insert("args".into(), toml::Value::Array(
            entry.args.iter().map(|a| toml::Value::String(a.clone())).collect()
        ));
    }
    if !entry.env.is_empty() {
        let mut env_table = toml::Table::new();
        for (k, v) in &entry.env {
            env_table.insert(k.clone(), toml::Value::String(v.clone()));
        }
        server_table.insert("env".into(), toml::Value::Table(env_table));
    }

    mcp_servers.insert(entry.name.clone(), toml::Value::Table(server_table));

    // Write back atomically
    atomic_write(config_path, &toml::to_string_pretty(&doc)?)?;

    Ok(())
}

/// Deploy a hook config entry into the target agent's config file.
/// Reads the existing JSON, appends the hook under "hooks" -> event, writes back.
pub fn deploy_hook(config_path: &Path, entry: &HookEntry, format: HookFormat) -> Result<()> {
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                let hooks = config.as_object_mut().context("Config is not an object")?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks.as_object_mut().context("hooks is not an object")?
                    .entry(&entry.event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr.as_array_mut().context("hook event is not an array")?;

                let matcher_val = entry.matcher.as_deref().map(serde_json::Value::from);
                let group = arr.iter_mut().find(|h| {
                    h.get("matcher").and_then(|v| v.as_str()).map(String::from) == entry.matcher
                });
                // Use object format {"type":"command","command":"..."} — accepted by Claude, required by Codex/Gemini
                let cmd_obj = serde_json::json!({ "type": "command", "command": entry.command });
                if let Some(group) = group {
                    let cmds = group.as_object_mut().and_then(|o| o.entry("hooks").or_insert_with(|| serde_json::json!([])).as_array_mut());
                    if let Some(cmds) = cmds {
                        if !cmds.iter().any(|c| c.get("command").and_then(|v| v.as_str()) == Some(&entry.command)) {
                            cmds.push(cmd_obj);
                        }
                    }
                } else {
                    let mut group = serde_json::json!({ "hooks": [cmd_obj] });
                    if let Some(m) = &matcher_val {
                        group.as_object_mut().unwrap().insert("matcher".into(), m.clone());
                    }
                    arr.push(group);
                }
            }
            HookFormat::Cursor => {
                config.as_object_mut().context("Config is not an object")?
                    .entry("version").or_insert(serde_json::json!(1));
                let hooks = config.as_object_mut().unwrap()
                    .entry("hooks").or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks.as_object_mut().context("hooks is not an object")?
                    .entry(&entry.event).or_insert_with(|| serde_json::json!([]));
                let arr = event_arr.as_array_mut().context("event is not an array")?;
                let hook_val = serde_json::json!({ "command": entry.command });
                if !arr.contains(&hook_val) { arr.push(hook_val); }
            }
            HookFormat::Copilot => {
                config.as_object_mut().context("Config is not an object")?
                    .entry("version").or_insert(serde_json::json!(1));
                let hooks = config.as_object_mut().unwrap()
                    .entry("hooks").or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks.as_object_mut().context("hooks is not an object")?
                    .entry(&entry.event).or_insert_with(|| serde_json::json!([]));
                let arr = event_arr.as_array_mut().context("event is not an array")?;
                let hook_val = serde_json::json!({ "type": "command", "command": entry.command });
                if !arr.contains(&hook_val) { arr.push(hook_val); }
            }
            HookFormat::None => {
                anyhow::bail!("Agent does not support hooks");
            }
        }
        Ok(())
    })
}

/// Remove an MCP server entry from a config file by name.
pub fn remove_mcp_server(config_path: &Path, server_name: &str, format: McpFormat) -> Result<()> {
    if !config_path.exists() { return Ok(()); }
    match format {
        McpFormat::Toml => {
            let content = std::fs::read_to_string(config_path)?;
            let mut doc: toml::Table = content.parse::<toml::Table>()?;
            if let Some(servers) = doc.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
                servers.remove(server_name);
            }
            atomic_write(config_path, &toml::to_string_pretty(&doc)?)?;
            Ok(())
        }
        _ => {
            locked_modify_json(config_path, |config| {
                let key = match format { McpFormat::Servers => "servers", _ => "mcpServers" };
                if let Some(servers) = config.get_mut(key).and_then(|v| v.as_object_mut()) {
                    servers.remove(server_name);
                }
                Ok(())
            })
        }
    }
}

/// Remove a specific hook command from a config file by event, matcher, and command.
/// Only removes the given command from the group's hooks array.
/// If the hooks array becomes empty, removes the group.
/// If the event array becomes empty, removes the event key.
pub fn remove_hook(config_path: &Path, event: &str, matcher: Option<&str>, command: &str, format: HookFormat) -> Result<()> {
    if !config_path.exists() { return Ok(()); }
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) {
                        for group in event_arr.iter_mut() {
                            let group_matcher = group.get("matcher").and_then(|v| v.as_str());
                            if group_matcher != matcher { continue; }
                            if let Some(cmds) = group.get_mut("hooks").and_then(|v| v.as_array_mut()) {
                                // Match both string format "cmd" and object format {"type":"command","command":"cmd"}
                                cmds.retain(|c| {
                                    if c.as_str() == Some(command) { return false; }
                                    if c.get("command").and_then(|v| v.as_str()) == Some(command) { return false; }
                                    true
                                });
                            }
                        }
                        event_arr.retain(|h| {
                            h.get("hooks").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(true)
                        });
                        if event_arr.is_empty() {
                            hooks.remove(event);
                        }
                    }
            }
            HookFormat::Cursor => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) {
                        let cmd_val = serde_json::json!({ "command": command });
                        event_arr.retain(|h| h != &cmd_val);
                        if event_arr.is_empty() {
                            hooks.remove(event);
                        }
                    }
            }
            HookFormat::Copilot => {
                if let Some(hooks) = config.get_mut("hooks").and_then(|v| v.as_object_mut())
                    && let Some(event_arr) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) {
                        event_arr.retain(|h| {
                            h.get("command").and_then(|v| v.as_str()) != Some(command)
                        });
                        if event_arr.is_empty() {
                            hooks.remove(event);
                        }
                    }
            }
            HookFormat::None => {
                anyhow::bail!("Agent does not support hooks");
            }
        }
        Ok(())
    })
}

/// Remove a plugin entry from a config file's enabledPlugins object by key.
pub fn remove_plugin_entry(config_path: &Path, plugin_key: &str) -> Result<()> {
    if !config_path.exists() { return Ok(()); }
    locked_modify_json(config_path, |config| {
        if let Some(plugins) = config.get_mut("enabledPlugins").and_then(|v| v.as_object_mut()) {
            plugins.remove(plugin_key);
        }
        Ok(())
    })
}

/// Restore a previously disabled MCP server entry into the config file.
pub fn restore_mcp_server(config_path: &Path, server_name: &str, entry: &serde_json::Value, format: McpFormat) -> Result<()> {
    match format {
        McpFormat::Toml => {
            // Convert saved JSON entry back to TOML and write
            let mcp_entry = McpServerEntry {
                name: server_name.to_string(),
                command: entry.get("command").and_then(|v| v.as_str()).unwrap_or("").into(),
                args: entry.get("args").and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                env: entry.get("env").and_then(|v| v.as_object())
                    .map(|obj| obj.iter().filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect())
                    .unwrap_or_default(),
            };
            deploy_mcp_server_toml(config_path, &mcp_entry)
        }
        _ => {
            let key = match format { McpFormat::Servers => "servers", _ => "mcpServers" };
            locked_modify_json(config_path, |config| {
                let servers = config.as_object_mut().context("Config is not an object")?
                    .entry(key)
                    .or_insert_with(|| serde_json::json!({}));
                servers.as_object_mut().context(format!("{key} is not an object"))?
                    .insert(server_name.to_string(), entry.clone());
                Ok(())
            })
        }
    }
}

/// Restore a previously disabled hook entry into the config file.
pub fn restore_hook(config_path: &Path, event: &str, entry: &serde_json::Value, format: HookFormat) -> Result<()> {
    locked_modify_json(config_path, |config| {
        match format {
            HookFormat::ClaudeLike => {
                let hooks = config.as_object_mut().context("Config is not an object")?
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks.as_object_mut().context("hooks is not an object")?
                    .entry(event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr.as_array_mut().context("hook event is not an array")?;
                arr.push(entry.clone());
            }
            HookFormat::Cursor | HookFormat::Copilot => {
                config.as_object_mut().context("Config is not an object")?
                    .entry("version").or_insert(serde_json::json!(1));
                let hooks = config.as_object_mut().unwrap()
                    .entry("hooks")
                    .or_insert_with(|| serde_json::json!({}));
                let event_arr = hooks.as_object_mut().context("hooks is not an object")?
                    .entry(event)
                    .or_insert_with(|| serde_json::json!([]));
                let arr = event_arr.as_array_mut().context("hook event is not an array")?;
                arr.push(entry.clone());
            }
            HookFormat::None => {
                anyhow::bail!("Agent does not support hooks");
            }
        }
        Ok(())
    })
}

/// Restore a previously disabled plugin entry into enabledPlugins.
pub fn restore_plugin_entry(config_path: &Path, plugin_key: &str, value: &serde_json::Value) -> Result<()> {
    locked_modify_json(config_path, |config| {
        let plugins = config.as_object_mut().context("Config is not an object")?
            .entry("enabledPlugins")
            .or_insert_with(|| serde_json::json!({}));
        plugins.as_object_mut().context("enabledPlugins is not an object")?
            .insert(plugin_key.to_string(), value.clone());
        Ok(())
    })
}

/// Ensure Codex hooks feature is enabled in config.toml.
/// Codex requires `[features] codex_hooks = true` to activate hook support.
pub fn ensure_codex_hooks_enabled(codex_base_dir: &Path) -> Result<()> {
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
pub fn read_mcp_server_config(config_path: &Path, server_name: &str, format: McpFormat) -> Result<Option<serde_json::Value>> {
    if !config_path.exists() { return Ok(None); }
    match format {
        McpFormat::Toml => {
            let content = std::fs::read_to_string(config_path)?;
            let doc: toml::Table = content.parse::<toml::Table>()?;
            let server = doc.get("mcp_servers")
                .and_then(|v| v.as_table())
                .and_then(|t| t.get(server_name));
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
            let key = match format { McpFormat::Servers => "servers", _ => "mcpServers" };
            Ok(config.get(key)
                .and_then(|v| v.get(server_name))
                .cloned())
        }
    }
}

/// Read a hook entry's full JSON value from a config file.
pub fn read_hook_config(config_path: &Path, event: &str, matcher: Option<&str>, command: &str, format: HookFormat) -> Result<Option<serde_json::Value>> {
    if !config_path.exists() { return Ok(None); }
    let config = read_or_create_json(config_path)?;
    let hooks = config.get("hooks").and_then(|v| v.as_object());
    let Some(hooks) = hooks else { return Ok(None); };
    let Some(event_arr) = hooks.get(event).and_then(|v| v.as_array()) else { return Ok(None); };
    match format {
        HookFormat::ClaudeLike => {
            for group in event_arr {
                let group_matcher = group.get("matcher").and_then(|v| v.as_str());
                if group_matcher != matcher { continue; }
                if let Some(cmds) = group.get("hooks").and_then(|v| v.as_array())
                    && cmds.iter().any(|c| {
                        // Match both string format "cmd" and object format {"command":"cmd"}
                        c.as_str() == Some(command)
                        || c.get("command").and_then(|v| v.as_str()) == Some(command)
                    }) {
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
pub fn read_plugin_config(config_path: &Path, plugin_key: &str) -> Result<Option<serde_json::Value>> {
    if !config_path.exists() { return Ok(None); }
    let config = read_or_create_json(config_path)?;
    Ok(config.get("enabledPlugins")
        .and_then(|v| v.get(plugin_key))
        .cloned())
}

fn read_or_create_json(path: &Path) -> Result<serde_json::Value> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(serde_json::json!({}))
    }
}

#[allow(dead_code)]
fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

/// Write content to a file atomically: write to a temp file, then rename.
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Read-modify-write a JSON config file with an exclusive advisory file lock.
fn locked_modify_json<F>(path: &Path, modify: F) -> Result<()>
where
    F: FnOnce(&mut serde_json::Value) -> Result<()>,
{
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .read(true).write(true).create(true).truncate(false)
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
        assert!(target_dir.path().join("my-skill").join("helper.py").exists());
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
        assert!(target_dir.path().join("git-skill").join("SKILL.md").exists());
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let server = &content["mcpServers"]["github"];
        assert_eq!(server["command"], "npx");
        assert_eq!(server["args"][0], "-y");
        assert_eq!(server["env"]["GITHUB_TOKEN"], "ghp_test");
    }

    #[test]
    fn test_deploy_mcp_server_existing_file() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"theme":"dark","mcpServers":{"existing":{"command":"node"}}}"#).unwrap();

        let entry = McpServerEntry {
            name: "new-server".into(),
            command: "python".into(),
            args: vec!["server.py".into()],
            env: Default::default(),
        };
        deploy_mcp_server(&config, &entry, McpFormat::McpServers).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(content.get("mcpServers").is_none(), "should not use mcpServers key");
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
        assert_eq!(server["args"].as_array().unwrap()[0].as_str().unwrap(), "-y");
        assert_eq!(server["env"]["MY_KEY"].as_str().unwrap(), "val");
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
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
        std::fs::write(&config, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo first"]}]}}"#).unwrap();

        let entry = HookEntry {
            event: "PreToolUse".into(),
            matcher: Some("Bash".into()),
            command: "echo second".into(),
        };
        deploy_hook(&config, &entry, HookFormat::ClaudeLike).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
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

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["hooks"]["PreToolUse"][0]["matcher"], "Bash");
        assert_eq!(content["hooks"]["PreToolUse"][0]["hooks"][0], "echo test");
    }

    #[test]
    fn test_restore_plugin_entry() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"enabledPlugins":{}}"#).unwrap();

        restore_plugin_entry(&config, "my-plugin@source", &serde_json::json!(true)).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["enabledPlugins"]["my-plugin@source"], true);
    }

    #[test]
    fn test_read_mcp_server_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"mcpServers":{"github":{"command":"npx","args":["-y"]}}}"#).unwrap();

        let entry = read_mcp_server_config(&config, "github", McpFormat::McpServers).unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["command"], "npx");

        let missing = read_mcp_server_config(&config, "nonexistent", McpFormat::McpServers).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_read_hook_config() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
        std::fs::write(&config, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo test"]}]}}"#).unwrap();

        let entry = read_hook_config(&config, "PreToolUse", Some("Bash"), "echo test", HookFormat::ClaudeLike).unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["matcher"], "Bash");

        let missing = read_hook_config(&config, "PreToolUse", Some("Bash"), "nonexistent", HookFormat::ClaudeLike).unwrap();
        assert!(missing.is_none());
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
        std::fs::write(&config, r#"{"mcpServers":{"github":{"command":"npx","args":["-y"],"env":{}}}}"#).unwrap();

        // Read, remove, restore
        let saved = read_mcp_server_config(&config, "github", McpFormat::McpServers).unwrap().unwrap();
        remove_mcp_server(&config, "github", McpFormat::McpServers).unwrap();

        let after_remove: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(after_remove["mcpServers"].get("github").is_none());

        restore_mcp_server(&config, "github", &saved, McpFormat::McpServers).unwrap();
        let after_restore: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after_restore["mcpServers"]["github"]["command"], "npx");
    }

    #[test]
    fn test_deploy_hook_cursor_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        let entry = HookEntry { event: "stop".into(), matcher: None, command: "echo done".into() };
        deploy_hook(&config, &entry, HookFormat::Cursor).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
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
        let entry = HookEntry { event: "PreToolUse".into(), matcher: None, command: "./check.sh".into() };
        deploy_hook(&config, &entry, HookFormat::Copilot).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["version"], 1);
        assert_eq!(content["hooks"]["PreToolUse"][0]["type"], "command");
        assert_eq!(content["hooks"]["PreToolUse"][0]["command"], "./check.sh");
    }

    #[test]
    fn test_remove_hook_cursor_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(&config, r#"{"version":1,"hooks":{"stop":[{"command":"echo done"},{"command":"echo other"}]}}"#).unwrap();

        remove_hook(&config, "stop", None, "echo done", HookFormat::Cursor).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let stops = content["hooks"]["stop"].as_array().unwrap();
        assert_eq!(stops.len(), 1);
        assert_eq!(stops[0]["command"], "echo other");
    }

    #[test]
    fn test_remove_hook_copilot_format() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(&config, r#"{"version":1,"hooks":{"PreToolUse":[{"type":"command","command":"./check.sh"},{"type":"command","command":"./other.sh"}]}}"#).unwrap();

        remove_hook(&config, "PreToolUse", None, "./check.sh", HookFormat::Copilot).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0]["command"], "./other.sh");
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
        assert!(!target_dir.path().join("my-skill").join("link-to-secret").exists());
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
}
