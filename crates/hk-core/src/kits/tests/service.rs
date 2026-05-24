use crate::adapter::all_adapters;
use crate::kits::manifest::{sha256_of_bytes, KitManifest, KIT_FORMAT_VERSION, ManifestExtension};
use crate::kits::service::{create_kit, delete_kit, export_kit, import_kit, list_kit_asset_candidates, list_kits, preview_kit_project_conflicts, sync_kit_to_project, unsync_kit_from_project, update_kit};
use crate::kits::types::{CreateKitRequest, KitConfigFileRef, PreviewKitConflictsRequest, SyncKitRequest, UnsyncKitRequest, UpdateKitRequest};
use crate::kits::zip_io::{pack_kit, read_manifest_from_zip, PackEntry};
use crate::models::{ConfigCategory, ConfigScope, Extension, ExtensionKind, Source, SourceOrigin};
use crate::store::{KitRow, Store};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

/// RAII guard that restores `HOME` on drop, so env pollution from one test
/// cannot affect later tests that run on other threads.
pub(super) struct RestoreHome(Option<std::ffi::OsString>);
impl RestoreHome {
    pub(super) fn new() -> Self {
        Self(std::env::var_os("HOME"))
    }
}
impl Drop for RestoreHome {
    fn drop(&mut self) {
        // SAFETY: called from the same thread that set HOME; `#[serial(kit_env)]`
        // ensures no other kit_env test runs concurrently at this point.
        unsafe {
            match &self.0 {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }
}

/// Create a basic single-config-file Kit named `name` for testing.
/// Writes a CLAUDE.md file at `dir/CLAUDE.md` with `content`, then creates
/// a Kit referencing it. Returns the created summary.
fn make_basic_kit(
    store: &Mutex<Store>,
    adapters: &[Box<dyn crate::adapter::AgentAdapter>],
    dir: &std::path::Path,
    name: &str,
    content: &str,
) -> crate::kits::types::KitSummary {
    let src = dir.join("CLAUDE.md");
    fs::write(&src, content).unwrap();
    create_kit(
        store,
        adapters,
        CreateKitRequest {
            name: name.into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![KitConfigFileRef {
                agent: "claude".into(),
                category: ConfigCategory::Rules,
                source_path: Some(src.to_string_lossy().into()),
                source_file_name: "CLAUDE.md".into(),
            }],
        },
    )
    .unwrap()
}

#[test]
#[serial(kit_env)]
fn create_kit_writes_zip_and_db_rows() {
    let dir = tempdir().unwrap();
    // Redirect HOME so kits land in our tempdir (avoid polluting real home).
    // SAFETY: this test is single-threaded at env mutation time; `#[serial(kit_env)]`
    // prevents concurrent mutation from other tests in this suite.
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let summary = make_basic_kit(&store, &adapters, dir.path(), "Empty-ext Test", "# real rules content");
    assert_eq!(summary.name, "Empty-ext Test");
    assert_eq!(summary.config_file_count, 1);

    let zip_path = dir.path().join(".harnesskit/kits").join(format!("{}.hk-kit.zip", summary.id));
    assert!(zip_path.exists(), "zip should exist at {}", zip_path.display());

    let manifest = read_manifest_from_zip(&zip_path).unwrap();
    assert_eq!(manifest.config_files.len(), 1);
    assert_eq!(manifest.config_files[0].filename, "CLAUDE.md");

    let listed = list_kits(&store).unwrap();
    assert_eq!(listed.len(), 1);
}

#[test]
#[serial(kit_env)]
fn create_kit_rejects_duplicate_name() {
    let dir = tempdir().unwrap();
    // SAFETY: single-threaded env mutation; `#[serial(kit_env)]` prevents races.
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let req = CreateKitRequest {
        name: "Dup".into(),
        description: "".into(),
        extension_ids: vec![],
        config_files: vec![],
    };
    create_kit(&store, &adapters, req.clone()).unwrap();
    let err = create_kit(&store, &adapters, req).unwrap_err();
    let msg = format!("{err}");
    // Frontend matches `kit-name-exists:` prefix to swap in a localized
    // message (see kit-editor-dialog.tsx handleSave). Keeping this tag
    // stable is the contract.
    assert!(
        msg.contains("kit-name-exists:"),
        "expected tagged duplicate-name error, got {msg}"
    );
}

#[test]
#[serial(kit_env)]
fn create_kit_treats_name_collisions_case_insensitively() {
    // SQLite's UNIQUE index on `kits.name` uses `COLLATE NOCASE`, so the
    // user-facing check must match — otherwise "Foo" and "foo" pass the
    // pre-flight check and then trip the DB constraint with the gibberish
    // error this whole change is meant to avoid.
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Foo".into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![],
        },
    )
    .unwrap();
    let err = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "foo".into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![],
        },
    )
    .unwrap_err();
    assert!(format!("{err}").contains("kit-name-exists:"));
}

#[test]
#[serial(kit_env)]
fn update_kit_allows_renaming_to_own_current_name() {
    // A no-op rename (saving the editor without changing the name) must
    // not be rejected by the duplicate-name check.
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let summary = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Solo".into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![],
        },
    )
    .unwrap();
    update_kit(
        &store,
        &adapters,
        UpdateKitRequest {
            id: summary.id,
            name: "Solo".into(),
            description: "new desc".into(),
            extension_ids: vec![],
            config_files: vec![],
        },
    )
    .expect("rename-to-self must succeed");
}

#[test]
#[serial(kit_env)]
fn list_kit_asset_candidates_filters_to_kit_able_kinds() {
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let candidates = list_kit_asset_candidates(&store, &adapters).unwrap();
    assert_eq!(candidates.extensions.len(), 0);
    let _ = candidates.config_files.len();
}

