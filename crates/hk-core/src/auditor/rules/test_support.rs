use crate::auditor::AuditInput;
use crate::models::{ExtensionKind, Source, SourceOrigin};

pub(super) fn skill_input(content: &str) -> AuditInput {
    AuditInput {
        extension_id: "test".into(),
        kind: ExtensionKind::Skill,
        name: "test-skill".into(),
        content: content.into(),
        raw_config: String::new(),
        source: Source {
            origin: SourceOrigin::Local,
            url: None,
            version: None,
            commit_hash: None,
            from_manifest: false,
        },
        file_path: "SKILL.md".into(),
        mcp_command: None,
        mcp_args: vec![],
        mcp_env: Default::default(),
        installed_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        permissions: vec![],
        cli_parent_id: None,
        cli_meta: None,
        child_permissions: vec![],
        pack: None,
    }
}

pub(super) fn mcp_input(command: &str, args: Vec<&str>, env: Vec<(&str, &str)>) -> AuditInput {
    AuditInput {
        extension_id: "test".into(),
        kind: ExtensionKind::Mcp,
        name: "test-mcp".into(),
        content: String::new(),
        raw_config: String::new(),
        source: Source {
            origin: SourceOrigin::Local,
            url: None,
            version: None,
            commit_hash: None,
            from_manifest: false,
        },
        file_path: "config.json".into(),
        mcp_command: Some(command.into()),
        mcp_args: args.into_iter().map(String::from).collect(),
        mcp_env: env
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        installed_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        permissions: vec![],
        cli_parent_id: None,
        cli_meta: None,
        child_permissions: vec![],
        pack: None,
    }
}
