//! Integration tests: full enable/disable roundtrip for various extension types.

use hk_core::models::*;
use hk_core::scanner::scan_skill_dir;
use hk_core::store::Store;
use tempfile::TempDir;

#[test]
fn test_skill_disable_enable_roundtrip() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    // Set up skill directory
    let skill_dir = dir.path().join("skills");
    let my_skill = skill_dir.join("my-skill");
    std::fs::create_dir_all(&my_skill).unwrap();
    std::fs::write(
        my_skill.join("SKILL.md"),
        "---\nname: my-skill\ndescription: test\n---\nHello",
    )
    .unwrap();

    // Phase 1: Initial scan — skill is enabled
    let exts = scan_skill_dir(&skill_dir, "claude");
    assert_eq!(exts.len(), 1);
    assert!(exts[0].enabled);
    store.sync_extensions(&exts).unwrap();

    let all = store.list_extensions(None, None).unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].enabled);
    let ext_id = all[0].id.clone();

    // Phase 2: Disable — rename SKILL.md → SKILL.md.disabled
    std::fs::rename(
        my_skill.join("SKILL.md"),
        my_skill.join("SKILL.md.disabled"),
    )
    .unwrap();
    store.set_enabled(&ext_id, false).unwrap();

    // Phase 3: Re-scan — disabled skill should be found with enabled=false
    let exts = scan_skill_dir(&skill_dir, "claude");
    assert_eq!(exts.len(), 1, "Scanner should find disabled skill");
    assert!(!exts[0].enabled, "Disabled skill should have enabled=false");
    assert_eq!(
        exts[0].id, ext_id,
        "ID should be stable across enable/disable"
    );
    store.sync_extensions(&exts).unwrap();

    let fetched = store.get_extension(&ext_id).unwrap().unwrap();
    assert!(
        !fetched.enabled,
        "Disabled extension should survive re-scan"
    );

    // Phase 4: Re-enable — rename back
    std::fs::rename(
        my_skill.join("SKILL.md.disabled"),
        my_skill.join("SKILL.md"),
    )
    .unwrap();
    store.set_enabled(&ext_id, true).unwrap();

    // Phase 5: Re-scan — should be enabled again
    let exts = scan_skill_dir(&skill_dir, "claude");
    assert_eq!(exts.len(), 1);
    assert!(exts[0].enabled);
    store.sync_extensions(&exts).unwrap();

    let fetched = store.get_extension(&ext_id).unwrap().unwrap();
    assert!(
        fetched.enabled,
        "Re-enabled extension should work after scan"
    );
}

#[test]
fn test_disabled_mcp_survives_rescan() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    // Insert MCP extension and disable it (simulating config removal)
    let ext = Extension {
        id: "mcp-test".into(),
        kind: ExtensionKind::Mcp,
        name: "github".into(),
        description: "".into(),
        source: Source {
            origin: SourceOrigin::Agent,
            url: None,
            version: None,
            commit_hash: None,
        },
        agents: vec!["claude".into()],
        tags: vec![],
        pack: None,
        permissions: vec![],
        enabled: true,
        trust_score: None,
        installed_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),

        source_path: None,
        cli_parent_id: None,
        cli_meta: None,
        install_meta: None,
        scope: ConfigScope::Global,
    };
    store.insert_extension(&ext).unwrap();
    store.set_enabled("mcp-test", false).unwrap();
    store
        .set_disabled_config(
            "mcp-test",
            Some(r#"{"command":"npx","args":["-y","@mcp/github"]}"#),
        )
        .unwrap();

    // Sync with empty results (MCP removed from config file)
    store.sync_extensions(&[]).unwrap();

    // Disabled MCP should survive the sync
    let fetched = store.get_extension("mcp-test").unwrap();
    assert!(fetched.is_some(), "Disabled MCP should survive sync");
    let fetched = fetched.unwrap();
    assert!(!fetched.enabled);

    // Saved config should still be available for re-enable
    let saved = store.get_disabled_config("mcp-test").unwrap();
    assert!(saved.is_some(), "Disabled config should survive sync");
    assert!(saved.unwrap().contains("npx"));
}