#[test]
#[serial(kit_env)]
fn delete_kit_cascades_assets_and_stack_but_keeps_sync_records() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let summary = make_basic_kit(&store, &adapters, dir.path(), "Doomed", "x");

    {
        let st = store.lock();
        st.upsert_sync_record(&crate::store::SyncRecordRow {
            id: "s1".into(),
            kit_id: summary.id.clone(),
            project_path: "/tmp/p".into(),
            agent_name: "claude".into(),
            written_paths: vec!["/tmp/p/CLAUDE.md".into()],
            synced_at: chrono::Utc::now(),
        })
        .unwrap();
    }

    delete_kit(&store, &summary.id).unwrap();

    let st = store.lock();
    assert!(st.get_kit_row(&summary.id).unwrap().is_none());
    assert_eq!(st.list_kit_assets(&summary.id).unwrap().len(), 0);
    assert_eq!(st.list_kit_config_files(&summary.id).unwrap().len(), 0);
    assert!(st.get_sync_record(&summary.id, "/tmp/p", "claude").unwrap().is_some());
}

#[test]
#[serial(kit_env)]
fn preview_reports_conflict_when_target_exists() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("CLAUDE.md"), "pre-existing").unwrap();

    let summary = make_basic_kit(&store, &adapters, dir.path(), "Preview Test", "x");

    let prev = preview_kit_project_conflicts(
        &store,
        &adapters,
        PreviewKitConflictsRequest {
            kit_id: summary.id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();
    assert_eq!(prev.config_conflicts.len(), 1);
    assert_eq!(prev.extension_conflicts.len(), 0);
}

#[test]
#[serial(kit_env)]
fn sync_extracts_files_and_writes_sync_record() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let summary = make_basic_kit(&store, &adapters, dir.path(), "Sync Test", "my rules");

    let result = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: summary.id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();
    assert_eq!(result.installed_count, 1);
    assert_eq!(result.skipped_conflict_count, 0);
    assert_eq!(
        fs::read(project.join("CLAUDE.md")).unwrap(),
        b"my rules".to_vec()
    );
    let rec = store
        .lock()
        .get_sync_record(&summary.id, &project.to_string_lossy(), "claude")
        .unwrap()
        .unwrap();
    assert_eq!(rec.written_paths.len(), 1);
    assert!(rec.written_paths[0].ends_with("CLAUDE.md"));
}

#[test]
#[serial(kit_env)]
fn sync_skips_conflict_unless_forced() {
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("CLAUDE.md"), "user content").unwrap();

    let src = dir.path().join("kit-source.md");
    fs::write(&src, "kit content").unwrap();
    let summary = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Conflict Test".into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![KitConfigFileRef {
                agent: "claude".into(),
                category: ConfigCategory::Rules,
                source_path: Some(src.to_string_lossy().into()),
                source_file_name: "CLAUDE.md".into(),
            }],
        },
    )
    .unwrap();

    // Without force flag — skipped
    let r = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: summary.id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();
    assert_eq!(r.installed_count, 0);
    assert_eq!(r.skipped_conflict_count, 1);
    assert_eq!(fs::read_to_string(project.join("CLAUDE.md")).unwrap(), "user content");

    // With force flag — overwritten
    let r = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: summary.id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec!["claude:rules".into()],
        },
    )
    .unwrap();
    assert_eq!(r.installed_count, 1);
    assert_eq!(fs::read_to_string(project.join("CLAUDE.md")).unwrap(), "kit content");
}

#[test]
#[serial(kit_env)]
fn unsync_removes_files_and_record_idempotent_on_missing() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let summary = make_basic_kit(&store, &adapters, dir.path(), "Unsync", "x");
    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: summary.id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();
    assert!(project.join("CLAUDE.md").exists());

    // Simulate user manually deleting the file
    fs::remove_file(project.join("CLAUDE.md")).unwrap();
    // unsync should still succeed and clean up the record.
    unsync_kit_from_project(
        &store,
        &adapters,
        UnsyncKitRequest {
            kit_id: summary.id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();
    assert!(store
        .lock()
        .get_sync_record(&summary.id, &project.to_string_lossy(), "claude")
        .unwrap()
        .is_none());
}

#[test]
#[serial(kit_env)]
fn export_is_byte_identical_copy() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let summary = make_basic_kit(&store, &adapters, dir.path(), "Export", "x");
    let target = dir.path().join("exported.hk-kit.zip");
    export_kit(&store, &summary.id, &target.to_string_lossy()).unwrap();
    let row = store.lock().get_kit_row(&summary.id).unwrap().unwrap();
    let original = fs::read(&row.zip_path).unwrap();
    let copied = fs::read(&target).unwrap();
    assert_eq!(original, copied);
}

#[test]
#[serial(kit_env)]
fn import_generates_new_id_and_dedupes_name() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let original = make_basic_kit(&store, &adapters, dir.path(), "Shared", "x");
    let exported = dir.path().join("shared.hk-kit.zip");
    export_kit(&store, &original.id, &exported.to_string_lossy()).unwrap();

    let imported = import_kit(&store, &exported.to_string_lossy()).unwrap();
    assert_ne!(imported.id, original.id);
    assert_eq!(
        imported.name, "Shared (2)",
        "expected disambiguated name, got {}",
        imported.name
    );
}

#[test]
#[serial(kit_env)]
fn import_rejects_path_traversal_in_manifest() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());

    // Build a hand-crafted bad zip with a single entry "../escape"
    let bad_zip = dir.path().join("bad.hk-kit.zip");
    {
        let f = fs::File::create(&bad_zip).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("manifest.json", opts).unwrap();
        std::io::Write::write_all(&mut w, br#"{
            "kit_format_version": 1, "kit_id": "x", "name": "bad", "description": "",
            "created_at": "2026-01-01T00:00:00Z",
            "exported_from": "x", "extensions": [], "config_files": [], "secrets_stripped": []
        }"#).unwrap();
        w.start_file("../escape", opts).unwrap();
        std::io::Write::write_all(&mut w, b"oops").unwrap();
        w.finish().unwrap();
    }
    let err = import_kit(&store, &bad_zip.to_string_lossy()).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("traversal"),
        "expected traversal-rejection message, got: {msg}"
    );
}

