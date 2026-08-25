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
            from_manifest: false,
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
        mcp_transport: None,
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
            from_manifest: false,
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
        mcp_transport: None,
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
            from_manifest: false,
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
        mcp_transport: None,
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
    store
        .set_disabled_config("plugin-1", Some(r#"{"key":"val"}"#))
        .unwrap();

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
    store
        .set_disabled_config("plugin-1", Some(r#"{"plugin_key":"k","value":true}"#))
        .unwrap();

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

#[test]
fn test_dsh_mcp_native_toggle_roundtrip() {
    use hk_core::adapter::dsh::DshAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();
    std::fs::create_dir_all(dir.path().join(".dsh")).unwrap();
    std::fs::write(
        dir.path().join(".dsh/cordis.patch.yml"),
        r#"- insert:
    - id: mcp-github
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: github
        transport: stdio
        command: npx
"#,
    )
    .unwrap();

    let adapter = DshAdapter::with_home(dir.path().to_path_buf());
    let servers = adapter.read_mcp_servers();
    assert_eq!(servers.len(), 1);
    assert!(servers[0].enabled);

    // Store the extension the way the scanner would, then toggle through the
    // manager with only the dsh adapter mounted.
    let exts = hk_core::scanner::scan_mcp_servers(&adapter);
    assert_eq!(exts.len(), 1);
    store.sync_extensions(&exts).unwrap();
    let ext_id = store.list_extensions(None, None).unwrap()[0].id.clone();

    let adapters: Vec<Box<dyn AgentAdapter>> =
        vec![Box::new(DshAdapter::with_home(dir.path().to_path_buf()))];
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, false).unwrap();

    // On-disk state flipped via the managed block; no DB snapshot taken.
    let servers = DshAdapter::with_home(dir.path().to_path_buf()).read_mcp_servers();
    assert!(!servers[0].enabled);
    assert!(store.get_disabled_config(&ext_id).unwrap().is_none());

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, true).unwrap();
    let servers = DshAdapter::with_home(dir.path().to_path_buf()).read_mcp_servers();
    assert!(servers[0].enabled);
}

/// Cross-layer joint test for the dsh plugin toggle: the SCANNER
/// (`adapter::dsh::read_plugins`, which folds profile row + home override and
/// deliberately INCLUDES HK's managed block, because the block is effective
/// state dsh applies) against the WRITER (`deployer::set_dsh_plugin_enabled`,
/// which folds owning layer + home *user* text and deliberately EXCLUDES the
/// block, because the block is HK's own output, not base state).
///
/// Both implement the same upstream compose rule with opposite block
/// treatment, and each is otherwise only covered in isolation — so the round
/// trip is the only thing that catches them drifting apart. In particular, if
/// the writer ever folded its own block back in, the re-enable below would
/// compute "already at base", write nothing, and the plugin would stay
/// disabled forever.
#[test]
fn test_dsh_profile_plugin_toggle_roundtrip_scanner_and_writer_agree() {
    use hk_core::adapter::dsh::DshAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    // A real .dsh tree: one profile whose patch layer DEFINES the plugin row
    // (`tool-policy`), with the package present in its node_modules. No home
    // patch yet — the writer must create it.
    let profile = dir.path().join(".dsh/profiles/web");
    std::fs::create_dir_all(profile.join("node_modules/dsh-plugin-tool")).unwrap();
    std::fs::write(
        profile.join("package.json"),
        r#"{"dependencies": {"dsh-plugin-tool": "1.0.0"}, "dsh": {"profile": {"bundles": []}}}"#,
    )
    .unwrap();
    let profile_patch = profile.join("cordis.patch.yml");
    let profile_patch_text =
        "- insert:\n    - id: tool-policy\n      name: dsh-plugin-tool\n      config:\n        mode: strict\n";
    std::fs::write(&profile_patch, profile_patch_text).unwrap();

    let adapter = || DshAdapter::with_home(dir.path().to_path_buf());
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];
    let rescan = || {
        let exts = hk_core::scanner::scan_plugins(&adapter());
        // Named by its patch row id, exactly as dsh's own plugin list shows
        // it; the package (`dsh-plugin-tool`) rides in the description.
        let row = exts
            .into_iter()
            .find(|e| e.name == "tool-policy")
            .expect("profile row must survive every rescan");
        (row.id.clone(), row.enabled)
    };

    // Scan → enabled, and the row is stored the way the manager will find it.
    let (ext_id, enabled) = rescan();
    assert!(enabled, "a plain profile row starts enabled");
    store
        .sync_extensions(&hk_core::scanner::scan_plugins(&adapter()))
        .unwrap();

    // Toggle off → rescan reads the writer's block back as disabled.
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, false).unwrap();
    let (id_after, enabled) = rescan();
    assert_eq!(id_after, ext_id, "identity is stable across the toggle");
    assert!(!enabled, "scanner must see the writer's managed block");
    let home = std::fs::read_to_string(dir.path().join(".dsh/cordis.patch.yml")).unwrap();
    assert!(home.contains("- id: tool-policy\n  disabled: true"), "{home}");
    // Only the home file is written; the profile layer is untouched.
    assert_eq!(
        std::fs::read_to_string(&profile_patch).unwrap(),
        profile_patch_text
    );

    // Toggle on → back to the profile's own base state, so the whole block
    // goes away rather than becoming a redundant `disabled: false`.
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, true).unwrap();
    let (id_after, enabled) = rescan();
    assert_eq!(id_after, ext_id);
    assert!(enabled, "re-enable must be visible to the scanner");
    let home = std::fs::read_to_string(dir.path().join(".dsh/cordis.patch.yml")).unwrap();
    assert!(!home.contains("managed by HarnessKit"), "block gone: {home}");
    assert!(!home.contains("disabled"), "no leftover override: {home}");
}

