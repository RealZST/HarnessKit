use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;

use crate::models::*;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS extensions (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                source_json TEXT NOT NULL DEFAULT '{}',
                agents_json TEXT NOT NULL DEFAULT '[]',
                tags_json TEXT NOT NULL DEFAULT '[]',
                permissions_json TEXT NOT NULL DEFAULT '[]',
                enabled INTEGER NOT NULL DEFAULT 1,
                trust_score INTEGER,
                installed_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                extension_id TEXT NOT NULL REFERENCES extensions(id) ON DELETE CASCADE,
                findings_json TEXT NOT NULL DEFAULT '[]',
                trust_score INTEGER NOT NULL,
                audited_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_extensions_kind ON extensions(kind);
            CREATE INDEX IF NOT EXISTS idx_audit_results_ext ON audit_results(extension_id);
            "
        )?;
        // Migration: add category column for existing databases
        let _ = self.conn.execute("ALTER TABLE extensions ADD COLUMN category TEXT", []);
        // Migration: add last_used_at column for skill usage tracking
        let _ = self.conn.execute("ALTER TABLE extensions ADD COLUMN last_used_at TEXT", []);
        // Migration: add disabled_config column for real enable/disable
        let _ = self.conn.execute("ALTER TABLE extensions ADD COLUMN disabled_config TEXT", []);
        // Migration: hidden_extensions table for surviving re-scans
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hidden_extensions (id TEXT PRIMARY KEY)"
        )?;
        // Migration: agent_settings table for custom paths and enabled state
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_settings (
                name TEXT PRIMARY KEY,
                custom_path TEXT,
                enabled INTEGER NOT NULL DEFAULT 1
            )"
        )?;
        Ok(())
    }

    // --- Agent settings ---

    pub fn get_agent_setting(&self, name: &str) -> Result<(Option<String>, bool)> {
        let mut stmt = self.conn.prepare(
            "SELECT custom_path, enabled FROM agent_settings WHERE name = ?1"
        )?;
        let result = stmt.query_row(params![name], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, bool>(1)?))
        });
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok((None, true)),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_agent_path(&self, name: &str, path: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO agent_settings (name, custom_path, enabled)
             VALUES (?1, ?2, 1)
             ON CONFLICT(name) DO UPDATE SET custom_path = excluded.custom_path",
            params![name, path],
        )?;
        Ok(())
    }

    pub fn set_agent_enabled(&self, name: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "INSERT INTO agent_settings (name, custom_path, enabled)
             VALUES (?1, NULL, ?2)
             ON CONFLICT(name) DO UPDATE SET enabled = excluded.enabled",
            params![name, enabled],
        )?;
        Ok(())
    }


    /// Upsert an extension: insert if new, update scanner-derived fields if existing.
    /// Preserves user-set fields: enabled, tags, category, trust_score.
    pub fn insert_extension(&self, ext: &Extension) -> Result<()> {
        self.conn.execute(
            "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
               kind = excluded.kind,
               name = excluded.name,
               description = excluded.description,
               source_json = excluded.source_json,
               agents_json = excluded.agents_json,
               permissions_json = excluded.permissions_json,
               updated_at = excluded.updated_at,
               category = extensions.category,
               last_used_at = excluded.last_used_at",
            params![
                ext.id,
                ext.kind.as_str(),
                ext.name,
                ext.description,
                serde_json::to_string(&ext.source)?,
                serde_json::to_string(&ext.agents)?,
                serde_json::to_string(&ext.tags)?,
                serde_json::to_string(&ext.permissions)?,
                ext.enabled as i32,
                ext.trust_score.map(|s| s as i32),
                ext.installed_at.to_rfc3339(),
                ext.updated_at.to_rfc3339(),
                ext.category,
                ext.last_used_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn get_extension(&self, id: &str) -> Result<Option<Extension>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, last_used_at
             FROM extensions WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(params![id], |row| Ok(self.row_to_extension(row)))?;
        match rows.next() {
            Some(Ok(Ok(ext))) => Ok(Some(ext)),
            Some(Ok(Err(e))) => Err(e),
            Some(Err(e)) => Err(e.into()),
            None => Ok(None),
        }
    }

    pub fn list_extensions(&self, kind: Option<ExtensionKind>, agent: Option<&str>) -> Result<Vec<Extension>> {
        let mut sql = "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, last_used_at FROM extensions WHERE 1=1".to_string();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(k) = kind {
            sql.push_str(&format!(" AND kind = ?{}", param_values.len() + 1));
            param_values.push(Box::new(k.as_str().to_string()));
        }

        if agent.is_some() {
            sql.push_str(&format!(" AND agents_json LIKE ?{}", param_values.len() + 1));
            param_values.push(Box::new(format!("%\"{}%", agent.unwrap())));
        }

        sql.push_str(" ORDER BY installed_at DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> = param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(self.row_to_extension(row)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row??);
        }
        Ok(results)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET enabled = ?1 WHERE id = ?2",
            params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn get_disabled_config(&self, id: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT disabled_config FROM extensions WHERE id = ?1"
        )?;
        let result = stmt.query_row(params![id], |row| row.get::<_, Option<String>>(0));
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_disabled_config(&self, id: &str, config: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET disabled_config = ?1 WHERE id = ?2",
            params![config, id],
        )?;
        Ok(())
    }

    pub fn update_trust_score(&self, id: &str, score: u8) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET trust_score = ?1 WHERE id = ?2",
            params![score as i32, id],
        )?;
        Ok(())
    }

    pub fn update_tags(&self, id: &str, tags: &[String]) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET tags_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(tags)?, id],
        )?;
        Ok(())
    }

    pub fn get_all_tags(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT DISTINCT tags_json FROM extensions WHERE tags_json != '[]'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut all_tags = std::collections::BTreeSet::new();
        for row in rows {
            let json: String = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&json) {
                for tag in tags { all_tags.insert(tag); }
            }
        }
        Ok(all_tags.into_iter().collect())
    }

    pub fn update_category(&self, id: &str, category: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET category = ?1 WHERE id = ?2",
            params![category, id],
        )?;
        Ok(())
    }

    pub fn delete_extension(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Sync all scanned extensions in a single transaction.
    /// Upserts every extension and removes stale entries that no longer exist on disk.
    /// Much faster than individual insert_extension calls (one fsync instead of N).
    pub fn sync_extensions(&self, extensions: &[Extension]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for ext in extensions {
            tx.execute(
                "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, last_used_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET
                   kind = excluded.kind,
                   name = excluded.name,
                   description = excluded.description,
                   source_json = excluded.source_json,
                   agents_json = excluded.agents_json,
                   permissions_json = excluded.permissions_json,
                   updated_at = excluded.updated_at,
                   category = extensions.category,
                   last_used_at = excluded.last_used_at",
                params![
                    ext.id,
                    ext.kind.as_str(),
                    ext.name,
                    ext.description,
                    serde_json::to_string(&ext.source)?,
                    serde_json::to_string(&ext.agents)?,
                    serde_json::to_string(&ext.tags)?,
                    serde_json::to_string(&ext.permissions)?,
                    ext.enabled as i32,
                    ext.trust_score.map(|s| s as i32),
                    ext.installed_at.to_rfc3339(),
                    ext.updated_at.to_rfc3339(),
                    ext.category,
                    ext.last_used_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
        }

        // Remove stale extensions no longer on disk
        let scanned_ids: std::collections::HashSet<&str> =
            extensions.iter().map(|e| e.id.as_str()).collect();
        let stale_ids: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM extensions")?;
            stmt.query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect()
        };
        for id in &stale_ids {
            if !scanned_ids.contains(id.as_str()) {
                tx.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn insert_audit_result(&self, result: &AuditResult) -> Result<()> {
        self.conn.execute(
            "INSERT INTO audit_results (extension_id, findings_json, trust_score, audited_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                result.extension_id,
                serde_json::to_string(&result.findings)?,
                result.trust_score as i32,
                result.audited_at.to_rfc3339(),
            ],
        )?;
        self.update_trust_score(&result.extension_id, result.trust_score)?;
        Ok(())
    }

    pub fn get_audit_results(&self, extension_id: &str) -> Result<Vec<AuditResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT extension_id, findings_json, trust_score, audited_at
             FROM audit_results WHERE extension_id = ?1 ORDER BY audited_at DESC"
        )?;
        let rows = stmt.query_map(params![extension_id], |row| {
            let findings_json: String = row.get(1)?;
            let audited_at_str: String = row.get(3)?;
            Ok(AuditResult {
                extension_id: row.get(0)?,
                findings: serde_json::from_str(&findings_json).unwrap_or_default(),
                trust_score: row.get::<_, i32>(2)? as u8,
                audited_at: DateTime::parse_from_rfc3339(&audited_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get the latest audit result for every non-hidden extension (one per extension_id).
    pub fn list_latest_audit_results(&self) -> Result<Vec<AuditResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT a.extension_id, a.findings_json, a.trust_score, a.audited_at
             FROM audit_results a
             INNER JOIN (
                 SELECT extension_id, MAX(audited_at) AS max_at
                 FROM audit_results GROUP BY extension_id
             ) latest ON a.extension_id = latest.extension_id AND a.audited_at = latest.max_at
             INNER JOIN extensions e ON a.extension_id = e.id"
        )?;
        let rows = stmt.query_map([], |row| {
            let findings_json: String = row.get(1)?;
            let audited_at_str: String = row.get(3)?;
            Ok(AuditResult {
                extension_id: row.get(0)?,
                findings: serde_json::from_str(&findings_json).unwrap_or_default(),
                trust_score: row.get::<_, i32>(2)? as u8,
                audited_at: DateTime::parse_from_rfc3339(&audited_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // --- Project methods ---

    pub fn insert_project(&self, project: &Project) -> Result<()> {
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

    pub fn delete_project(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, path, created_at FROM projects ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            let created_at_str: String = row.get(3)?;
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn row_to_extension(&self, row: &rusqlite::Row) -> Result<Extension> {
        let kind_str: String = row.get(1)?;
        let source_json: String = row.get(4)?;
        let agents_json: String = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let permissions_json: String = row.get(7)?;
        let installed_at_str: String = row.get(10)?;
        let updated_at_str: String = row.get(11)?;
        let last_used_at_str: Option<String> = row.get::<_, Option<String>>(13).ok().flatten();

        Ok(Extension {
            id: row.get(0)?,
            kind: kind_str.parse()?,
            name: row.get(2)?,
            description: row.get(3)?,
            source: serde_json::from_str(&source_json)?,
            agents: serde_json::from_str(&agents_json)?,
            tags: serde_json::from_str(&tags_json)?,
            category: row.get::<_, Option<String>>(12).ok().flatten(),
            permissions: serde_json::from_str(&permissions_json)?,
            enabled: row.get::<_, i32>(8)? != 0,
            trust_score: row.get::<_, Option<i32>>(9)?.map(|s| s as u8),
            installed_at: DateTime::parse_from_rfc3339(&installed_at_str)?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?
                .with_timezone(&Utc),
            last_used_at: last_used_at_str.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_store() -> (Store, TempDir) {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();
        (store, dir)
    }

    fn sample_extension() -> Extension {
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
            },
            agents: vec!["claude".into()],
            tags: vec!["test".into()],
            category: None,
            permissions: vec![Permission::FileSystem {
                paths: vec!["/tmp".into()],
            }],
            enabled: true,
            trust_score: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
            last_used_at: None,
        }
    }

    #[test]
    fn test_open_and_migrate() {
        let (store, _dir) = test_store();
        let exts = store.list_extensions(None, None).unwrap();
        assert!(exts.is_empty());
    }

    #[test]
    fn test_insert_and_get_extension() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.name, "test-skill");
        assert_eq!(fetched.kind, ExtensionKind::Skill);
        assert_eq!(fetched.agents, vec!["claude"]);
        assert_eq!(fetched.tags, vec!["test"]);
    }

    #[test]
    fn test_list_extensions_filter_by_kind() {
        let (store, _dir) = test_store();
        let mut skill = sample_extension();
        skill.name = "my-skill".into();
        store.insert_extension(&skill).unwrap();

        let mut mcp = sample_extension();
        mcp.id = uuid::Uuid::new_v4().to_string();
        mcp.kind = ExtensionKind::Mcp;
        mcp.name = "my-mcp".into();
        store.insert_extension(&mcp).unwrap();

        let skills = store.list_extensions(Some(ExtensionKind::Skill), None).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
    }

    #[test]
    fn test_list_extensions_filter_by_agent() {
        let (store, _dir) = test_store();
        let mut ext1 = sample_extension();
        ext1.agents = vec!["claude".into()];
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = uuid::Uuid::new_v4().to_string();
        ext2.name = "cursor-skill".into();
        ext2.agents = vec!["cursor".into()];
        store.insert_extension(&ext2).unwrap();

        let claude_exts = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude_exts.len(), 1);
        assert_eq!(claude_exts[0].name, "test-skill");
    }

    #[test]
    fn test_update_extension_toggle() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        store.set_enabled(&ext.id, false).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert!(!fetched.enabled);
    }

    #[test]
    fn test_delete_extension() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store.delete_extension(&ext.id).unwrap();
        assert!(store.get_extension(&ext.id).unwrap().is_none());
    }

    #[test]
    fn test_insert_and_get_audit_result() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        let audit = AuditResult {
            extension_id: ext.id.clone(),
            findings: vec![AuditFinding {
                rule_id: "prompt-injection".into(),
                severity: Severity::Critical,
                message: "Found prompt injection pattern".into(),
                location: "SKILL.md:5".into(),
            }],
            trust_score: 75,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit).unwrap();

        let results = store.get_audit_results(&ext.id).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].trust_score, 75);
        assert_eq!(results[0].findings.len(), 1);
        assert_eq!(results[0].findings[0].rule_id, "prompt-injection");
    }

    #[test]
    fn test_update_trust_score() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store.update_trust_score(&ext.id, 85).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.trust_score, Some(85));
    }

    #[test]
    fn test_update_tags() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        store.update_tags(&ext.id, &["security".into(), "audit".into()]).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.tags, vec!["security", "audit"]);
    }

    #[test]
    fn test_get_all_tags() {
        let (store, _dir) = test_store();
        let mut ext1 = sample_extension();
        ext1.tags = vec!["security".into(), "audit".into()];
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = uuid::Uuid::new_v4().to_string();
        ext2.tags = vec!["audit".into(), "testing".into()];
        store.insert_extension(&ext2).unwrap();

        let tags = store.get_all_tags().unwrap();
        assert_eq!(tags, vec!["audit", "security", "testing"]);
    }

    #[test]
    fn test_update_category() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        assert_eq!(store.get_extension(&ext.id).unwrap().unwrap().category, None);

        store.update_category(&ext.id, Some("Security")).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.category, Some("Security".to_string()));

        store.update_category(&ext.id, None).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.category, None);
    }

    #[test]
    fn test_insert_and_list_projects() {
        let (store, _dir) = test_store();
        let project = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
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
        };
        let project2 = Project {
            id: "proj-002".into(),
            name: "my-project-dup".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
        };
        store.insert_project(&project1).unwrap();
        store.insert_project(&project2).unwrap();
        let projects = store.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, "proj-001");
    }

    #[test]
    fn test_disabled_config_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        assert!(store.get_disabled_config(&ext.id).unwrap().is_none());

        let config = r#"{"command":"npx","args":["-y","@mcp/server"]}"#;
        store.set_disabled_config(&ext.id, Some(config)).unwrap();
        assert_eq!(store.get_disabled_config(&ext.id).unwrap().unwrap(), config);

        store.set_disabled_config(&ext.id, None).unwrap();
        assert!(store.get_disabled_config(&ext.id).unwrap().is_none());
    }

    #[test]
    fn test_delete_project() {
        let (store, _dir) = test_store();
        let project = Project {
            id: "proj-001".into(),
            name: "my-project".into(),
            path: "/tmp/my-project".into(),
            created_at: Utc::now(),
        };
        store.insert_project(&project).unwrap();
        store.delete_project("proj-001").unwrap();
        let projects = store.list_projects().unwrap();
        assert!(projects.is_empty());
    }
}