#[test]
#[serial(kit_env)]
fn list_kits_kind_counts_fall_back_to_manifest_for_cross_machine_imports() {
    // Simulates the "Bob received Alice's .hk-kit.zip" scenario: the
    // source_extension_id values inside the manifest reference Alice's
    // DB UUIDs, which don't exist in Bob's extensions table. Without the
    // manifest fallback in list_kits, Bob would see "0 skills · 0 MCP"
    // even though the kit clearly has extensions.
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());

    // Hand-craft a manifest with two extensions (a skill + an MCP) whose
    // source_extension_ids point at UUIDs that aren't in this Store.
    let alice_skill_id = "00000000-0000-4000-8000-000000000001";
    let alice_mcp_id = "00000000-0000-4000-8000-000000000002";
    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: "alice-kit".into(),
        name: "from-alice".into(),
        description: "".into(),
        created_at: Utc::now(),
        exported_from: "HarnessKit test".into(),
        extensions: vec![
            ManifestExtension {
                name: "skill-a".into(),
                kind: ExtensionKind::Skill,
                source_extension_id: alice_skill_id.into(),
                source_url: None,
                content_hash: "sha256:placeholder".into(),
                asset_path: format!("assets/{alice_skill_id}/SKILL.md"),
                position: 0,
                source_revision: None,
                source_branch: None,
            },
            ManifestExtension {
                name: "mcp-a".into(),
                kind: ExtensionKind::Mcp,
                source_extension_id: alice_mcp_id.into(),
                source_url: None,
                content_hash: "sha256:placeholder".into(),
                asset_path: format!("assets/{alice_mcp_id}/mcp.json"),
                position: 1,
                source_revision: None,
                source_branch: None,
            },
        ],
        config_files: vec![],
        secrets_stripped: vec![],
    };
    let zip_path = dir.path().join("from-alice.hk-kit.zip");
    pack_kit(
        &zip_path,
        &manifest,
        &[
            PackEntry {
                zip_path: format!("assets/{alice_skill_id}/SKILL.md"),
                bytes: b"# skill".to_vec(),
            },
            PackEntry {
                zip_path: format!("assets/{alice_mcp_id}/mcp.json"),
                bytes: b"{}".to_vec(),
            },
        ],
    )
    .unwrap();

    crate::kits::service::import_kit(&store, &zip_path.to_string_lossy()).unwrap();

    let kits = list_kits(&store).unwrap();
    let imported = kits.iter().find(|k| k.name == "from-alice").unwrap();
    assert_eq!(imported.extension_count, 2, "extension_count is fed by asset rows");
    assert_eq!(
        imported.kind_counts.skill, 1,
        "skill kind must be recovered from the manifest fallback",
    );
    assert_eq!(
        imported.kind_counts.mcp, 1,
        "mcp kind must be recovered from the manifest fallback",
    );
}

#[test]
#[serial(kit_env)]
fn import_rejects_future_kit_format_version() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());

    // Build a zip with manifest claiming kit_format_version = 99
    let zip_path = dir.path().join("future.hk-kit.zip");
    {
        let f = fs::File::create(&zip_path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("manifest.json", opts).unwrap();
        std::io::Write::write_all(&mut w, br#"{
            "kit_format_version": 99, "kit_id": "x", "name": "future", "description": "",
            "created_at": "2026-01-01T00:00:00Z",
            "exported_from": "x", "extensions": [], "config_files": [], "secrets_stripped": []
        }"#).unwrap();
        w.finish().unwrap();
    }
    let err = crate::kits::service::import_kit(&store, &zip_path.to_string_lossy()).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("not supported") || msg.contains("v99"),
        "expected future-version rejection, got: {msg}"
    );
}

#[test]
#[serial(kit_env)]
fn update_kit_changes_name_preserves_created_at() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let src = dir.path().join("CLAUDE.md");

    let summary = make_basic_kit(&store, &adapters, dir.path(), "Original", "v1");
    let original_created_at = summary.created_at;
    let original_updated_at = summary.updated_at;

    // Sleep so updated_at can change observably.
    std::thread::sleep(std::time::Duration::from_millis(20));

    let updated = update_kit(
        &store,
        &adapters,
        UpdateKitRequest {
            id: summary.id.clone(),
            name: "Renamed".into(),
            description: "second".into(),
            extension_ids: vec![],
            config_files: vec![KitConfigFileRef {
                agent: "claude".into(),
                category: ConfigCategory::Rules,
                source_path: Some(src.to_string_lossy().into()),
                source_file_name: "CLAUDE.md".into(),
            }],
        },
    )
    .unwrap();

    assert_eq!(updated.name, "Renamed");
    assert_eq!(updated.description, "second");
    assert_eq!(updated.created_at, original_created_at, "created_at must be preserved across update_kit");
    assert!(updated.updated_at > original_updated_at, "updated_at must bump on update_kit");
}

// ---------------------------------------------------------------------------
// Helpers for MCP extension kit tests
// ---------------------------------------------------------------------------