/// A row provided by a mounted BUNDLE is toggleable exactly like a user row:
/// the write lands in the HOME patch and the bundle's own patch file — which
/// HK must never edit — stays byte-identical. Also pins the `hmr` case: the
/// row is DEFINED enabled by one bundle and DISABLED by a later one, so the
/// base state only comes out right if the whole layer chain is folded.
#[test]
fn test_dsh_bundle_row_toggle_writes_home_patch_and_never_the_bundle_patch() {
    use hk_core::adapter::dsh::DshAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();

    // dsh's symlink farm with two in-box bundles, mirroring rc.6: base
    // defines `timer` and `hmr`; web-app disables `hmr` two layers later.
    let farm = dir.path().join(".dsh/profiles/node_modules/@deepseek-ai");
    let base_patch_text = "- insert:\n    - id: timer\n      name: '@deepseek-ai/cordis-plugin-timer'\n    - id: hmr\n      name: '@deepseek-ai/cordis-plugin-hmr'\n";
    let web_app_patch_text = "- id: hmr\n  disabled: true\n";
    for (pkg, patch) in [
        ("dsh-base", base_patch_text),
        ("dsh-web-app", web_app_patch_text),
    ] {
        let d = farm.join(pkg);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("package.json"),
            r#"{"dsh": {"bundle": {"patch": "./cordis.patch.yml"}}}"#,
        )
        .unwrap();
        std::fs::write(d.join("cordis.patch.yml"), patch).unwrap();
    }
    let base_patch = farm.join("dsh-base/cordis.patch.yml");
    let web_app_patch = farm.join("dsh-web-app/cordis.patch.yml");

    let profile = dir.path().join(".dsh/profiles/web");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(
        profile.join("package.json"),
        r#"{"dependencies": {}, "dsh": {"profile": {"bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"]}}}"#,
    )
    .unwrap();

    let adapter = || DshAdapter::with_home(dir.path().to_path_buf());
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];
    let home = dir.path().join(".dsh/cordis.patch.yml");
    let rescan = |name: &str| {
        hk_core::scanner::scan_plugins(&adapter())
            .into_iter()
            .find(|e| e.name == name)
            .map(|e| (e.id.clone(), e.enabled))
            .expect("bundle row must survive every rescan")
    };
    let bundle_patches_untouched = || {
        assert_eq!(std::fs::read_to_string(&base_patch).unwrap(), base_patch_text);
        assert_eq!(
            std::fs::read_to_string(&web_app_patch).unwrap(),
            web_app_patch_text
        );
    };

    // The bundle itself is not an extension; only its rows are — under
    // neither its package name nor the `package <pkg>` slot of a row source.
    let exts = hk_core::scanner::scan_plugins(&adapter());
    assert!(exts.iter().all(|e| e.name != "@deepseek-ai/dsh-base"
        && !e.description.ends_with("package @deepseek-ai/dsh-base")));
    store.sync_extensions(&exts).unwrap();

    // `timer`: enabled at base state → disable writes an override.
    let (timer_id, enabled) = rescan("timer");
    assert!(enabled);
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &timer_id, false).unwrap();
    let (_, enabled) = rescan("timer");
    assert!(!enabled, "scanner must see the writer's managed block");
    let home_text = std::fs::read_to_string(&home).unwrap();
    assert!(home_text.contains("- id: timer\n  disabled: true"), "{home_text}");
    bundle_patches_untouched();

    // Back to base state → the block goes away entirely.
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &timer_id, true).unwrap();
    assert!(rescan("timer").1);
    let home_text = std::fs::read_to_string(&home).unwrap();
    assert!(!home_text.contains("managed by HarnessKit"), "{home_text}");
    bundle_patches_untouched();

    // `hmr`: base state is DISABLED (by the second bundle), so enabling must
    // write an explicit `disabled: false` rather than being a silent no-op —
    // the whole-chain fold is what makes this correct.
    let (hmr_id, enabled) = rescan("hmr");
    assert!(!enabled, "the web-app bundle layer disables hmr");
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &hmr_id, true).unwrap();
    let (_, enabled) = rescan("hmr");
    assert!(enabled, "enable must survive the rescan, not fold back to base");
    let home_text = std::fs::read_to_string(&home).unwrap();
    assert!(home_text.contains("- id: hmr\n  disabled: false"), "{home_text}");
    bundle_patches_untouched();
}

