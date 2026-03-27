use anyhow::{Context, Result};
use std::path::Path;
use crate::adapter::{McpServerEntry, HookEntry};

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
    let mut config = read_or_create_json(config_path)?;
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
    write_json(config_path, &config)
}

/// Deploy a hook config entry into the target agent's config file.
/// Reads the existing JSON, appends the hook under "hooks" -> event, writes back.
pub fn deploy_hook(config_path: &Path, entry: &HookEntry) -> Result<()> {
    let mut config = read_or_create_json(config_path)?;
    let hooks = config.as_object_mut().context("Config is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let event_arr = hooks.as_object_mut().context("hooks is not an object")?
        .entry(&entry.event)
        .or_insert_with(|| serde_json::json!([]));
    let arr = event_arr.as_array_mut().context("hook event is not an array")?;

    // Find an existing hook group with the same matcher, or create a new one
    let matcher_val = entry.matcher.as_deref().map(serde_json::Value::from);
    let group = arr.iter_mut().find(|h| {
        h.get("matcher").and_then(|v| v.as_str()).map(String::from) == entry.matcher
    });
    if let Some(group) = group {
        // Append command to existing group's hooks array
        let cmds = group.as_object_mut().and_then(|o| o.entry("hooks").or_insert_with(|| serde_json::json!([])).as_array_mut());
        if let Some(cmds) = cmds {
            let cmd_val = serde_json::Value::from(entry.command.as_str());
            if !cmds.contains(&cmd_val) {
                cmds.push(cmd_val);
            }
        }
    } else {
        // Create a new hook group
        let mut group = serde_json::json!({ "hooks": [entry.command] });
        if let Some(m) = &matcher_val {
            group.as_object_mut().unwrap().insert("matcher".into(), m.clone());
        }
        arr.push(group);
    }
    write_json(config_path, &config)
}

fn read_or_create_json(path: &Path) -> Result<serde_json::Value> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(serde_json::json!({}))
    }
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(value)?)?;
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
        deploy_hook(&config, &entry).unwrap();

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
        deploy_hook(&config, &entry).unwrap();

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
        deploy_hook(&config, &entry).unwrap();

        let content: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&config).unwrap()).unwrap();
        let hooks = content["hooks"]["PreToolUse"][0]["hooks"].as_array().unwrap();
        assert_eq!(hooks.len(), 1); // not duplicated
    }
}