/// Build a synthetic Kit zip containing one MCP server entry for `server_name`
/// with the given value blob (the inner value, NOT wrapped in mcpServers).
/// Inserts a KitRow into the store pointing at the zip. Returns the kit id.
fn make_mcp_kit(
    store: &Mutex<Store>,
    zip_dir: &std::path::Path,
    kit_name: &str,
    server_name: &str,
    server_value: serde_json::Value,
) -> String {
    let now = chrono::Utc::now();
    let kit_id = uuid::Uuid::new_v4().to_string();
    let ext_id = uuid::Uuid::new_v4().to_string();
    let asset_path = format!("assets/ext-{ext_id}/mcp.json");

    let entry_bytes = serde_json::to_vec_pretty(&server_value).unwrap();
    let content_hash = sha256_of_bytes(&entry_bytes);

    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: kit_id.clone(),
        name: kit_name.into(),
        description: "".into(),
        created_at: now,
        exported_from: "test".into(),
        extensions: vec![ManifestExtension {
            name: server_name.into(),
            kind: ExtensionKind::Mcp,
            source_extension_id: ext_id.clone(),
            source_url: None,
            content_hash,
            asset_path: asset_path.clone(),
            position: 0,
            source_revision: None,
            source_branch: None,
        }],
        config_files: vec![],
        secrets_stripped: vec![],
    };

    let zip_path = zip_dir.join(format!("{kit_id}.hk-kit.zip"));
    pack_kit(
        &zip_path,
        &manifest,
        &[PackEntry { zip_path: asset_path, bytes: entry_bytes }],
    )
    .unwrap();

    store
        .lock()
        .insert_kit(&KitRow {
            id: kit_id.clone(),
            name: kit_name.into(),
            description: "".into(),
            zip_path: zip_path.to_string_lossy().into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    kit_id
}

// ---------------------------------------------------------------------------
// MCP sync correctness tests
// ---------------------------------------------------------------------------

/// Syncing an MCP Kit into a project with a pre-existing `.mcp.json` must
/// MERGE the new server in, not overwrite the whole file.
#[test]
#[serial(kit_env)]
fn sync_mcp_merges_into_existing_mcp_json() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    // Pre-populate .mcp.json with an existing server.
    let mcp_path = project.join(".mcp.json");
    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"existing":{"command":"old","args":[],"env":{}}}}"#,
    )
    .unwrap();

    let kit_id = make_mcp_kit(
        &store,
        dir.path(),
        "MCP Merge Test",
        "new-server",
        serde_json::json!({"command": "node", "args": ["server.js"], "env": {}}),
    );

    let result = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    assert_eq!(result.installed_count, 1, "should have installed 1 extension");
    assert_eq!(result.skipped_conflict_count, 0);

    let content: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    // Existing entry must still be present.
    assert!(
        content["mcpServers"]["existing"].is_object(),
        "pre-existing 'existing' server must survive merge"
    );
    // New entry must have been added.
    assert_eq!(
        content["mcpServers"]["new-server"]["command"].as_str(),
        Some("node"),
        "new-server must be present after sync"
    );

    // Sync record uses "mcp:<path>:<name>" format.
    let rec = store
        .lock()
        .get_sync_record(&kit_id, &project.to_string_lossy(), "claude")
        .unwrap()
        .unwrap();
    assert_eq!(rec.written_paths.len(), 1);
    assert!(
        rec.written_paths[0].starts_with("mcp:"),
        "written_paths entry must use 'mcp:' prefix, got: {}",
        rec.written_paths[0]
    );
    assert!(
        rec.written_paths[0].ends_with(":new-server"),
        "written_paths entry must end with server name, got: {}",
        rec.written_paths[0]
    );
}

/// Syncing an MCP Kit into an empty project must produce a valid
/// `{"mcpServers":{...}}` file, not a bare entry blob.
#[test]
#[serial(kit_env)]
fn sync_mcp_writes_wrapped_format_on_empty_project() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let kit_id = make_mcp_kit(
        &store,
        dir.path(),
        "MCP Fresh Test",
        "my-tool",
        serde_json::json!({"command": "npx", "args": ["-y", "my-tool"], "env": {}}),
    );

    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    let mcp_path = project.join(".mcp.json");
    assert!(mcp_path.exists(), ".mcp.json must be created");

    let content: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap())
            .expect(".mcp.json must be valid JSON");

    // Must have the mcpServers wrapper — not just the raw entry.
    assert!(
        content.get("mcpServers").is_some(),
        "output must be wrapped in mcpServers, got: {content}"
    );
    assert_eq!(
        content["mcpServers"]["my-tool"]["command"].as_str(),
        Some("npx"),
        "server entry must be present under mcpServers"
    );
}

/// Unsyncing must remove ONLY the named entry from `.mcp.json`, leaving other
/// servers in the file intact.
#[test]
#[serial(kit_env)]
fn unsync_mcp_removes_only_named_entry() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    // Pre-populate .mcp.json with a server that should survive.
    let mcp_path = project.join(".mcp.json");
    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"survivor":{"command":"keep-me","args":[],"env":{}}}}"#,
    )
    .unwrap();

    let kit_id = make_mcp_kit(
        &store,
        dir.path(),
        "MCP Unsync Test",
        "removable",
        serde_json::json!({"command": "node", "args": [], "env": {}}),
    );

    // Sync the kit (adds "removable" alongside "survivor").
    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    // Both entries should be present now.
    let after_sync: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(after_sync["mcpServers"]["survivor"].is_object());
    assert!(after_sync["mcpServers"]["removable"].is_object());

    // Unsync — must remove "removable" but keep "survivor".
    unsync_kit_from_project(
        &store,
        &adapters,
        UnsyncKitRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();

    let after_unsync: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(
        after_unsync["mcpServers"]["survivor"].is_object(),
        "'survivor' must still be present after unsync"
    );
    assert!(
        after_unsync["mcpServers"]["removable"].is_null()
            || after_unsync["mcpServers"].get("removable").is_none(),
        "'removable' must be gone after unsync, got: {}",
        after_unsync
    );

    // Sync record must be deleted.
    assert!(
        store
            .lock()
            .get_sync_record(&kit_id, &project.to_string_lossy(), "claude")
            .unwrap()
            .is_none(),
        "sync record must be deleted after unsync"
    );
}

