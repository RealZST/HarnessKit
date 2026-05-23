use crate::models::ConfigCategory;
use crate::store::{KitAssetRow, KitConfigFileRow, KitRow, Store, SyncRecordRow};
use chrono::Utc;
use tempfile::tempdir;

fn open_store() -> (Store, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("hk.db");
    let store = Store::open(&db_path).unwrap();
    (store, dir)
}

#[test]
fn insert_then_list_kit_summary_returns_one() {
    let (store, _dir) = open_store();
    let now = Utc::now();
    store
        .insert_kit(&KitRow {
            id: "k1".into(),
            name: "Frontend".into(),
            description: "".into(),
            zip_path: "/tmp/k1.hk-kit.zip".into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    let rows = store.list_kit_rows().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Frontend");
}

#[test]
fn cascade_delete_kit_removes_assets_preserves_sync_records() {
    let (store, _dir) = open_store();
    let now = Utc::now();
    store
        .insert_kit(&KitRow {
            id: "k1".into(),
            name: "K1".into(),
            description: "".into(),
            zip_path: "/tmp/k1.hk-kit.zip".into(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    store
        .replace_kit_assets(
            "k1",
            &[KitAssetRow {
                kit_id: "k1".into(),
                extension_id: "e1".into(),
                asset_name: "Skill".into(),
                position: 0,
            }],
        )
        .unwrap();
    store
        .replace_kit_config_files(
            "k1",
            &[KitConfigFileRow {
                kit_id: "k1".into(),
                agent: "claude".into(),
                category: ConfigCategory::Rules,
                source_path: "/CLAUDE.md".into(),
                source_file_name: "CLAUDE.md".into(),
                position: 0,
            }],
        )
        .unwrap();
    store
        .upsert_sync_record(&SyncRecordRow {
            id: "s1".into(),
            kit_id: "k1".into(),
            project_path: "/p".into(),
            agent_name: "claude".into(),
            written_paths: vec!["/p/CLAUDE.md".into()],
            synced_at: now,
        })
        .unwrap();

    store.delete_kit("k1").unwrap();

    assert_eq!(store.list_kit_assets("k1").unwrap().len(), 0);
    assert_eq!(store.list_kit_config_files("k1").unwrap().len(), 0);
    // Sync records preserved (soft reference):
    let rec = store
        .get_sync_record("k1", "/p", "claude")
        .unwrap()
        .expect("sync record should still exist");
    assert_eq!(rec.written_paths, vec!["/p/CLAUDE.md".to_string()]);
}

#[test]
fn upsert_sync_record_replaces_previous_for_same_triple() {
    let (store, _dir) = open_store();
    let now = Utc::now();
    store
        .upsert_sync_record(&SyncRecordRow {
            id: "s1".into(),
            kit_id: "k1".into(),
            project_path: "/p".into(),
            agent_name: "claude".into(),
            written_paths: vec!["/p/a".into()],
            synced_at: now,
        })
        .unwrap();
    store
        .upsert_sync_record(&SyncRecordRow {
            id: "s2".into(),
            kit_id: "k1".into(),
            project_path: "/p".into(),
            agent_name: "claude".into(),
            written_paths: vec!["/p/b".into(), "/p/c".into()],
            synced_at: now,
        })
        .unwrap();
    let r = store.get_sync_record("k1", "/p", "claude").unwrap().unwrap();
    assert_eq!(r.written_paths, vec!["/p/b".to_string(), "/p/c".to_string()]);
}