#[test]
fn test_shared_skill_sibling_detection() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("test.db");
    let store = Store::open(&db_path).unwrap();

    let shared_path = dir.path().join("agents/skills/shared-skill/SKILL.md");

    // Create two extensions pointing to the same source_path (different agents)
    let ext1 = Extension {
        id: "shared-cursor".into(),
        kind: ExtensionKind::Skill,
        name: "shared-skill".into(),
        description: "".into(),
        source: Source {
            origin: SourceOrigin::Local,
            url: None,
            version: None,
            commit_hash: None,
        },
        agents: vec!["cursor".into()],
        tags: vec![],
        pack: None,
        permissions: vec![],
        enabled: true,
        trust_score: None,
        installed_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),

        source_path: Some(shared_path.to_string_lossy().to_string()),
        cli_parent_id: None,
        cli_meta: None,
        install_meta: None,
        scope: ConfigScope::Global,
    };
    store.insert_extension(&ext1).unwrap();

    let mut ext2 = ext1.clone();
    ext2.id = "shared-codex".into();
    ext2.agents = vec!["codex".into()];
    store.insert_extension(&ext2).unwrap();

    // Find siblings
    let siblings = store.find_siblings_by_source_path("shared-cursor").unwrap();
    assert_eq!(siblings.len(), 2);
    assert!(siblings.contains(&"shared-cursor".to_string()));
    assert!(siblings.contains(&"shared-codex".to_string()));

    // Toggling one should allow toggling all siblings
    for sib_id in &siblings {
        store.set_enabled(sib_id, false).unwrap();
    }

    let e1 = store.get_extension("shared-cursor").unwrap().unwrap();
    let e2 = store.get_extension("shared-codex").unwrap().unwrap();
    assert!(!e1.enabled);
    assert!(!e2.enabled);
}

// ---------------------------------------------------------------------------
// Plugin toggle tests — reproduce Issue #16
// ---------------------------------------------------------------------------

fn sample_plugin(id: &str, agent: &str) -> Extension {
    Extension {
        id: id.into(),
        kind: ExtensionKind::Plugin,
        name: "test-plugin".into(),
        description: "Plugin from marketplace".into(),
        source: Source {
            origin: SourceOrigin::Agent,
            url: None,
            version: None,
            commit_hash: None,
        },
        agents: vec![agent.into()],
        tags: vec![],
        pack: None,
        permissions: vec![],
        enabled: true,
        trust_score: None,
        installed_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        source_path: None,
        cli_parent_id: None,
        cli_meta: None,
        install_meta: None,
        scope: ConfigScope::Global,
    }
}

/// Tests deployer primitives (remove_plugin_entry / restore_plugin_entry) in isolation.
/// Note: the current Claude toggle path uses set_plugin_enabled instead;
/// this test validates the legacy deployer APIs still used by non-Claude agents.
#[test]
fn test_plugin_disable_enable_roundtrip_store_level() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    // Set up settings.json with the plugin enabled
    let settings_path = dir.path().join("settings.json");
    std::fs::write(
        &settings_path,
        r#"{"enabledPlugins":{"test-plugin@marketplace":true}}"#,
    )
    .unwrap();

    let ext = sample_plugin("plugin-1", "claude");
    store.insert_extension(&ext).unwrap();

    // Phase 1: Disable — read value, save to disabled_config, remove from config
    let value = hk_core::deployer::read_plugin_config(&settings_path, "test-plugin@marketplace")
        .unwrap()
        .expect("Plugin should be in config");
    let saved = serde_json::json!({ "plugin_key": "test-plugin@marketplace", "value": value });
    store
        .set_disabled_config("plugin-1", Some(&saved.to_string()))
        .unwrap();
    hk_core::deployer::remove_plugin_entry(&settings_path, "test-plugin@marketplace").unwrap();
    store.set_enabled("plugin-1", false).unwrap();

    // Verify disabled state
    let fetched = store.get_extension("plugin-1").unwrap().unwrap();
    assert!(!fetched.enabled);
    assert!(store.get_disabled_config("plugin-1").unwrap().is_some());

    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(
        settings["enabledPlugins"]
            .get("test-plugin@marketplace")
            .is_none(),
        "Plugin should be removed from enabledPlugins"
    );

    // Phase 2: Re-enable — read disabled_config, restore to settings, clear saved
    let saved_str = store
        .get_disabled_config("plugin-1")
        .unwrap()
        .expect("disabled_config should exist for re-enable");
    let saved_obj: serde_json::Value = serde_json::from_str(&saved_str).unwrap();
    let plugin_key = saved_obj["plugin_key"].as_str().unwrap();
    let restore_value = saved_obj.get("value").unwrap();
    hk_core::deployer::restore_plugin_entry(&settings_path, plugin_key, restore_value).unwrap();
    store.set_disabled_config("plugin-1", None).unwrap();
    store.set_enabled("plugin-1", true).unwrap();

    // Verify re-enabled state
    let fetched = store.get_extension("plugin-1").unwrap().unwrap();
    assert!(fetched.enabled, "Should be enabled after re-enable");
    assert!(
        store.get_disabled_config("plugin-1").unwrap().is_none(),
        "disabled_config should be cleared"
    );

    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
    assert!(
        settings["enabledPlugins"]
            .get("test-plugin@marketplace")
            .is_some(),
        "Plugin should be restored in enabledPlugins"
    );
}

