//! Project registry.

use super::*;

impl Store {
    pub fn insert_project(&self, project: &Project) -> Result<(), HkError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO projects (id, name, path, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                project.id,
                project.name,
                project.path,
                project.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }
    /// Register a project by path if it isn't already registered. Used by
    /// the Kit sync flow to handle the "install into a new folder" case.
    /// Best-effort: errors are swallowed so an install never fails on
    /// project-registry bookkeeping.
    pub fn register_project_by_path(&self, project_path: &str) {
        let projects = self.list_project_tuples();
        if projects.iter().any(|(_, p)| p == project_path) {
            return;
        }
        let name = std::path::Path::new(project_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(project_path)
            .to_string();
        let _ = self.insert_project(&Project {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            path: project_path.to_string(),
            created_at: Utc::now(),
            exists: true,
        });
    }
    pub fn delete_project(&self, id: &str) -> Result<(), HkError> {
        // Look up the project's path before deletion so we can cascade-delete
        // any extensions scoped to it. Without this, scope_json continues to
        // reference a project that no longer exists in the projects table,
        // and those rows show up as ghosts in the "All scopes" view with no
        // project to filter into.
        let path: Option<String> = self
            .conn
            .query_row(
                "SELECT path FROM projects WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        // unchecked_transaction: safe because Store is behind a Mutex
        // (single-writer guaranteed at the call sites).
        let tx = self.conn.unchecked_transaction()?;
        if let Some(path) = path {
            tx.execute(
                "DELETE FROM extensions \
                 WHERE json_extract(scope_json, '$.type') = 'project' \
                   AND json_extract(scope_json, '$.path') = ?1",
                params![path],
            )?;
        }
        tx.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }
    /// Convenience: list all projects flattened to `(name, path)` tuples — the
    /// shape that scanner / `find_skill_by_id` expect. Swallows errors and
    /// returns an empty list, matching how nearly every caller already wraps
    /// the call.
    pub fn list_project_tuples(&self) -> Vec<(String, String)> {
        self.list_projects()
            .unwrap_or_default()
            .into_iter()
            .map(|p| (p.name, p.path))
            .collect()
    }
    pub fn list_projects(&self) -> Result<Vec<Project>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, path, created_at FROM projects ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(3)?;
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
                exists: true, // Will be updated by the command layer
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
    #[test]
    fn test_insert_and_list_projects() {
        let (store, _dir) = test_store();
        let project = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
            exists: true,
        };
        store.insert_project(&project).unwrap();
        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "my-project");
        assert_eq!(projects[0].path, "/tmp/my-project");
    }
    #[test]
    fn test_insert_project_ignores_duplicate_path() {
        let (store, _dir) = test_store();
        let project1 = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
            exists: true,
        };
        let project2 = Project {
            id: "proj-002".into(),
            name: "my-project-dup".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
            exists: true,
        };
        store.insert_project(&project1).unwrap();
        store.insert_project(&project2).unwrap();
        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "proj-001");
    }
    #[test]
    fn test_delete_project() {
        let (store, _dir) = test_store();
        let project = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
            exists: true,
        };
        store.insert_project(&project).unwrap();
        store.delete_project("proj-001").unwrap();
        let projects = store.list_projects().unwrap();
        assert!(projects.is_empty());
    }
    #[test]
    fn test_delete_project_cascades_to_extensions() {
        let (store, _dir) = test_store();
        let project = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
            exists: true,
        };
        store.insert_project(&project).unwrap();

        // One extension in the project, one global, one in a different project.
        // Only the first should disappear when proj-001 is deleted.
        let mut in_project = sample_extension();
        in_project.id = "ext-in-project".into();
        in_project.scope = ConfigScope::Project {
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
        };
        store.insert_extension(&in_project).unwrap();

        let mut global = sample_extension();
        global.id = "ext-global".into();
        store.insert_extension(&global).unwrap();

        let other = Project {
            id: "proj-002".into(),
            name: "other".into(),
            path: "/tmp/other".into(),
            created_at: Utc::now(),
            exists: true,
        };
        store.insert_project(&other).unwrap();
        let mut in_other = sample_extension();
        in_other.id = "ext-in-other".into();
        in_other.scope = ConfigScope::Project {
            name: "other".into(),
            path: "/tmp/other".into(),
        };
        store.insert_extension(&in_other).unwrap();

        store.delete_project("proj-001").unwrap();

        let remaining: Vec<String> = store
            .list_extensions(None, None)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert!(!remaining.contains(&"ext-in-project".to_string()));
        assert!(remaining.contains(&"ext-global".to_string()));
        assert!(remaining.contains(&"ext-in-other".to_string()));
    }
}
