use crate::store::Store;
use tempfile::tempdir;

#[test]
fn migrate_creates_kit_tables() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("hk.db");
    let store = Store::open(&db_path).unwrap();
    assert_eq!(store.schema_version().unwrap(), 8);

    let conn = store.conn_for_test();
    let names: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(Result::ok)
        .collect();

    for required in &[
        "kits",
        "kit_assets",
        "kit_config_files",
        "kit_sync_records",
    ] {
        assert!(
            names.iter().any(|n| n == required),
            "missing table {required}, got {names:?}"
        );
    }

    // project_stacks must NOT exist (dropped in migrate_v8).
    assert!(
        !names.iter().any(|n| n == "project_stacks"),
        "project_stacks should have been dropped by migrate_v8, got {names:?}"
    );
}

#[test]
fn migrate_is_idempotent() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("hk.db");
    let _store = Store::open(&db_path).unwrap();
    // Re-open to re-run migration logic
    let store = Store::open(&db_path).unwrap();
    assert_eq!(store.schema_version().unwrap(), 8);
}