#[test]
fn test_grok_mcp_native_toggle_roundtrip_and_preserves_unrelated() {
    use hk_core::adapter::grok::GrokAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();
    let adapter = || GrokAdapter::with_home(dir.path().to_path_buf());
    std::fs::create_dir_all(dir.path().join(".grok")).unwrap();
    std::fs::write(
        dir.path().join(".grok/config.toml"),
        r#"theme = "dark"
startup_timeout_sec = 12

[mcp_servers.github]
command = "npx"
args = ["-y", "@mcp/github"]
cwd = "/tmp/work"
"#,
    )
    .unwrap();

    let exts = hk_core::scanner::scan_mcp_servers(&adapter());
    assert_eq!(exts.len(), 1);
    assert!(exts[0].enabled);
    store.sync_extensions(&exts).unwrap();
    let ext_id = store.list_extensions(None, None).unwrap()[0].id.clone();
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, false).unwrap();
    let servers = adapter().read_mcp_servers();
    assert!(!servers[0].enabled);
    assert!(store.get_disabled_config(&ext_id).unwrap().is_none());
    let user = std::fs::read_to_string(dir.path().join(".grok/config.toml")).unwrap();
    assert!(user.contains("disabled_mcp_servers"));
    assert!(user.contains("github"));
    assert!(user.contains("theme"));
    assert!(user.contains("startup_timeout_sec"));
    assert!(user.contains("cwd"));

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, false).unwrap();
    let again = adapter().read_mcp_servers();
    assert!(!again[0].enabled, "disable is idempotent");

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, true).unwrap();
    assert!(adapter().read_mcp_servers()[0].enabled);
    let user = std::fs::read_to_string(dir.path().join(".grok/config.toml")).unwrap();
    assert!(
        !user.contains("disabled_mcp_servers"),
        "empty disable list is removed: {user}"
    );
}

