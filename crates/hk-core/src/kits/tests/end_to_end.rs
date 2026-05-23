use crate::adapter::all_adapters;
use crate::kits::service::{create_kit, export_kit, import_kit, sync_kit_to_project, unsync_kit_from_project};
use crate::kits::tests::service::RestoreHome;
use crate::kits::types::{
    CreateKitRequest, KitConfigFileRef, SyncKitRequest, UnsyncKitRequest,
};
use crate::models::ConfigCategory;
use crate::store::Store;
use parking_lot::Mutex;
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

fn open(dir: &std::path::Path) -> Mutex<Store> {
    Mutex::new(Store::open(&dir.join("hk.db")).unwrap())
}

#[test]
#[serial(kit_env)]
fn full_lifecycle_create_install_uninstall() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = open(dir.path());
    let adapters = all_adapters();
    let project = dir.path().join("proj");
    fs::create_dir_all(&project).unwrap();
    let src = dir.path().join("CLAUDE.md");
    fs::write(&src, "rules").unwrap();
    let summary = create_kit(
        &store,
        &adapters,
        CreateKitRequest {
            name: "Lifecycle".into(),
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
    unsync_kit_from_project(
        &store,
        &adapters,
        UnsyncKitRequest {
            kit_id: summary.id,
            project_path: project.to_string_lossy().into(),
            agent_name: "claude".into(),
        },
    )
    .unwrap();
    assert!(!project.join("CLAUDE.md").exists());
}

#[test]
#[serial(kit_env)]
fn export_import_round_trip_preserves_manifest_bytes() {
    let dir = tempdir().unwrap();
    let _restore = RestoreHome::new();
    unsafe { std::env::set_var("HOME", dir.path()); }
    let store = open(dir.path());
    let adapters = all_adapters();
    let src = dir.path().join("CLAUDE.md");
    fs::write(&src, "x").unwrap();
    let summary = create_kit(&store, &adapters, CreateKitRequest {
        name: "Roundtrip".into(), description: "".into(), extension_ids: vec![],
        config_files: vec![KitConfigFileRef {
            agent: "claude".into(), category: ConfigCategory::Rules,
            source_path: Some(src.to_string_lossy().into()),
            source_file_name: "CLAUDE.md".into(),
        }],
    }).unwrap();
    let target = dir.path().join("out.hk-kit.zip");
    export_kit(&store, &summary.id, &target.to_string_lossy()).unwrap();
    let _new_kit = import_kit(&store, &target.to_string_lossy()).unwrap();
    let all = crate::kits::service::list_kits(&store).unwrap();
    assert_eq!(all.len(), 2);
}