/// Conflict detection must check the NAMED entry's presence, not just whether
/// the config file exists.
///
/// - When the file exists but with a DIFFERENT server name → no conflict.
/// - When the file exists with the SAME server name → conflict.
#[test]
#[serial(kit_env)]
fn preview_mcp_conflict_only_when_name_exists() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let kit_id = make_mcp_kit(
        &store,
        dir.path(),
        "MCP Conflict Test",
        "target-server",
        serde_json::json!({"command": "node", "args": [], "env": {}}),
    );

    // Scenario A: file exists but with a DIFFERENT server name → no conflict.
    let mcp_path = project.join(".mcp.json");
    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"other-server":{"command":"x","args":[],"env":{}}}}"#,
    )
    .unwrap();

    let prev_a = preview_kit_project_conflicts(
        &store,
        &adapters,
        PreviewKitConflictsRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();
    assert_eq!(
        prev_a.extension_conflicts.len(),
        0,
        "file exists with different server name → no conflict expected, got: {:?}",
        prev_a.extension_conflicts
    );

    // Scenario B: file exists with the SAME server name → conflict.
    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"target-server":{"command":"old","args":[],"env":{}}}}"#,
    )
    .unwrap();

    let prev_b = preview_kit_project_conflicts(
        &store,
        &adapters,
        PreviewKitConflictsRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();
    assert_eq!(
        prev_b.extension_conflicts.len(),
        1,
        "file exists with same server name → conflict expected, got: {:?}",
        prev_b.extension_conflicts
    );
    assert_eq!(prev_b.extension_conflicts[0].asset_name, "target-server");
}

/// Attempting to add a Hook extension to a Kit must fail at pack time with a
/// clear v1-unsupported error message.
#[test]
#[serial(kit_env)]
fn create_kit_rejects_hook_extension() {
    // embed_extension is private so we test the observable behavior at sync
    // time: a Kit zip that contains an ExtensionKind::Hook manifest entry
    // must cause sync_kit_to_project to return the Hook-unsupported error.
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let hook_ext_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let kit_id = uuid::Uuid::new_v4().to_string();
    let hook_zip = dir.path().join(format!("{kit_id}.hk-kit.zip"));

    // Build a synthetic zip with a Hook extension manifest entry.
    {
        let hook_manifest = KitManifest {
            kit_format_version: KIT_FORMAT_VERSION,
            kit_id: kit_id.clone(),
            name: "Hook Test Kit".into(),
            description: "".into(),
            created_at: now,
            exported_from: "test".into(),
            extensions: vec![ManifestExtension {
                name: "PreToolUse:*:echo test".into(),
                kind: ExtensionKind::Hook,
                source_extension_id: hook_ext_id.clone(),
                source_url: None,
                content_hash: "sha256:abc".into(),
                asset_path: "assets/ext-x/hook.json".into(),
                position: 0,
                source_revision: None,
                source_branch: None,
            }],
            config_files: vec![],
            secrets_stripped: vec![],
        };
        pack_kit(
            &hook_zip,
            &hook_manifest,
            &[PackEntry {
                zip_path: "assets/ext-x/hook.json".into(),
                bytes: br#"{"event":"PreToolUse","command":"echo test"}"#.to_vec(),
            }],
        )
        .unwrap();
    }

    store
        .lock()
        .insert_kit(&KitRow {
            id: kit_id.clone(),
            name: "Hook Test Kit".into(),
            description: "".into(),
            zip_path: hook_zip.to_string_lossy().into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let err = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap_err();

    let msg = format!("{err}");
    assert!(
        msg.contains("Hook") && (msg.contains("v1") || msg.contains("not Kit-able") || msg.contains("redesign")),
        "expected Hook v1-unsupported error, got: {msg}"
    );
}

/// Create a fresh `Store` backed by an on-disk SQLite file inside a tempdir.
/// Returns the store and the tempdir guard — callers must hold the guard
/// alive for the duration of the test, otherwise the DB file is deleted.
fn make_store() -> (Store, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let store = Store::open(&dir.path().join("hk.db")).unwrap();
    (store, dir)
}

/// Build a `Skill` extension with the given identity fields. Used by the
/// dedup tests below to populate the store with rows that vary only by
/// `id`, `agents`, and `updated_at`.
fn make_skill_ext(
    id: &str,
    name: &str,
    agent: &str,
    origin: SourceOrigin,
    url: Option<&str>,
    updated_at: DateTime<Utc>,
) -> Extension {
    Extension {
        id: id.into(),
        kind: ExtensionKind::Skill,
        name: name.into(),
        description: String::new(),
        source: Source { origin, url: url.map(String::from), version: None, commit_hash: None },
        agents: vec![agent.into()],
        tags: vec![],
        pack: None,
        permissions: vec![],
        enabled: true,
        trust_score: None,
        installed_at: updated_at,
        updated_at,
        source_path: None,
        cli_parent_id: None,
        cli_meta: None,
        install_meta: None,
        scope: ConfigScope::Global,
    }
}

// Kit editor candidate dedup happens in the frontend via the same
// `buildGroups` used by the Extensions page (editor-asset-tab.tsx), so
// the two surfaces are guaranteed to match. The backend `list_kit_asset_
// candidates` returns raw rows; previous Rust-side dedup unit tests were
// removed when the dedup logic moved.

#[test]
#[serial(kit_env)]
fn create_kit_packs_standalone_md_skill_as_folder_with_skill_md() {
    // Skills can be either a folder with SKILL.md inside, or a bare `<name>.md`
    // file (scanner accepts both shapes). The pack path historically assumed
    // folder-only and exploded on a single .md with "Not a directory (os error
    // 20)". This test pins the new behavior: a standalone .md is repacked as
    // `<prefix>SKILL.md` inside the zip so install-side recreates it as a
    // proper folder skill the receiving agent's scanner can discover.
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);

    // Real .md skill on disk under claude's global skill dir.
    let skill_path = dir
        .path()
        .join(".claude")
        .join("skills")
        .join("paper-to-codebase.md");
    fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
    fs::write(
        &skill_path,
        "---\nname: paper-to-codebase\n---\n# paper-to-codebase",
    )
    .unwrap();

    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    // Match the ID the resolver will compute (global-scope, claude, name).
    let ext_id = crate::scanner::stable_id_for("paper-to-codebase", "skill", "claude");
    let ext = make_skill_ext(
        &ext_id,
        "paper-to-codebase",
        "claude",
        SourceOrigin::Local,
        None,
        Utc::now(),
    );
    {
        let g = store.lock();
        g.insert_extension(&ext).unwrap();
    }

    let summary = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Bare-MD".into(),
            description: "".into(),
            extension_ids: vec![ext_id.clone()],
            config_files: vec![],
        },
    )
    .expect("standalone .md skill must pack without 'Not a directory' failure");

    let zip_path = dir
        .path()
        .join(".harnesskit/kits")
        .join(format!("{}.hk-kit.zip", summary.id));
    let manifest = read_manifest_from_zip(&zip_path).unwrap();
    assert_eq!(manifest.extensions.len(), 1);
    // The zip must contain a SKILL.md entry under the extension's asset
    // prefix (not the original "paper-to-codebase.md" name) so the install
    // side produces `<dest>/paper-to-codebase/SKILL.md`, which agents pick up.
    let f = std::fs::File::open(&zip_path).unwrap();
    let archive = zip::ZipArchive::new(f).unwrap();
    let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    assert!(
        names.iter().any(|n| n.ends_with("/SKILL.md")),
        "zip should contain a SKILL.md entry, got names: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.ends_with("/paper-to-codebase.md")),
        "original .md filename should not survive the rename, got names: {names:?}"
    );
}

