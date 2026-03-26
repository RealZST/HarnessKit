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

            CREATE INDEX IF NOT EXISTS idx_extensions_kind ON extensions(kind);
            CREATE INDEX IF NOT EXISTS idx_audit_results_ext ON audit_results(extension_id);
            "
        )?;
        Ok(())
    }

    pub fn insert_extension(&self, ext: &Extension) -> Result<()> {
        self.conn.execute(
            "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
            ],
        )?;
        Ok(())
    }

    pub fn get_extension(&self, id: &str) -> Result<Option<Extension>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at
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
        let mut sql = "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at FROM extensions WHERE 1=1".to_string();
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

    pub fn update_trust_score(&self, id: &str, score: u8) -> Result<()> {
        self.conn.execute(
            "UPDATE extensions SET trust_score = ?1 WHERE id = ?2",
            params![score as i32, id],
        )?;
        Ok(())
    }

    pub fn delete_extension(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
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

    fn row_to_extension(&self, row: &rusqlite::Row) -> Result<Extension> {
        let kind_str: String = row.get(1)?;
        let source_json: String = row.get(4)?;
        let agents_json: String = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let permissions_json: String = row.get(7)?;
        let installed_at_str: String = row.get(10)?;
        let updated_at_str: String = row.get(11)?;

        Ok(Extension {
            id: row.get(0)?,
            kind: kind_str.parse()?,
            name: row.get(2)?,
            description: row.get(3)?,
            source: serde_json::from_str(&source_json)?,
            agents: serde_json::from_str(&agents_json)?,
            tags: serde_json::from_str(&tags_json)?,
            permissions: serde_json::from_str(&permissions_json)?,
            enabled: row.get::<_, i32>(8)? != 0,
            trust_score: row.get::<_, Option<i32>>(9)?.map(|s| s as u8),
            installed_at: DateTime::parse_from_rfc3339(&installed_at_str)?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)?
                .with_timezone(&Utc),
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
            permissions: vec![Permission::FileSystem {
                paths: vec!["/tmp".into()],
            }],
            enabled: true,
            trust_score: None,
            installed_at: Utc::now(),
            updated_at: Utc::now(),
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
}
