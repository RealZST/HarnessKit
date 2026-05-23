use crate::adapter::all_adapters;
use crate::kits::source_resolver::{find_extension_source_by_id, strip_secrets_from_env};
use crate::models::{ConfigScope, ExtensionKind};
use std::collections::HashMap;

#[test]
fn find_extension_source_by_id_returns_none_for_unknown_id() {
    let adapters = all_adapters();
    let res = find_extension_source_by_id(
        &adapters,
        "no-such-id",
        ExtensionKind::Skill,
        &["claude".to_string()],
        &ConfigScope::Global,
        &[],
    );
    assert!(res.is_none());
}

#[test]
fn find_mcp_source_routes_to_project_mcp_file_when_scope_is_project() {
    // Repro for the original bug: a project-scoped MCP must be resolved against
    // the project's `.mcp.json`, not the agent's global config (which was the
    // hard-coded behavior before this fix).
    let tmp = tempfile::tempdir().unwrap();
    let project_path = tmp.path().to_path_buf();
    std::fs::write(
        project_path.join(".mcp.json"),
        r#"{"mcpServers":{"my-server":{"command":"echo"}}}"#,
    )
    .unwrap();

    let adapters = all_adapters();
    let scope = ConfigScope::Project {
        name: "tmp-proj".into(),
        path: project_path.to_string_lossy().into(),
    };

    // ext_id is unused for MCP dispatch (the entry-by-name lookup happens later
    // at pack time), so any string works here.
    let loc = find_extension_source_by_id(
        &adapters,
        "any-id",
        ExtensionKind::Mcp,
        &["claude".to_string()],
        &scope,
        &[],
    )
    .expect("project-scoped MCP resolves to the project's .mcp.json");
    assert_eq!(loc.entry_path, project_path.join(".mcp.json"));
    // `agent` must be populated so embed_extension can dispatch the
    // format-aware parser (JSON for claude here; TOML for codex, etc.).
    assert_eq!(loc.agent.as_deref(), Some("claude"));
}

#[test]
fn find_mcp_source_returns_none_when_project_file_missing() {
    // Project scope but no `.mcp.json` on disk → None (must NOT silently fall
    // back to the global ~/.claude.json — that would re-introduce the bug).
    let tmp = tempfile::tempdir().unwrap();
    let adapters = all_adapters();
    let scope = ConfigScope::Project {
        name: "tmp-proj".into(),
        path: tmp.path().to_string_lossy().into(),
    };
    let res = find_extension_source_by_id(
        &adapters,
        "any-id",
        ExtensionKind::Mcp,
        &["claude".to_string()],
        &scope,
        &[],
    );
    assert!(res.is_none());
}

#[test]
fn strip_secrets_blanks_secret_keys_keeps_others() {
    let mut env: HashMap<String, String> = [
        ("FIGMA_TOKEN".to_string(), "secret123".to_string()),
        ("NODE_ENV".to_string(), "production".to_string()),
    ]
    .into_iter()
    .collect();
    let was_stripped = strip_secrets_from_env(&mut env);
    assert!(was_stripped);
    assert_eq!(env["FIGMA_TOKEN"], "");
    // NODE_ENV is well-known non-secret — preserved as-is by the heuristic.
    assert_eq!(env["NODE_ENV"], "production");
}

#[test]
fn strip_secrets_reports_false_when_env_empty() {
    let mut env: HashMap<String, String> = HashMap::new();
    assert!(!strip_secrets_from_env(&mut env));
}

#[test]
fn strip_secrets_already_blank_secret_is_idempotent() {
    // Pre-blanked entries (e.g. re-importing an already-stripped Kit) must
    // NOT flip `was_stripped` to true — `secrets_stripped` in the manifest
    // would otherwise toggle on every re-pack with no new exposure.
    let mut env: HashMap<String, String> =
        [("API_KEY".to_string(), String::new())].into_iter().collect();
    assert!(!strip_secrets_from_env(&mut env));
    assert_eq!(env["API_KEY"], "");
}