#[test]
#[serial(kit_env)]
fn list_kit_asset_candidates_excludes_rows_with_missing_source_on_disk() {
    // Marketplace-installed rows survive `sync_extensions` even when their
    // source disappears (see store.rs:969-989 "keep ... has_install_meta").
    // Without the stale-row guard the row would surface as a candidate and
    // then fail at pack time. This test inserts a skill row whose computed
    // source path doesn't exist anywhere under the tempdir HOME, then asserts
    // it does NOT appear in candidates.
    let dir = tempdir().unwrap();
    let orig_home = std::env::var_os("HOME");
    unsafe { std::env::set_var("HOME", dir.path()); }
    let _restore = RestoreHome(orig_home);
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();

    let stale = make_skill_ext(
        "stale-id",
        "ghost",
        "claude",
        SourceOrigin::Registry,
        Some("hub://ghost"),
        Utc::now(),
    );
    {
        let g = store.lock();
        g.insert_extension(&stale).unwrap();
    }

    let candidates = list_kit_asset_candidates(&store, &adapters).unwrap();
    let ghosts: Vec<_> = candidates
        .extensions
        .iter()
        .filter(|e| e.name == "ghost")
        .collect();
    assert!(
        ghosts.is_empty(),
        "stale row whose source can't be located must not appear as a candidate"
    );
}


/// Insert a minimal KitRow with the given id + name. zip_path is a placeholder
/// path; tests using this helper should not exercise zip IO.
fn insert_kit_row(store: &Mutex<Store>, kit_id: &str, name: &str) {
    let now = Utc::now();
    let g = store.lock();
    g.insert_kit(&KitRow {
        id: kit_id.into(),
        name: name.into(),
        description: String::new(),
        zip_path: format!("/tmp/{kit_id}.zip"),
        created_at: now,
        updated_at: now,
    })
    .unwrap();
}

