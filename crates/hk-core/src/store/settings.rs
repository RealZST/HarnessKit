//! Agent settings (custom paths, enabled, order) and user-defined custom config paths.

use super::*;

impl Store {
    pub fn get_agent_setting(&self, name: &str) -> Result<(Option<String>, bool), HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT custom_path, enabled FROM agent_settings WHERE name = ?1")?;
        let result = stmt.query_row(params![name], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, bool>(1)?))
        });
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((None, true)),
            Err(e) => Err(e.into()),
        }
    }
    pub fn set_agent_path(&self, name: &str, path: Option<&str>) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT INTO agent_settings (name, custom_path, enabled)
             VALUES (?1, ?2, 1)
             ON CONFLICT(name) DO UPDATE SET custom_path = excluded.custom_path",
            params![name, path],
        )?;
        Ok(())
    }
    pub fn set_agent_enabled(&self, name: &str, enabled: bool) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT INTO agent_settings (name, custom_path, enabled)
             VALUES (?1, NULL, ?2)
             ON CONFLICT(name) DO UPDATE SET enabled = excluded.enabled",
            params![name, enabled],
        )?;
        Ok(())
    }
    /// Returns agent names in user-defined order. Agents without a sort_order
    /// are appended at the end in their default order.
    pub fn get_agent_order(&self) -> Result<Vec<(String, i32)>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT name, sort_order FROM agent_settings WHERE sort_order IS NOT NULL ORDER BY sort_order"
        )?;
        let rows: Vec<(String, i32)> = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect();
        Ok(rows)
    }
    /// Persist a custom agent order. `names` is the full ordered list of agent names.
    pub fn set_agent_order(&self, names: &[String]) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;
        for (i, name) in names.iter().enumerate() {
            tx.execute(
                "INSERT INTO agent_settings (name, custom_path, enabled, sort_order)
                 VALUES (?1, NULL, 1, ?2)
                 ON CONFLICT(name) DO UPDATE SET sort_order = excluded.sort_order",
                params![name, i as i32],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn add_custom_config_path(
        &self,
        agent: &str,
        path: &str,
        label: &str,
        category: &str,
        scope_json: Option<&str>,
    ) -> Result<i64, HkError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO custom_config_paths (agent, path, label, category, scope_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![agent, path, label, category, scope_json],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM custom_config_paths WHERE agent = ?1 AND path = ?2",
            params![agent, path],
            |row| row.get(0),
        )?;
        Ok(id)
    }
    pub fn update_custom_config_path(
        &self,
        id: i64,
        path: &str,
        label: &str,
        category: &str,
    ) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE custom_config_paths SET path = ?2, label = ?3, category = ?4 WHERE id = ?1",
            params![id, path, label, category],
        )?;
        Ok(())
    }
    pub fn remove_custom_config_path(&self, id: i64) -> Result<(), HkError> {
        self.conn
            .execute("DELETE FROM custom_config_paths WHERE id = ?1", params![id])?;
        Ok(())
    }
    pub fn list_custom_config_paths(
        &self,
        agent: &str,
    ) -> Result<Vec<CustomConfigPathRow>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, label, category, scope_json FROM custom_config_paths WHERE agent = ?1 ORDER BY label"
        )?;
        let rows = stmt
            .query_map(params![agent], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect();
        Ok(rows)
    }
    pub fn list_all_custom_config_paths(&self) -> Result<Vec<String>, HkError> {
        let mut stmt = self.conn.prepare("SELECT path FROM custom_config_paths")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
    #[test]
    fn test_agent_order_roundtrip() {
        let (store, _dir) = test_store();
        // Initially empty
        assert!(store.get_agent_order().unwrap().is_empty());

        let order = vec!["cursor".into(), "claude".into(), "codex".into()];
        store.set_agent_order(&order).unwrap();

        let saved = store.get_agent_order().unwrap();
        assert_eq!(saved.len(), 3);
        assert_eq!(saved[0], ("cursor".into(), 0));
        assert_eq!(saved[1], ("claude".into(), 1));
        assert_eq!(saved[2], ("codex".into(), 2));

        // Update order
        let new_order = vec!["codex".into(), "cursor".into(), "claude".into()];
        store.set_agent_order(&new_order).unwrap();
        let saved = store.get_agent_order().unwrap();
        assert_eq!(saved[0].0, "codex");
        assert_eq!(saved[1].0, "cursor");
        assert_eq!(saved[2].0, "claude");
    }
    #[test]
    fn test_add_custom_config_path_returns_correct_id_on_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();
        let id1 = store
            .add_custom_config_path("claude", "/some/path", "label", "settings", None)
            .unwrap();
        // Insert a different path to change last_insert_rowid
        let _id_other = store
            .add_custom_config_path("claude", "/other/path", "label", "settings", None)
            .unwrap();
        // Now try to insert the first path again - this should return id1, not id_other
        let id2 = store
            .add_custom_config_path("claude", "/some/path", "label", "settings", None)
            .unwrap();
        assert_eq!(id1, id2, "Duplicate insert should return the same ID");
        assert!(id1 > 0, "ID should be positive");
    }
    #[test]
    fn test_custom_config_path_persists_scope_round_trip() {
        let (store, _dir) = test_store();
        let scope = ConfigScope::Project {
            name: "demo".into(),
            path: "/p/demo".into(),
        };
        let scope_json = serde_json::to_string(&scope).unwrap();
        store
            .add_custom_config_path(
                "claude",
                "/p/demo/foo",
                "foo",
                "settings",
                Some(&scope_json),
            )
            .unwrap();
        // NULL scope row coexists (legacy / Global default)
        store
            .add_custom_config_path("claude", "/u/global/bar", "bar", "settings", None)
            .unwrap();

        let rows = store.list_custom_config_paths("claude").unwrap();
        let scoped = rows.iter().find(|r| r.1 == "/p/demo/foo").unwrap();
        assert_eq!(scoped.4, Some(scope_json), "project scope persisted");
        let global = rows.iter().find(|r| r.1 == "/u/global/bar").unwrap();
        assert_eq!(global.4, None, "NULL scope (legacy/Global) preserved");
    }
    #[test]
    fn test_list_all_custom_config_paths_includes_all_agents() {
        let (store, _dir) = test_store();
        store
            .add_custom_config_path("claude", "/tmp/a", "a", "settings", None)
            .unwrap();
        store
            .add_custom_config_path("codex", "/tmp/b", "b", "rules", None)
            .unwrap();

        let mut paths = store.list_all_custom_config_paths().unwrap();
        paths.sort();

        assert_eq!(paths, vec!["/tmp/a".to_string(), "/tmp/b".to_string()]);
    }
}
