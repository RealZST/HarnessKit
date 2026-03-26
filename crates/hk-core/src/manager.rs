use anyhow::Result;
use crate::store::Store;

pub struct Manager {
    pub store: Store,
}

impl Manager {
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    pub fn toggle(&self, id: &str, enabled: bool) -> Result<()> {
        self.store.set_enabled(id, enabled)
    }

    pub fn uninstall(&self, id: &str) -> Result<()> {
        self.store.delete_extension(id)
    }

    pub fn update_tags(&self, _id: &str, _tags: Vec<String>) -> Result<()> {
        // v1: tags stored in extension_tags_json, update via store
        // Implementation: read extension, modify tags, write back
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use tempfile::TempDir;

    #[test]
    fn test_toggle_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();
        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "test".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.toggle(&ext.id, false).unwrap();
        let fetched = manager.store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_uninstall_extension() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = crate::store::Store::open(&db_path).unwrap();
        let ext = Extension {
            id: uuid::Uuid::new_v4().to_string(),
            kind: ExtensionKind::Skill,
            name: "to-delete".into(),
            description: "".into(),
            source: Source { origin: SourceOrigin::Local, url: None, version: None, commit_hash: None },
            agents: vec!["claude".into()],
            tags: vec![],
            permissions: vec![],
            enabled: true,
            trust_score: None,
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        store.insert_extension(&ext).unwrap();

        let manager = Manager::new(store);
        manager.uninstall(&ext.id).unwrap();
        assert!(manager.store.get_extension(&ext.id).unwrap().is_none());
    }
}
