//! Shared test helpers for the store submodules.

use super::*;
pub use tempfile::TempDir;

    pub fn test_store() -> (Store, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        (store, dir)
    }
    pub fn sample_extension() -> Extension {
        Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "test-skill".into(),
            description: "A test skill".into(),
            source: Source {
                origin: SourceOrigin::Local,
                url: None,
                version: None,
                commit_hash: None,
                from_manifest: false,
            },
            agents: vec!["claude".into()],
            tags: vec!["test".into()],
            pack: None,
            permissions: vec![Permission::FileSystem {
                paths: vec!["/tmp".into()],
            }],
            enabled: true,
            trust_score: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
            scope: ConfigScope::Global,
            mcp_transport: None,
        }
    }
