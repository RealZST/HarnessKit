use anyhow::{Context, Result};
use std::path::Path;
use crate::adapter::{McpServerEntry, HookEntry, HookFormat};
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
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            if entry.file_name() == ".git" { continue; }
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Deploy an MCP server config entry into the target agent's config file.
/// Reads the existing JSON, inserts/overwrites the entry under "mcpServers", writes back.
pub fn deploy_mcp_server(config_path: &Path, entry: &McpServerEntry) -> Result<()> {
    locked_modify_json(config_path, |config| {
        let servers = config.as_object_mut().context("Config is not an object")?
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        let server_val = serde_json::json!({
            "command": entry.command,
            "args": entry.args,
            "env": entry.env,
        });
        servers.as_object_mut().context("mcpServers is not an object")?
            .insert(entry.name.clone(), server_val);
        Ok(())
    })
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
                if let Some(group) = group {
                    let cmds = group.as_object_mut().and_then(|o| o.entry("hooks").or_insert_with(|| serde_json::json!([])).as_array_mut());
                    if let Some(cmds) = cmds {
                        let cmd_val = serde_json::Value::from(entry.command.as_str());
                        if !cmds.contains(&cmd_val) {
                            cmds.push(cmd_val);
                        }
                    }
                } else {
                    let mut group = serde_json::json!({ "hooks": [entry.command] });
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
                let hook_val = serde_json::json!({ "type": "command", "bash": entry.command });
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
pub fn remove_mcp_server(config_path: &Path, server_name: &str) -> Result<()> {
    if !config_path.exists() { return Ok(()); }
    locked_modify_json(config_path, |config| {
        if let Some(servers) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
            servers.remove(server_name);
        }
        Ok(())
    })
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
                                let cmd_val = serde_json::Value::from(command);
                                cmds.retain(|c| c != &cmd_val);
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
                            h.get("bash").and_then(|v| v.as_str()) != Some(command)
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
pub fn restore_mcp_server(config_path: &Path, server_name: &str, entry: &serde_json::Value) -> Result<()> {
    locked_modify_json(config_path, |config| {
        let servers = config.as_object_mut().context("Config is not an object")?
            .entry("mcpServers")
            .or_insert_with(|| serde_json::json!({}));
        servers.as_object_mut().context("mcpServers is not an object")?
            .insert(server_name.to_string(), entry.clone());
        Ok(())
    })
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

/// Read an MCP server entry's full JSON value from a config file.
pub fn read_mcp_server_config(config_path: &Path, server_name: &str) -> Result<Option<serde_json::Value>> {
    if !config_path.exists() { return Ok(None); }
    let config = read_or_create_json(config_path)?;
    Ok(config.get("mcpServers")
        .and_then(|v| v.get(server_name))
        .cloned())
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
                    && cmds.iter().any(|c| c.as_str() == Some(command)) {
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
                if entry.get("bash").and_then(|v| v.as_str()) == Some(command) {
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
        deploy_mcp_server(&config, &entry).unwrap();

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
        deploy_mcp_server(&config, &entry).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(content["theme"], "dark"); // preserved
        assert_eq!(content["mcpServers"]["existing"]["command"], "node"); // preserved
        assert_eq!(content["mcpServers"]["new-server"]["command"], "python"); // added
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
        assert_eq!(hook["hooks"][0], "echo test");
    }

    #[test]
    fn test_deploy_hook_appends_to_existing_group() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("settings.json");
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
        assert_eq!(hooks[0], "echo first");
        assert_eq!(hooks[1], "echo second");
    }

    #[test]
    fn test_deploy_hook_no_duplicate_command() {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("hooks.json");
        std::fs::write(&config, r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":["echo test"]}]}}"#).unwrap();

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
        restore_mcp_server(&config, "github", &entry).unwrap();

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

        let entry = read_mcp_server_config(&config, "github").unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap()["command"], "npx");

        let missing = read_mcp_server_config(&config, "nonexistent").unwrap();
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
        let saved = read_mcp_server_config(&config, "github").unwrap().unwrap();
        remove_mcp_server(&config, "github").unwrap();

        let after_remove: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert!(after_remove["mcpServers"].get("github").is_none());

        restore_mcp_server(&config, "github", &saved).unwrap();
        let after_restore: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        assert_eq!(after_restore["mcpServers"]["github"]["command"], "npx");
    }
}