/// Insert a kit asset for the given kit. Creates the underlying Extension row
/// with the requested `kind` so `list_kits` can resolve kind via the extensions
/// table when computing `kind_counts`. Appends to any existing kit_assets for
/// the kit (read-modify-write through `replace_kit_assets`).
fn insert_kit_asset(
    store: &Mutex<Store>,
    kit_id: &str,
    ext_id: &str,
    kind: ExtensionKind,
) {
    let now = Utc::now();
    let ext = Extension {
        id: ext_id.into(),
        kind,
        name: ext_id.into(),
        description: String::new(),
        source: Source {
            origin: SourceOrigin::Local,
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
        installed_at: now,
        updated_at: now,
        source_path: None,
        cli_parent_id: None,
        cli_meta: None,
        install_meta: None,
        scope: ConfigScope::Global,
    };
    let g = store.lock();
    g.insert_extension(&ext).unwrap();
    let mut existing = g.list_kit_assets(kit_id).unwrap();
    let next_pos = existing.last().map(|r| r.position + 1).unwrap_or(0);
    existing.push(crate::store::KitAssetRow {
        kit_id: kit_id.into(),
        extension_id: ext_id.into(),
        asset_name: ext_id.into(),
        position: next_pos,
    });
    g.replace_kit_assets(kit_id, &existing).unwrap();
}

#[test]
fn list_kits_populates_kind_counts() {
    let (store, _tmp) = make_store();
    let store = Mutex::new(store);
    let kit_id = "k1";
    insert_kit_row(&store, kit_id, "kit1");
    insert_kit_asset(&store, kit_id, "ext-a", ExtensionKind::Skill);
    insert_kit_asset(&store, kit_id, "ext-b", ExtensionKind::Skill);
    insert_kit_asset(&store, kit_id, "ext-c", ExtensionKind::Mcp);
    insert_kit_asset(&store, kit_id, "ext-d", ExtensionKind::Cli);

    let kits = list_kits(&store).unwrap();
    let k = &kits[0];
    assert_eq!(k.kind_counts.skill, 2);
    assert_eq!(k.kind_counts.mcp, 1);
    assert_eq!(k.kind_counts.cli, 1);
    assert_eq!(k.kind_counts.hook, 0);
    assert_eq!(k.kind_counts.plugin, 0);
}

#[test]
#[serial(kit_env)]
fn sync_merges_same_target_config_sources_with_blank_line_separator() {
    // Two CLAUDE.md picks from different source paths share the (claude,
    // Rules, "CLAUDE.md") triple — install_plan derives the same target
    // (`<project>/CLAUDE.md`) for both, so sync should concatenate their
    // contents (in pack order) with a blank-line separator rather than the
    // second source silently overwriting the first.
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let src_a = dir.path().join("a/CLAUDE.md");
    fs::create_dir_all(src_a.parent().unwrap()).unwrap();
    fs::write(&src_a, "section A\n").unwrap();
    let src_b = dir.path().join("b/CLAUDE.md");
    fs::create_dir_all(src_b.parent().unwrap()).unwrap();
    fs::write(&src_b, "section B\n").unwrap();

    let summary = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Merge".into(),
            description: "".into(),
            extension_ids: vec![],
            config_files: vec![
                KitConfigFileRef {
                    agent: "claude".into(),
                    category: ConfigCategory::Rules,
                    source_path: Some(src_a.to_string_lossy().into()),
                    source_file_name: "CLAUDE.md".into(),
                },
                KitConfigFileRef {
                    agent: "claude".into(),
                    category: ConfigCategory::Rules,
                    source_path: Some(src_b.to_string_lossy().into()),
                    source_file_name: "CLAUDE.md".into(),
                },
            ],
        },
    )
    .unwrap();

    let r = sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: summary.id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    // Only ONE installed_count even though the Kit carries two source picks
    // — they collapse into a single target file.
    assert_eq!(r.installed_count, 1);
    let merged = fs::read_to_string(project.join("CLAUDE.md")).unwrap();
    assert_eq!(merged, "section A\n\nsection B\n");
}

#[test]
fn mcp_server_entry_serde_round_trip() {
    use crate::adapter::McpServerEntry;
    let mut env = std::collections::HashMap::new();
    env.insert("API_KEY".to_string(), "x".to_string());
    let original = McpServerEntry {
        name: "my-server".to_string(),
        command: "node".to_string(),
        args: vec!["server.js".to_string()],
        env,
        enabled: true,
    };

    // Canonical Kit blob omits name/enabled — install side supplies name from
    // the plan-item context and assumes enabled.
    let blob = serde_json::to_value(&original).unwrap();
    assert!(!blob.as_object().unwrap().contains_key("name"));
    assert!(!blob.as_object().unwrap().contains_key("enabled"));
    assert_eq!(blob["command"], "node");

    let mut roundtrip: McpServerEntry = serde_json::from_value(blob).unwrap();
    assert_eq!(roundtrip.name, ""); // name is skipped, deserialize default
    assert!(roundtrip.enabled); // default_enabled = true
    roundtrip.name = original.name.clone(); // simulate install-side restore
    assert_eq!(roundtrip.command, original.command);
    assert_eq!(roundtrip.args, original.args);
    assert_eq!(roundtrip.env, original.env);
}

#[test]
fn mcp_server_entry_deserialize_missing_args_and_env() {
    use crate::adapter::McpServerEntry;
    // Args and env have #[serde(default)] so missing fields are tolerated
    // (matches the old mcp_entry_from_value leniency for secret-stripped entries).
    let blob = serde_json::json!({ "command": "uvx" });
    let entry: McpServerEntry = serde_json::from_value(blob).unwrap();
    assert_eq!(entry.command, "uvx");
    assert!(entry.args.is_empty());
    assert!(entry.env.is_empty());
    assert!(entry.enabled);
}

// ---------------------------------------------------------------------------
// install_meta propagation tests
// ---------------------------------------------------------------------------

/// Build a synthetic Kit zip containing one Skill carrying the given source_url
/// in its manifest. The skill payload is a single SKILL.md file under the
/// asset prefix. Inserts a KitRow and returns the kit id.
fn make_skill_kit_with_url(
    store: &Mutex<Store>,
    zip_dir: &std::path::Path,
    kit_name: &str,
    skill_name: &str,
    source_url: &str,
) -> String {
    let now = chrono::Utc::now();
    let kit_id = uuid::Uuid::new_v4().to_string();
    let ext_id = uuid::Uuid::new_v4().to_string();
    let asset_prefix = format!("assets/ext-{ext_id}/");
    let skill_md_path = format!("{asset_prefix}SKILL.md");
    let skill_md_bytes = b"# Test skill\n".to_vec();
    let content_hash = sha256_of_bytes(&skill_md_bytes);

    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: kit_id.clone(),
        name: kit_name.into(),
        description: "".into(),
        created_at: now,
        exported_from: "test".into(),
        extensions: vec![ManifestExtension {
            name: skill_name.into(),
            kind: ExtensionKind::Skill,
            source_extension_id: ext_id.clone(),
            source_url: Some(source_url.into()),
            content_hash,
            asset_path: asset_prefix.clone(),
            position: 0,
            source_revision: None,
            source_branch: None,
        }],
        config_files: vec![],
        secrets_stripped: vec![],
    };

    let zip_path = zip_dir.join(format!("{kit_id}.hk-kit.zip"));
    pack_kit(
        &zip_path,
        &manifest,
        &[PackEntry {
            zip_path: skill_md_path,
            bytes: skill_md_bytes,
        }],
    )
    .unwrap();

    store
        .lock()
        .insert_kit(&KitRow {
            id: kit_id.clone(),
            name: kit_name.into(),
            description: "".into(),
            zip_path: zip_path.to_string_lossy().into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    kit_id
}

/// Without this propagation, the deployed skill row has install_meta=NULL
/// and `extensionGroupKey` falls back to scopeKey-based isolation — the
/// Kit-installed skill shows as a separate row from its origin.
#[test]
#[serial(kit_env)]
fn sync_skill_writes_install_meta_url_from_manifest() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let kit_id = make_skill_kit_with_url(
        &store,
        dir.path(),
        "URL Test Kit",
        "my-skill",
        "https://github.com/example/repo.git",
    );

    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    let project_name = project
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let scope = crate::models::ConfigScope::Project {
        name: project_name,
        path: project.to_string_lossy().into(),
    };
    let ext_id = crate::scanner::stable_id_with_scope_for(
        "my-skill", "skill", "claude", &scope,
    );

    let deployed = store
        .lock()
        .get_extension(&ext_id)
        .unwrap()
        .expect("deployed skill should appear in extensions table");
    let meta = deployed
        .install_meta
        .as_ref()
        .expect("install_meta should be set for Kit-installed skill with manifest URL");
    assert_eq!(meta.install_type, "kit");
    assert_eq!(meta.url.as_deref(), Some("https://github.com/example/repo.git"));
}