/// Scenario 3: Verify that sync_extensions does NOT overwrite enabled state
/// when HK is managing the extension (disabled_config is set).
#[test]
fn test_sync_preserves_enabled_after_toggle() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    let ext = sample_plugin("plugin-1", "claude");
    store.insert_extension(&ext).unwrap();
    store.set_enabled("plugin-1", false).unwrap();
    // Must set disabled_config so UPSERT knows HK manages this extension
    store.set_disabled_config("plugin-1", Some(r#"{"key":"val"}"#)).unwrap();

    let scanned = sample_plugin("plugin-1", "claude"); // enabled: true from scanner
    store.sync_extensions(&[scanned]).unwrap();

    let fetched = store.get_extension("plugin-1").unwrap().unwrap();
    assert!(!fetched.enabled, "HK-managed disable should survive rescan");
}

/// External disable (e.g. user runs `/plugin disable` in Claude Code)
/// should be reflected in HK after rescan.
#[test]
fn test_rescan_syncs_external_disable() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    // First scan: plugin enabled
    let ext = sample_plugin("plugin-1", "claude");
    store.sync_extensions(&[ext]).unwrap();
    assert!(store.get_extension("plugin-1").unwrap().unwrap().enabled);

    // External change: scanner reports disabled
    let mut ext_disabled = sample_plugin("plugin-1", "claude");
    ext_disabled.enabled = false;
    store.sync_extensions(&[ext_disabled]).unwrap();

    // No disabled_config → HK is not managing this → should sync
    assert!(
        !store.get_extension("plugin-1").unwrap().unwrap().enabled,
        "External disable should be reflected after rescan"
    );
}

/// HK-managed disable must NOT be overwritten by rescan.
#[test]
fn test_rescan_preserves_hk_managed_disable() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    let ext = sample_plugin("plugin-1", "claude");
    store.sync_extensions(&[ext]).unwrap();
    store.set_enabled("plugin-1", false).unwrap();
    store.set_disabled_config("plugin-1", Some(r#"{"plugin_key":"k","value":true}"#)).unwrap();

    // Scanner says enabled (stale) but HK has disabled_config
    let ext_enabled = sample_plugin("plugin-1", "claude");
    store.sync_extensions(&[ext_enabled]).unwrap();

    assert!(
        !store.get_extension("plugin-1").unwrap().unwrap().enabled,
        "HK-managed disable must survive rescan"
    );
}

/// Scenario 4: Single instance (single agent) buildGroups + toggle simulation.
/// Verifies that the frontend's optimistic update pattern works correctly
/// when there's only one instance in the group.
#[test]
fn test_single_agent_extension_toggle_state() {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    // Single agent, single plugin
    let ext = sample_plugin("plugin-1", "claude");
    store.insert_extension(&ext).unwrap();

    // Simulate toggle: set enabled to false
    store.set_enabled("plugin-1", false).unwrap();
    let all = store.list_extensions(None, None).unwrap();
    assert_eq!(all.len(), 1);
    assert!(!all[0].enabled, "Single extension should show disabled");

    // Simulate toggle back: set enabled to true
    store.set_enabled("plugin-1", true).unwrap();
    let all = store.list_extensions(None, None).unwrap();
    assert_eq!(all.len(), 1);
    assert!(all[0].enabled, "Single extension should show enabled");
}