#[test]
fn test_grok_project_mcp_disable_does_not_rewrite_project_file() {
    use hk_core::adapter::grok::GrokAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();
    let adapter = || GrokAdapter::with_home(dir.path().to_path_buf());
    std::fs::create_dir_all(dir.path().join(".grok")).unwrap();
    std::fs::write(dir.path().join(".grok/config.toml"), "theme = \"dark\"\n").unwrap();

    let project = dir.path().join("repo");
    let project_cfg = project.join(".grok/config.toml");
    std::fs::create_dir_all(project_cfg.parent().unwrap()).unwrap();
    let original = r#"# keep this comment if we can
[mcp_servers.shared]
command = "echo"
cwd = "/shared"
enabled = true
"#;
    std::fs::write(&project_cfg, original).unwrap();

    let exts = hk_core::scanner::scan_project_extensions(&adapter(), "repo", &project);
    let mcp = exts
        .into_iter()
        .find(|e| e.kind == hk_core::models::ExtensionKind::Mcp && e.name == "shared")
        .expect("project MCP");
    assert!(mcp.enabled);
    store.register_project_by_path(&project.to_string_lossy());
    store.sync_extensions(std::slice::from_ref(&mcp)).unwrap();
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &mcp.id, false).unwrap();
    let after_disable = std::fs::read_to_string(&project_cfg).unwrap();
    assert_eq!(
        after_disable, original,
        "personal disable must not rewrite the shared project file"
    );
    let user = std::fs::read_to_string(dir.path().join(".grok/config.toml")).unwrap();
    assert!(user.contains("disabled_mcp_servers"));
    assert!(user.contains("theme"));
    assert!(!adapter().read_mcp_servers_from(&project_cfg)[0].enabled);

    // A later sticky `enabled = false` on the shared file is unstuck only
    // when the user re-enables (store still has enabled=false here).
    std::fs::write(
        &project_cfg,
        r#"
[mcp_servers.shared]
command = "echo"
cwd = "/shared"
enabled = false
other = "keep-me"
"#,
    )
    .unwrap();
    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &mcp.id, true).unwrap();
    let unstuck = std::fs::read_to_string(&project_cfg).unwrap();
    assert!(unstuck.contains("keep-me"), "unrelated keys survive unstick: {unstuck}");
    assert!(adapter().read_mcp_servers_from(&project_cfg)[0].enabled);
}

#[test]
fn test_grok_hook_native_toggle_roundtrip() {
    use hk_core::adapter::grok::GrokAdapter;
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();
    let adapter = || GrokAdapter::with_home(dir.path().to_path_buf());
    let hook_file = dir.path().join(".grok/hooks/session-start.json");
    std::fs::create_dir_all(hook_file.parent().unwrap()).unwrap();
    std::fs::write(
        &hook_file,
        r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo hi"}]}]}}"#,
    )
    .unwrap();

    let exts = hk_core::scanner::scan_hooks(&adapter());
    assert_eq!(exts.len(), 1);
    assert!(exts[0].enabled);
    store.sync_extensions(&exts).unwrap();
    let ext_id = store.list_extensions(None, None).unwrap()[0].id.clone();
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, false).unwrap();
    let hooks = adapter().read_hooks();
    assert_eq!(hooks.len(), 1);
    assert!(!hooks[0].enabled);
    let disabled = std::fs::read_to_string(dir.path().join(".grok/disabled-hooks")).unwrap();
    assert!(disabled.contains("global/session-start:pre_tool_use[0].hooks[0]"));
    assert!(store.get_disabled_config(&ext_id).unwrap().is_none());

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &ext_id, true).unwrap();
    assert!(adapter().read_hooks()[0].enabled);
}

#[test]
fn test_grok_plugin_native_toggle_roundtrip() {
    use hk_core::adapter::grok::{grok_plugin_id, GrokAdapter};
    use hk_core::adapter::AgentAdapter;

    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("test.db")).unwrap();
    let adapter = || GrokAdapter::with_home(dir.path().to_path_buf());
    let plugin = dir.path().join(".grok/plugins/my-tool");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("plugin.json"), r#"{"name":"my-tool"}"#).unwrap();
    let id = grok_plugin_id("user", &plugin, "my-tool");
    std::fs::write(
        dir.path().join(".grok/config.toml"),
        format!("[plugins]\nenabled = [\"{id}\"]\n"),
    )
    .unwrap();

    let exts = hk_core::scanner::scan_plugins(&adapter());
    let row = exts.into_iter().find(|e| e.name == "my-tool").unwrap();
    assert!(row.enabled);
    store.sync_extensions(std::slice::from_ref(&row)).unwrap();
    let adapters: Vec<Box<dyn AgentAdapter>> = vec![Box::new(adapter())];

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &row.id, false).unwrap();
    assert!(!adapter().read_plugins()[0].enabled);
    let cfg = std::fs::read_to_string(dir.path().join(".grok/config.toml")).unwrap();
    assert!(cfg.contains("disabled"));
    assert!(store.get_disabled_config(&row.id).unwrap().is_none());

    hk_core::manager::toggle_extension_with_adapters(&store, &adapters, &row.id, true).unwrap();
    assert!(adapter().read_plugins()[0].enabled);
}