/// Unsync should clear install_meta on skills it wrote during sync, otherwise
/// the scanner self-heal at store.rs:972 (which keeps rows with install_meta
/// when files are gone) leaves ghost rows in the extensions table.
#[test]
#[serial(kit_env)]
fn unsync_clears_install_meta_on_kit_installed_skills() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    let kit_id = make_skill_kit_with_url(
        &store,
        dir.path(),
        "Unsync Clear Test",
        "my-skill",
        "https://github.com/example/repo.git",
    );

    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id: kit_id.clone(),
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    let project_name = project
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let scope = crate::models::ConfigScope::Project {
        name: project_name,
        path: project.to_string_lossy().into(),
    };
    let ext_id = crate::scanner::stable_id_with_scope_for(
        "my-skill", "skill", "claude", &scope,
    );
    // Sanity: install_meta is set after sync (covered by another test, but
    // confirm here for sequencing clarity).
    assert!(
        store
            .lock()
            .get_extension(&ext_id)
            .unwrap()
            .and_then(|e| e.install_meta)
            .is_some()
    );

    unsync_kit_from_project(
        &store,
        &adapters,
        UnsyncKitRequest {
            kit_id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();

    let ext_after = store.lock().get_extension(&ext_id).unwrap();
    if let Some(ext) = ext_after {
        assert!(
            ext.install_meta.is_none(),
            "install_meta should be cleared after unsync so self-heal can drop the row"
        );
    }
}

/// Skills whose manifest has source_url = None (e.g. a Kit packed from a
/// locally-authored skill with no origin URL) must NOT trigger install_meta
/// writes — leaving install_meta NULL preserves the conservative "no URL =
/// don't cross-merge" grouping policy.
#[test]
#[serial(kit_env)]
fn sync_skill_leaves_install_meta_null_when_manifest_url_is_none() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = Mutex::new(Store::open(&dir.path().join("hk.db")).unwrap());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();

    // Build a manifest with source_url: None (mirrors a locally-authored skill).
    let now = chrono::Utc::now();
    let kit_id = uuid::Uuid::new_v4().to_string();
    let ext_id_internal = uuid::Uuid::new_v4().to_string();
    let asset_prefix = format!("assets/ext-{ext_id_internal}/");
    let skill_md_path = format!("{asset_prefix}SKILL.md");
    let skill_md_bytes = b"# Local skill\n".to_vec();
    let content_hash = sha256_of_bytes(&skill_md_bytes);
    let manifest = KitManifest {
        kit_format_version: KIT_FORMAT_VERSION,
        kit_id: kit_id.clone(),
        name: "Local Only Kit".into(),
        description: "".into(),
        created_at: now,
        exported_from: "test".into(),
        extensions: vec![ManifestExtension {
            name: "local-skill".into(),
            kind: ExtensionKind::Skill,
            source_extension_id: ext_id_internal.clone(),
            source_url: None,
            content_hash,
            asset_path: asset_prefix.clone(),
            position: 0,
            source_revision: None,
            source_branch: None,
        }],
        config_files: vec![],
        secrets_stripped: vec![],
    };
    let zip_path = dir.path().join(format!("{kit_id}.hk-kit.zip"));
    pack_kit(
        &zip_path,
        &manifest,
        &[PackEntry {
            zip_path: skill_md_path,
            bytes: skill_md_bytes,
        }],
    )
    .unwrap();
    store
        .lock()
        .insert_kit(&KitRow {
            id: kit_id.clone(),
            name: "Local Only Kit".into(),
            description: "".into(),
            zip_path: zip_path.to_string_lossy().into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    sync_kit_to_project(
        &store,
        &adapters,
        SyncKitRequest {
            kit_id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
            force_overwrite_extension_ids: vec![],
            force_overwrite_config_keys: vec![],
        },
    )
    .unwrap();

    let project_name = project
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let scope = crate::models::ConfigScope::Project {
        name: project_name,
        path: project.to_string_lossy().into(),
    };
    let ext_id = crate::scanner::stable_id_with_scope_for(
        "local-skill", "skill", "claude", &scope,
    );
    // When manifest has no source_url, sync skips the post-deploy scan-and-
    // insert path entirely, so the row may not be in the table yet — the
    // scanner picks it up on the next list_extensions. Both states (row
    // absent, or row present with NULL meta) satisfy the no-write guarantee.
    let ext_after = store.lock().get_extension(&ext_id).unwrap();
    assert!(
        ext_after.as_ref().map_or(true, |e| e.install_meta.is_none()),
        "install_meta should remain NULL when manifest carries no source_url"
    );
}

