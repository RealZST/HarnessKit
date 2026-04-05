use crate::HkError;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

use crate::models::*;

/// Latest schema version supported by this binary.
const LATEST_SCHEMA_VERSION: i64 = 2;

/// Upsert SQL for scanner-derived extensions (17 columns, no install meta).
/// Used by `sync_extensions` and `sync_extensions_for_agent`.
const UPSERT_EXTENSION_SQL: &str =
    "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, pack)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
     ON CONFLICT(id) DO UPDATE SET
       kind = excluded.kind,
       name = excluded.name,
       description = excluded.description,
       source_json = excluded.source_json,
       agents_json = excluded.agents_json,
       permissions_json = excluded.permissions_json,
       installed_at = extensions.installed_at,
       updated_at = excluded.updated_at,
       pack = COALESCE(extensions.pack, excluded.pack),
       source_path = excluded.source_path,
       cli_parent_id = excluded.cli_parent_id,
       cli_meta_json = excluded.cli_meta_json
       /* install meta columns intentionally excluded — preserved across re-scans */";

/// Full upsert SQL for `insert_extension` (26 columns, includes install meta).
const UPSERT_EXTENSION_FULL_SQL: &str =
    "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, install_type, install_url, install_url_resolved, install_branch, install_subpath, install_revision, remote_revision, checked_at, check_error, pack)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
     ON CONFLICT(id) DO UPDATE SET
       kind = excluded.kind,
       name = excluded.name,
       description = excluded.description,
       source_json = excluded.source_json,
       agents_json = excluded.agents_json,
       permissions_json = excluded.permissions_json,
       installed_at = extensions.installed_at,
       updated_at = excluded.updated_at,
       pack = COALESCE(extensions.pack, excluded.pack),
       source_path = excluded.source_path,
       cli_parent_id = excluded.cli_parent_id,
       cli_meta_json = excluded.cli_meta_json";

pub struct Store {
    conn: Connection,
    /// Path to the database file, used for pre-migration backups.
    db_path: PathBuf,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, HkError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let store = Self { conn, db_path: path.to_path_buf() };
        store.migrate()?;

        // Set file permissions to owner-only on Unix (0o600) to protect
        // the database from being read by other users on the system.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if path.exists() {
                let perms = std::fs::Permissions::from_mode(0o600);
                let _ = std::fs::set_permissions(path, perms);
            }
        }

        let version = store.schema_version().unwrap_or(0);
        if version > LATEST_SCHEMA_VERSION {
            eprintln!(
                "[harnesskit] Warning: database schema v{} is newer than this binary supports (v{})",
                version, LATEST_SCHEMA_VERSION
            );
        }

        Ok(store)
    }

    /// Run an ALTER TABLE migration, ignoring "duplicate column" errors.
    fn migrate_add_column(&self, sql: &str) {
        if let Err(e) = self.conn.execute(sql, []) {
            let msg = e.to_string();
            // "duplicate column name" is expected for idempotent re-runs
            if !msg.contains("duplicate column") {
                eprintln!("[harnesskit] Migration warning: {} — {}", sql, msg);
            }
        }
    }

    /// Back up the database file before running migrations.
    /// The backup is written to `{db_path}.backup-v{current_version}`.
    /// Errors are logged but do not abort the migration — a failed backup
    /// should not prevent the app from starting.
    fn backup_before_migrate(&self, current_version: i64) {
        let backup_path = self.db_path.with_extension(
            format!("db.backup-v{}", current_version),
        );
        // Only create a backup if the DB file actually exists (skip for in-memory / new DBs)
        if self.db_path.exists() && !backup_path.exists()
            && let Err(e) = std::fs::copy(&self.db_path, &backup_path)
        {
            eprintln!(
                "[harnesskit] Warning: failed to back up database before migration: {}",
                e,
            );
        }
    }

    fn migrate(&self) -> Result<(), HkError> {
        // Ensure schema_version table exists and has an initial row
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);
             INSERT OR IGNORE INTO schema_version (rowid, version) VALUES (1, 0);",
        )?;

        let current_version: i64 = self.conn.query_row(
            "SELECT version FROM schema_version WHERE rowid = 1",
            [],
            |row| row.get(0),
        )?;

        // Back up before running any migration
        if current_version < LATEST_SCHEMA_VERSION {
            self.backup_before_migrate(current_version);
        }

        if current_version < 1 { self.migrate_v1()?; }
        if current_version < 2 { self.migrate_v2()?; }

        // Update schema version to latest
        if current_version < LATEST_SCHEMA_VERSION {
            self.conn.execute(
                "UPDATE schema_version SET version = ?1 WHERE rowid = 1",
                params![LATEST_SCHEMA_VERSION],
            )?;
        }

        Ok(())
    }

    /// Schema v1: core tables (extensions, audit_results, projects, etc.)
    fn migrate_v1(&self) -> Result<(), HkError> {
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
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN category TEXT");
        // Migration: add pack column (replaces category for repo-based grouping)
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN pack TEXT");
        // Migration: add last_used_at column for skill usage tracking
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN last_used_at TEXT");
        // Migration: add disabled_config column for real enable/disable
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN disabled_config TEXT");
        // Migration: add source_path column for tracking physical file locations
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN source_path TEXT");
        // Migration: add cli_parent_id for linking child skills to parent CLI
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN cli_parent_id TEXT");
        // Migration: add cli_meta_json for CLI-specific metadata
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN cli_meta_json TEXT");
        // Migration: add install meta columns for install-source tracking
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_type TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_url TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_url_resolved TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_branch TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_subpath TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN install_revision TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN remote_revision TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN checked_at TEXT");
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN check_error TEXT");
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
        // Migration: add sort_order to agent_settings
        self.migrate_add_column("ALTER TABLE agent_settings ADD COLUMN sort_order INTEGER");
        // Migration: custom_config_paths table for user-defined config file/folder paths
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS custom_config_paths (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                agent TEXT NOT NULL,
                path TEXT NOT NULL,
                label TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'settings',
                UNIQUE(agent, path)
            )"
        )?;
        Ok(())
    }

    /// Schema v2: extension_agents join table for efficient agent-based filtering.
    fn migrate_v2(&self) -> Result<(), HkError> {
        self.conn.execute_batch("
            CREATE TABLE IF NOT EXISTS extension_agents (
                extension_id TEXT NOT NULL,
                agent_name TEXT NOT NULL,
                PRIMARY KEY (extension_id, agent_name),
                FOREIGN KEY (extension_id) REFERENCES extensions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_ext_agents_agent ON extension_agents(agent_name);
        ")?;
        // Backfill from existing agents_json (OR IGNORE for idempotency)
        self.conn.execute_batch("
            INSERT OR IGNORE INTO extension_agents (extension_id, agent_name)
            SELECT e.id, json_each.value
            FROM extensions e, json_each(e.agents_json)
            WHERE e.agents_json IS NOT NULL AND e.agents_json != '[]';
        ")?;
        Ok(())
    }

    /// Returns the current schema version of the database.
    pub fn schema_version(&self) -> Result<i64, HkError> {
        self.conn
            .query_row(
                "SELECT version FROM schema_version WHERE rowid = 1",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    // --- Agent settings ---

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

    // --- Custom config paths ---

    pub fn add_custom_config_path(
        &self,
        agent: &str,
        path: &str,
        label: &str,
        category: &str,
    ) -> Result<i64, HkError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO custom_config_paths (agent, path, label, category) VALUES (?1, ?2, ?3, ?4)",
            params![agent, path, label, category],
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
    ) -> Result<Vec<(i64, String, String, String)>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, label, category FROM custom_config_paths WHERE agent = ?1 ORDER BY label"
        )?;
        let rows = stmt
            .query_map(params![agent], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
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

    /// Upsert an extension: insert if new, update scanner-derived fields if existing.
    /// Preserves user-set fields: enabled, tags, pack, trust_score, and install meta.
    pub fn insert_extension(&self, ext: &Extension) -> Result<(), HkError> {
        let im = ext.install_meta.as_ref();
        self.conn.execute(
            UPSERT_EXTENSION_FULL_SQL,
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
                Option::<String>::None,
                ext.source_path,
                ext.cli_parent_id,
                ext.cli_meta.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                im.map(|m| m.install_type.as_str()),
                im.and_then(|m| m.url.as_deref()),
                im.and_then(|m| m.url_resolved.as_deref()),
                im.and_then(|m| m.branch.as_deref()),
                im.and_then(|m| m.subpath.as_deref()),
                im.and_then(|m| m.revision.as_deref()),
                im.and_then(|m| m.remote_revision.as_deref()),
                im.and_then(|m| m.checked_at.map(|t| t.to_rfc3339())),
                im.and_then(|m| m.check_error.as_deref()),
                ext.pack,
            ],
        )?;
        // Keep extension_agents join table in sync
        Self::sync_extension_agents(&self.conn, &ext.id, &ext.agents)?;
        Ok(())
    }

    pub fn get_extension(&self, id: &str) -> Result<Option<Extension>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, install_type, install_url, install_url_resolved, install_branch, install_subpath, install_revision, remote_revision, checked_at, check_error, pack
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

    pub fn list_extensions(
        &self,
        kind: Option<ExtensionKind>,
        agent: Option<&str>,
    ) -> Result<Vec<Extension>, HkError> {
        let ext_cols = "e.id, e.kind, e.name, e.description, e.source_json, e.agents_json, e.tags_json, e.permissions_json, e.enabled, e.trust_score, e.installed_at, e.updated_at, e.category, e.source_path, e.cli_parent_id, e.cli_meta_json, e.install_type, e.install_url, e.install_url_resolved, e.install_branch, e.install_subpath, e.install_revision, e.remote_revision, e.checked_at, e.check_error, e.pack";

        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // Build FROM/WHERE depending on whether we filter by agent
        let mut sql = if let Some(agent_val) = agent {
            param_values.push(Box::new(agent_val.to_string()));
            format!(
                "SELECT DISTINCT {} FROM extensions e INNER JOIN extension_agents ea ON e.id = ea.extension_id WHERE ea.agent_name = ?1",
                ext_cols
            )
        } else {
            format!("SELECT {} FROM extensions e WHERE 1=1", ext_cols)
        };

        if let Some(k) = kind {
            sql.push_str(&format!(" AND e.kind = ?{}", param_values.len() + 1));
            param_values.push(Box::new(k.as_str().to_string()));
        }

        sql.push_str(" ORDER BY e.name ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_ref.as_slice(), |row| Ok(self.row_to_extension(row)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row??);
        }
        Ok(results)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET enabled = ?1 WHERE id = ?2",
            params![enabled as i32, id],
        )?;
        Ok(())
    }

    pub fn get_disabled_config(&self, id: &str) -> Result<Option<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT disabled_config FROM extensions WHERE id = ?1")?;
        let result = stmt.query_row(params![id], |row| row.get::<_, Option<String>>(0));
        match result {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_disabled_config(&self, id: &str, config: Option<&str>) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET disabled_config = ?1 WHERE id = ?2",
            params![config, id],
        )?;
        Ok(())
    }

    /// Persist install source metadata for an extension.
    pub fn set_install_meta(&self, id: &str, meta: &InstallMeta) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET install_type = ?1, install_url = ?2, install_url_resolved = ?3, install_branch = ?4, install_subpath = ?5, install_revision = ?6, remote_revision = ?7, checked_at = ?8, check_error = ?9 WHERE id = ?10",
            params![
                meta.install_type,
                meta.url,
                meta.url_resolved,
                meta.branch,
                meta.subpath,
                meta.revision,
                meta.remote_revision,
                meta.checked_at.map(|t| t.to_rfc3339()),
                meta.check_error,
                id,
            ],
        )?;
        Ok(())
    }

    /// Update remote revision check state for an extension.
    pub fn update_check_state(
        &self,
        id: &str,
        remote_revision: Option<&str>,
        checked_at: DateTime<Utc>,
        check_error: Option<&str>,
    ) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET remote_revision = ?1, checked_at = ?2, check_error = ?3 WHERE id = ?4",
            params![remote_revision, checked_at.to_rfc3339(), check_error, id],
        )?;
        Ok(())
    }

    pub fn update_trust_score(&self, id: &str, score: u8) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET trust_score = ?1 WHERE id = ?2",
            params![score as i32, id],
        )?;
        Ok(())
    }

    pub fn update_tags(&self, id: &str, tags: &[String]) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET tags_json = ?1 WHERE id = ?2",
            params![serde_json::to_string(tags)?, id],
        )?;
        Ok(())
    }

    pub fn get_all_tags(&self) -> Result<Vec<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT tags_json FROM extensions WHERE tags_json != '[]'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut all_tags = std::collections::BTreeSet::new();
        for row in rows {
            let json: String = row?;
            if let Ok(tags) = serde_json::from_str::<Vec<String>>(&json) {
                for tag in tags {
                    all_tags.insert(tag);
                }
            }
        }
        Ok(all_tags.into_iter().collect())
    }

    pub fn update_pack(&self, id: &str, pack: Option<&str>) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET pack = ?1 WHERE id = ?2",
            params![pack, id],
        )?;
        Ok(())
    }

    pub fn get_all_packs(&self) -> Result<Vec<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT pack FROM extensions WHERE pack IS NOT NULL ORDER BY pack")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn find_ids_by_pack(&self, pack: &str) -> Result<Vec<String>, HkError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM extensions WHERE pack = ?1")?;
        let rows = stmt.query_map(params![pack], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Find all extension IDs that share the same source_path as the given extension.
    pub fn find_siblings_by_source_path(&self, id: &str) -> Result<Vec<String>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT e2.id FROM extensions e1
             JOIN extensions e2 ON e1.source_path = e2.source_path
             WHERE e1.id = ?1 AND e1.source_path IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get all child skills linked to a CLI extension
    pub fn get_child_skills(&self, cli_id: &str) -> Result<Vec<Extension>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, install_type, install_url, install_url_resolved, install_branch, install_subpath, install_revision, remote_revision, checked_at, check_error, pack
             FROM extensions WHERE cli_parent_id = ?1"
        )?;
        let rows = stmt.query_map(params![cli_id], |row| Ok(self.row_to_extension(row)))?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row??);
        }
        Ok(results)
    }

    /// Link child skills to a CLI parent
    pub fn link_skills_to_cli(&self, cli_id: &str, skill_ids: &[String]) -> Result<(), HkError> {
        for skill_id in skill_ids {
            self.conn.execute(
                "UPDATE extensions SET cli_parent_id = ?1 WHERE id = ?2",
                params![cli_id, skill_id],
            )?;
        }
        Ok(())
    }

    /// Unlink all children from a CLI (set cli_parent_id to NULL)
    pub fn unlink_cli_children(&self, cli_id: &str) -> Result<(), HkError> {
        self.conn.execute(
            "UPDATE extensions SET cli_parent_id = NULL WHERE cli_parent_id = ?1",
            params![cli_id],
        )?;
        Ok(())
    }

    /// Sync the extension_agents join table for a single extension.
    /// Deletes existing rows and re-inserts from the provided agent list.
    fn sync_extension_agents(conn: &rusqlite::Connection, ext_id: &str, agents: &[String]) -> Result<(), HkError> {
        conn.execute("DELETE FROM extension_agents WHERE extension_id = ?1", params![ext_id])?;
        for agent in agents {
            conn.execute(
                "INSERT INTO extension_agents (extension_id, agent_name) VALUES (?1, ?2)",
                params![ext_id, agent],
            )?;
        }
        Ok(())
    }

    pub fn delete_extension(&self, id: &str) -> Result<(), HkError> {
        self.conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Sync all scanned extensions in a single transaction.
    /// Upserts every extension and removes stale entries that no longer exist on disk.
    /// Much faster than individual insert_extension calls (one fsync instead of N).
    /// NOTE: The ON CONFLICT clause intentionally does NOT touch install meta columns
    /// so that install source metadata survives re-scans.
    pub fn sync_extensions(&self, extensions: &[Extension]) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;

        for ext in extensions {
            tx.execute(
                UPSERT_EXTENSION_SQL,
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
                    Option::<String>::None,
                    ext.source_path,
                    ext.cli_parent_id,
                    ext.cli_meta.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    ext.pack,
                ],
            )?;
            // Keep extension_agents join table in sync
            Self::sync_extension_agents(&tx, &ext.id, &ext.agents)?;
        }

        // Remove stale extensions no longer on disk — but keep:
        // - Disabled ones (intentionally absent from scan results)
        // - Extensions with install_meta (user explicitly installed, e.g. CLI via install_cli)
        //   These may temporarily disappear from scan if binary detection fails on startup.
        let scanned_ids: std::collections::HashSet<&str> =
            extensions.iter().map(|e| e.id.as_str()).collect();
        let stale_ids: Vec<(String, bool, bool)> = {
            let mut stmt = tx.prepare(
                "SELECT id, enabled, (install_type IS NOT NULL) as has_meta FROM extensions"
            )?;
            stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?, row.get::<_, bool>(2)?))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect()
        };
        for (id, enabled, has_install_meta) in &stale_ids {
            if !scanned_ids.contains(id.as_str()) && *enabled && !has_install_meta {
                tx.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            }
        }

        // Backfill install_meta from scanner-detected git source for extensions
        // that have no install metadata yet. This covers:
        // - Skills that existed before harnesskit was installed (user git-cloned them)
        // - Skills from previous versions before install tracking was added
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'git',
                 install_url = json_extract(source_json, '$.url'),
                 install_revision = json_extract(source_json, '$.commit_hash')
             WHERE install_type IS NULL
               AND json_extract(source_json, '$.origin') = 'git'
               AND json_extract(source_json, '$.url') IS NOT NULL",
        )?;

        // Backfill install_type for CLI extensions that were installed before
        // install_meta tracking was added to install_cli
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'cli-registry'
             WHERE install_type IS NULL
               AND kind = 'cli'
               AND cli_meta_json IS NOT NULL",
        )?;

        // Backfill pack from install_url or source_json URL for deployed extensions
        // that lost their git context after being copied to agent directories
        Self::backfill_packs(&tx)?;

        tx.commit()?;
        Ok(())
    }

    /// Sync extensions for a specific agent only — upsert scanned extensions and remove stale ones.
    /// Only deletes stale extensions that belong to the specified agent.
    pub fn sync_extensions_for_agent(
        &self,
        agent: &str,
        extensions: &[Extension],
    ) -> Result<(), HkError> {
        // unchecked_transaction: safe because Store is behind a Mutex (single-writer guaranteed)
        let tx = self.conn.unchecked_transaction()?;
        for ext in extensions {
            tx.execute(
                UPSERT_EXTENSION_SQL,
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
                    Option::<String>::None,
                    ext.source_path,
                    ext.cli_parent_id,
                    ext.cli_meta.as_ref().map(|m| serde_json::to_string(m).unwrap_or_default()),
                    ext.pack,
                ],
            )?;
            // Keep extension_agents join table in sync
            Self::sync_extension_agents(&tx, &ext.id, &ext.agents)?;
        }

        // Remove stale extensions for THIS agent only — keep disabled ones
        // and extensions with install_meta (user-installed, may temporarily not scan)
        let scanned_ids: std::collections::HashSet<&str> =
            extensions.iter().map(|e| e.id.as_str()).collect();
        let stale_ids: Vec<(String, bool, bool)> = {
            let mut stmt = tx.prepare(
                "SELECT DISTINCT e.id, e.enabled, (e.install_type IS NOT NULL) as has_meta
                 FROM extensions e
                 INNER JOIN extension_agents ea ON e.id = ea.extension_id
                 WHERE ea.agent_name = ?1"
            )?;
            stmt.query_map(params![agent], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?, row.get::<_, bool>(2)?))
            })?
                .filter_map(|r| r.ok())
                .collect()
        };
        for (id, enabled, has_install_meta) in &stale_ids {
            if !scanned_ids.contains(id.as_str()) && *enabled && !has_install_meta {
                tx.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            }
        }

        // Backfill install_meta from scanner-detected git source
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'git',
                 install_url = json_extract(source_json, '$.url'),
                 install_revision = json_extract(source_json, '$.commit_hash')
             WHERE install_type IS NULL
               AND json_extract(source_json, '$.origin') = 'git'
               AND json_extract(source_json, '$.url') IS NOT NULL",
        )?;

        // Backfill install_type for CLI extensions missing install_meta
        tx.execute_batch(
            "UPDATE extensions
             SET install_type = 'cli-registry'
             WHERE install_type IS NULL
               AND kind = 'cli'
               AND cli_meta_json IS NOT NULL",
        )?;

        Self::backfill_packs(&tx)?;

        tx.commit()?;
        Ok(())
    }

    /// Backfill `pack` from install_url, source_json URL, or child extensions.
    /// Deployed skills lose their git context after being copied to agent directories,
    /// but install_url retains the repo URL. CLI parent extensions inherit pack from children.
    fn backfill_packs(conn: &rusqlite::Connection) -> Result<(), HkError> {
        // 1. Backfill from own install_url or source_json URL
        let mut stmt = conn.prepare(
            "SELECT id, install_url, json_extract(source_json, '$.url')
             FROM extensions
             WHERE pack IS NULL
               AND (install_url IS NOT NULL OR json_extract(source_json, '$.url') IS NOT NULL)",
        )?;
        let rows: Vec<(String, Option<String>, Option<String>)> = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?
            .filter_map(|r| r.map_err(|e| eprintln!("[hk] row error: {e}")).ok())
            .collect();

        for (id, install_url, source_url) in &rows {
            let url = install_url.as_deref().or(source_url.as_deref());
            if let Some(pack) = url.and_then(crate::scanner::extract_pack_from_url) {
                conn.execute(
                    "UPDATE extensions SET pack = ?1 WHERE id = ?2",
                    params![pack, id],
                )?;
            }
        }

        // 2. CLI parents inherit pack from their children
        conn.execute_batch(
            "UPDATE extensions SET pack = (
                SELECT c.pack FROM extensions c
                WHERE c.cli_parent_id = extensions.id AND c.pack IS NOT NULL
                LIMIT 1
             )
             WHERE pack IS NULL
               AND kind = 'cli'
               AND EXISTS (
                SELECT 1 FROM extensions c
                WHERE c.cli_parent_id = extensions.id AND c.pack IS NOT NULL
               )",
        )?;

        Ok(())
    }

    pub fn insert_audit_result(&self, result: &AuditResult) -> Result<(), HkError> {
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

    pub fn get_audit_results(&self, extension_id: &str) -> Result<Vec<AuditResult>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT extension_id, findings_json, trust_score, audited_at
             FROM audit_results WHERE extension_id = ?1 ORDER BY audited_at DESC",
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
    pub fn list_latest_audit_results(&self) -> Result<Vec<AuditResult>, HkError> {
        let mut stmt = self.conn.prepare(
            "SELECT a.extension_id, a.findings_json, a.trust_score, a.audited_at
             FROM audit_results a
             INNER JOIN (
                 SELECT extension_id, MAX(audited_at) AS max_at
                 FROM audit_results GROUP BY extension_id
             ) latest ON a.extension_id = latest.extension_id AND a.audited_at = latest.max_at
             INNER JOIN extensions e ON a.extension_id = e.id",
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

    /// Count audit findings by severity across all latest audit results.
    /// Uses a single SQL query (list_latest_audit_results) then aggregates
    /// in Rust, replacing the previous N+1 pattern of querying per-extension.
    pub fn count_latest_findings_by_severity(&self) -> Result<std::collections::HashMap<String, usize>, HkError> {
        let results = self.list_latest_audit_results()?;
        let mut counts = std::collections::HashMap::new();
        for result in &results {
            for finding in &result.findings {
                *counts.entry(finding.severity.as_str().to_string()).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    // --- Project methods ---

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

    pub fn delete_project(&self, id: &str) -> Result<(), HkError> {
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
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

    fn row_to_extension(&self, row: &rusqlite::Row) -> Result<Extension, HkError> {
        let kind_str: String = row.get(1)?;
        let source_json: String = row.get(4)?;
        let agents_json: String = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let permissions_json: String = row.get(7)?;
        let installed_at_str: String = row.get(10)?;
        let updated_at_str: String = row.get(11)?;
        let cli_meta_json: Option<String> = row.get::<_, Option<String>>(15).ok().flatten();

        // Install meta columns (16-24)
        let install_type: Option<String> = row.get::<_, Option<String>>(16).ok().flatten();
        let install_meta = install_type.map(|it| {
            let checked_at_str: Option<String> = row.get::<_, Option<String>>(23).ok().flatten();
            InstallMeta {
                install_type: it,
                url: row.get::<_, Option<String>>(17).ok().flatten(),
                url_resolved: row.get::<_, Option<String>>(18).ok().flatten(),
                branch: row.get::<_, Option<String>>(19).ok().flatten(),
                subpath: row.get::<_, Option<String>>(20).ok().flatten(),
                revision: row.get::<_, Option<String>>(21).ok().flatten(),
                remote_revision: row.get::<_, Option<String>>(22).ok().flatten(),
                checked_at: checked_at_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|d| d.with_timezone(&Utc))
                }),
                check_error: row.get::<_, Option<String>>(24).ok().flatten(),
            }
        });

        Ok(Extension {
            id: row.get(0)?,
            kind: kind_str
                .parse()
                .map_err(|e: anyhow::Error| HkError::Internal(e.to_string()))?,
            name: row.get(2)?,
            description: row.get(3)?,
            source: serde_json::from_str(&source_json)?,
            agents: serde_json::from_str(&agents_json)?,
            tags: serde_json::from_str(&tags_json)?,
            pack: row.get::<_, Option<String>>(25).ok().flatten(),
            permissions: serde_json::from_str(&permissions_json)?,
            enabled: row.get::<_, i32>(8)? != 0,
            trust_score: row.get::<_, Option<i32>>(9)?.map(|s| s as u8),
            installed_at: DateTime::parse_from_rfc3339(&installed_at_str)
                .map_err(|e| HkError::Internal(format!("Invalid installed_at timestamp: {e}")))?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| HkError::Internal(format!("Invalid updated_at timestamp: {e}")))?
                .with_timezone(&Utc),
            source_path: row.get::<_, Option<String>>(13).ok().flatten(),
            cli_parent_id: row.get::<_, Option<String>>(14).ok().flatten(),
            cli_meta: cli_meta_json.and_then(|s| serde_json::from_str::<CliMeta>(&s).ok()),
            install_meta,
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

    #[cfg(unix)]
    #[test]
    fn test_db_file_permissions_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("permissions_test.db");
        let _store = Store::open(&db_path).unwrap();
        let perms = std::fs::metadata(&db_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600, "Database file should be owner-only (0600)");
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

        let skills = store
            .list_extensions(Some(ExtensionKind::Skill), None)
            .unwrap();
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
        store
            .update_tags(&ext.id, &["security".into(), "audit".into()])
            .unwrap();
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
    fn test_update_pack() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();
        assert_eq!(store.get_extension(&ext.id).unwrap().unwrap().pack, None);

        store.update_pack(&ext.id, Some("alice/repo")).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.pack, Some("alice/repo".to_string()));

        store.update_pack(&ext.id, None).unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.pack, None);
    }

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
            exists: true,
        };
        store.insert_project(&project).unwrap();
        store.delete_project("proj-001").unwrap();
        let projects = store.list_projects().unwrap();
        assert!(projects.is_empty());
    }

    #[test]
    fn test_find_siblings_by_source_path() {
        let (store, _dir) = test_store();
        let shared_path = "/home/.agents/skills/my-skill/SKILL.md";

        let mut ext1 = sample_extension();
        ext1.id = "ext-cursor".into();
        ext1.agents = vec!["cursor".into()];
        ext1.source_path = Some(shared_path.to_string());
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = "ext-codex".into();
        ext2.agents = vec!["codex".into()];
        ext2.source_path = Some(shared_path.to_string());
        store.insert_extension(&ext2).unwrap();

        let mut ext3 = sample_extension();
        ext3.id = "ext-claude".into();
        ext3.agents = vec!["claude".into()];
        ext3.source_path = Some("/home/.claude/skills/other/SKILL.md".to_string());
        store.insert_extension(&ext3).unwrap();

        let siblings = store.find_siblings_by_source_path("ext-cursor").unwrap();
        assert_eq!(siblings.len(), 2);
        assert!(siblings.contains(&"ext-cursor".to_string()));
        assert!(siblings.contains(&"ext-codex".to_string()));
    }

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
    fn test_sync_preserves_disabled_extensions() {
        let (store, _dir) = test_store();

        // Insert an extension and disable it
        let mut ext = sample_extension();
        ext.id = "disabled-mcp".into();
        ext.kind = ExtensionKind::Mcp;
        ext.name = "my-mcp".into();
        store.insert_extension(&ext).unwrap();
        store.set_enabled("disabled-mcp", false).unwrap();

        // Sync with an empty scan result (simulating MCP removed from config)
        store.sync_extensions(&[]).unwrap();

        // Disabled extension should survive the sync
        let fetched = store.get_extension("disabled-mcp").unwrap();
        assert!(
            fetched.is_some(),
            "Disabled extension should not be deleted by sync"
        );
        assert!(!fetched.unwrap().enabled);
    }

    #[test]
    fn test_cli_extension_roundtrip() {
        let (store, _dir) = test_store();
        let meta = CliMeta {
            binary_name: "wecom-cli".into(),
            binary_path: Some("/usr/local/bin/wecom-cli".into()),
            install_method: Some("npm".into()),
            credentials_path: Some("~/.config/wecom/bot.enc".into()),
            version: Some("1.2.3".into()),
            api_domains: vec!["qyapi.weixin.qq.com".into()],
        };
        let mut ext = sample_extension();
        ext.kind = ExtensionKind::Cli;
        ext.name = "wecom-cli".into();
        ext.cli_meta = Some(meta.clone());
        store.insert_extension(&ext).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert_eq!(fetched.kind, ExtensionKind::Cli);
        assert_eq!(fetched.name, "wecom-cli");
        let fetched_meta = fetched.cli_meta.unwrap();
        assert_eq!(fetched_meta.binary_name, "wecom-cli");
        assert_eq!(
            fetched_meta.binary_path,
            Some("/usr/local/bin/wecom-cli".into())
        );
        assert_eq!(fetched_meta.install_method, Some("npm".into()));
        assert_eq!(
            fetched_meta.credentials_path,
            Some("~/.config/wecom/bot.enc".into())
        );
        assert_eq!(fetched_meta.version, Some("1.2.3".into()));
        assert_eq!(fetched_meta.api_domains, vec!["qyapi.weixin.qq.com"]);
        assert!(fetched.cli_parent_id.is_none());
    }

    #[test]
    fn test_cli_parent_child_link() {
        let (store, _dir) = test_store();

        // Create CLI parent
        let mut cli = sample_extension();
        cli.id = "cli-parent".into();
        cli.kind = ExtensionKind::Cli;
        cli.name = "my-cli".into();
        cli.cli_meta = Some(CliMeta {
            binary_name: "my-cli".into(),
            binary_path: None,
            install_method: None,
            credentials_path: None,
            version: None,
            api_domains: vec![],
        });
        store.insert_extension(&cli).unwrap();

        // Create 2 child skills
        let mut child1 = sample_extension();
        child1.id = "child-skill-1".into();
        child1.name = "skill-one".into();
        child1.cli_parent_id = Some("cli-parent".into());
        store.insert_extension(&child1).unwrap();

        let mut child2 = sample_extension();
        child2.id = "child-skill-2".into();
        child2.name = "skill-two".into();
        child2.cli_parent_id = Some("cli-parent".into());
        store.insert_extension(&child2).unwrap();

        // Verify get_child_skills returns both
        let children = store.get_child_skills("cli-parent").unwrap();
        assert_eq!(children.len(), 2);
        let child_ids: Vec<&str> = children.iter().map(|c| c.id.as_str()).collect();
        assert!(child_ids.contains(&"child-skill-1"));
        assert!(child_ids.contains(&"child-skill-2"));

        // Verify parent_id roundtrips
        let fetched = store.get_extension("child-skill-1").unwrap().unwrap();
        assert_eq!(fetched.cli_parent_id, Some("cli-parent".to_string()));

        // Unlink, verify empty
        store.unlink_cli_children("cli-parent").unwrap();
        let children = store.get_child_skills("cli-parent").unwrap();
        assert!(children.is_empty());

        // Verify child still exists but has no parent
        let fetched = store.get_extension("child-skill-1").unwrap().unwrap();
        assert!(fetched.cli_parent_id.is_none());
    }

    #[test]
    fn test_link_skills_to_cli() {
        let (store, _dir) = test_store();

        // Create CLI parent
        let mut cli = sample_extension();
        cli.id = "cli-parent".into();
        cli.kind = ExtensionKind::Cli;
        cli.name = "my-cli".into();
        store.insert_extension(&cli).unwrap();

        // Create children without parent initially
        let mut child1 = sample_extension();
        child1.id = "orphan-1".into();
        child1.name = "orphan-one".into();
        store.insert_extension(&child1).unwrap();

        let mut child2 = sample_extension();
        child2.id = "orphan-2".into();
        child2.name = "orphan-two".into();
        store.insert_extension(&child2).unwrap();

        // Link them
        store
            .link_skills_to_cli("cli-parent", &["orphan-1".into(), "orphan-2".into()])
            .unwrap();

        let children = store.get_child_skills("cli-parent").unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_install_meta_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        // Initially no install meta
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        assert!(fetched.install_meta.is_none());

        // Set install meta
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: Some("https://github.com/user/repo.git".into()),
            branch: Some("main".into()),
            subpath: Some("skills/my-skill".into()),
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store.set_install_meta(&ext.id, &meta).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.url.as_deref(), Some("https://github.com/user/repo"));
        assert_eq!(
            im.url_resolved.as_deref(),
            Some("https://github.com/user/repo.git")
        );
        assert_eq!(im.branch.as_deref(), Some("main"));
        assert_eq!(im.subpath.as_deref(), Some("skills/my-skill"));
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert!(im.remote_revision.is_none());
        assert!(im.checked_at.is_none());
        assert!(im.check_error.is_none());
    }

    #[test]
    fn test_update_check_state_roundtrip() {
        let (store, _dir) = test_store();
        let ext = sample_extension();
        store.insert_extension(&ext).unwrap();

        // Set initial install meta
        let meta = InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc123".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        };
        store.set_install_meta(&ext.id, &meta).unwrap();

        // Update check state
        let now = Utc::now();
        store
            .update_check_state(&ext.id, Some("def456"), now, None)
            .unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert_eq!(im.remote_revision.as_deref(), Some("def456"));
        assert!(im.checked_at.is_some());
        assert!(im.check_error.is_none());

        // Update check state with error
        store
            .update_check_state(&ext.id, None, now, Some("network timeout"))
            .unwrap();
        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert!(im.remote_revision.is_none());
        assert_eq!(im.check_error.as_deref(), Some("network timeout"));
    }

    #[test]
    fn test_sync_preserves_install_meta() {
        let (store, _dir) = test_store();

        // Insert extension with install meta
        let mut ext = sample_extension();
        ext.id = "git-skill".into();
        ext.name = "git-skill".into();
        ext.install_meta = Some(InstallMeta {
            install_type: "git".into(),
            url: Some("https://github.com/user/repo".into()),
            url_resolved: None,
            branch: None,
            subpath: None,
            revision: Some("abc123".into()),
            remote_revision: Some("def456".into()),
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        // Verify install meta was stored
        let fetched = store.get_extension("git-skill").unwrap().unwrap();
        assert!(fetched.install_meta.is_some());
        assert_eq!(
            fetched.install_meta.as_ref().unwrap().revision.as_deref(),
            Some("abc123")
        );

        // Sync with the same extension (scanner doesn't know about install meta)
        let mut synced = ext.clone();
        synced.install_meta = None;
        store.sync_extensions(&[synced]).unwrap();

        // Install meta should survive the sync
        let fetched = store.get_extension("git-skill").unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "git");
        assert_eq!(im.revision.as_deref(), Some("abc123"));
        assert_eq!(im.remote_revision.as_deref(), Some("def456"));
    }

    #[test]
    fn test_sync_backfills_install_meta_from_git_source() {
        let (store, _dir) = test_store();

        // Create an extension with git source but no install_meta
        // (simulates a skill that existed before harnesskit was installed)
        let mut ext = sample_extension();
        ext.id = "pre-existing".into();
        ext.name = "pre-existing".into();
        ext.source = Source {
            origin: SourceOrigin::Git,
            url: Some("https://github.com/user/old-skill".into()),
            version: None,
            commit_hash: Some("aaa111".into()),
        };
        ext.install_meta = None;

        // Sync (as if scanner discovered it for the first time)
        store.sync_extensions(&[ext.clone()]).unwrap();

        // install_meta should be backfilled from source_json
        let fetched = store.get_extension("pre-existing").unwrap().unwrap();
        let im = fetched
            .install_meta
            .expect("install_meta should be backfilled");
        assert_eq!(im.install_type, "git");
        assert_eq!(im.url.as_deref(), Some("https://github.com/user/old-skill"));
        assert_eq!(im.revision.as_deref(), Some("aaa111"));
        // Fields not derivable from Source should remain None
        assert!(im.branch.is_none());
        assert!(im.subpath.is_none());
    }

    #[test]
    fn test_sync_backfill_does_not_overwrite_existing_install_meta() {
        let (store, _dir) = test_store();

        // Extension with explicit install_meta (installed through our UI)
        let mut ext = sample_extension();
        ext.id = "our-install".into();
        ext.name = "our-install".into();
        ext.source = Source {
            origin: SourceOrigin::Git,
            url: Some("https://github.com/user/skill".into()),
            version: None,
            commit_hash: Some("new-scan-hash".into()),
        };
        ext.install_meta = Some(InstallMeta {
            install_type: "marketplace".into(),
            url: Some("marketplace-source".into()),
            url_resolved: Some("https://github.com/user/skill".into()),
            branch: None,
            subpath: Some("my-skill".into()),
            revision: Some("original-hash".into()),
            remote_revision: None,
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        // Sync with scanner data (install_meta = None from scanner)
        ext.install_meta = None;
        store.sync_extensions(&[ext]).unwrap();

        // Backfill should NOT overwrite — install_type is already set
        let fetched = store.get_extension("our-install").unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "marketplace"); // NOT overwritten to "git"
        assert_eq!(im.url.as_deref(), Some("marketplace-source")); // preserved
        assert_eq!(im.revision.as_deref(), Some("original-hash")); // NOT overwritten
    }

    #[test]
    fn test_sync_backfill_skips_non_git_sources() {
        let (store, _dir) = test_store();

        // Extension with agent source (no .git detected)
        let mut ext = sample_extension();
        ext.id = "agent-skill".into();
        ext.name = "agent-skill".into();
        ext.source = Source {
            origin: SourceOrigin::Agent,
            url: None,
            version: None,
            commit_hash: None,
        };
        ext.install_meta = None;

        store.sync_extensions(&[ext]).unwrap();

        // Should NOT be backfilled
        let fetched = store.get_extension("agent-skill").unwrap().unwrap();
        assert!(fetched.install_meta.is_none());
    }

    #[test]
    fn test_insert_extension_with_install_meta() {
        let (store, _dir) = test_store();
        let mut ext = sample_extension();
        ext.install_meta = Some(InstallMeta {
            install_type: "marketplace".into(),
            url: Some("https://marketplace.example.com/skill/42".into()),
            url_resolved: None,
            branch: None,
            subpath: Some("42".into()),
            revision: None,
            remote_revision: None,
            checked_at: None,
            check_error: None,
        });
        store.insert_extension(&ext).unwrap();

        let fetched = store.get_extension(&ext.id).unwrap().unwrap();
        let im = fetched.install_meta.unwrap();
        assert_eq!(im.install_type, "marketplace");
        assert_eq!(im.subpath.as_deref(), Some("42"));
    }

    #[test]
    fn test_add_custom_config_path_returns_correct_id_on_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();
        let id1 = store
            .add_custom_config_path("claude", "/some/path", "label", "settings")
            .unwrap();
        // Insert a different path to change last_insert_rowid
        let _id_other = store
            .add_custom_config_path("claude", "/other/path", "label", "settings")
            .unwrap();
        // Now try to insert the first path again - this should return id1, not id_other
        let id2 = store
            .add_custom_config_path("claude", "/some/path", "label", "settings")
            .unwrap();
        assert_eq!(id1, id2, "Duplicate insert should return the same ID");
        assert!(id1 > 0, "ID should be positive");
    }

    #[test]
    fn test_list_all_custom_config_paths_includes_all_agents() {
        let (store, _dir) = test_store();
        store
            .add_custom_config_path("claude", "/tmp/a", "a", "settings")
            .unwrap();
        store
            .add_custom_config_path("codex", "/tmp/b", "b", "rules")
            .unwrap();

        let mut paths = store.list_all_custom_config_paths().unwrap();
        paths.sort();

        assert_eq!(paths, vec!["/tmp/a".to_string(), "/tmp/b".to_string()]);
    }

    #[test]
    fn test_list_extensions_agent_filter_escapes_wildcards() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.db")).unwrap();

        // Insert extension for "claude" agent
        let ext_claude = Extension {
            id: "ext-claude".into(),
            kind: ExtensionKind::Skill,
            name: "claude-skill".into(),
            description: "".into(),
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
            installed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source_path: None,
            cli_parent_id: None,
            cli_meta: None,
            install_meta: None,
        };
        store.insert_extension(&ext_claude).unwrap();

        // A wildcard agent filter should NOT match everything
        let results = store.list_extensions(None, Some("%")).unwrap();
        assert_eq!(results.len(), 0, "Wildcard '%' should not match any agent");
    }

    #[test]
    fn test_extension_agents_join_table_populated_by_insert() {
        let (store, _dir) = test_store();

        let mut ext = sample_extension();
        ext.agents = vec!["claude".into(), "cursor".into()];
        store.insert_extension(&ext).unwrap();

        // Verify join table rows exist
        let count: i64 = store.conn.query_row(
            "SELECT COUNT(*) FROM extension_agents WHERE extension_id = ?1",
            params![ext.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2);

        // Verify agent filter uses the join table correctly
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 1);
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        let codex = store.list_extensions(None, Some("codex")).unwrap();
        assert!(codex.is_empty());
    }

    #[test]
    fn test_extension_agents_join_table_synced_by_sync_extensions() {
        let (store, _dir) = test_store();

        let mut ext1 = sample_extension();
        ext1.id = "ext-1".into();
        ext1.name = "ext-one".into();
        ext1.agents = vec!["claude".into()];

        let mut ext2 = sample_extension();
        ext2.id = "ext-2".into();
        ext2.name = "ext-two".into();
        ext2.agents = vec!["cursor".into(), "claude".into()];

        store.sync_extensions(&[ext1.clone(), ext2.clone()]).unwrap();

        // Verify both come back for claude
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 2);

        // Only ext2 for cursor
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        assert_eq!(cursor[0].id, "ext-2");

        // Re-sync ext1 with changed agents (now also cursor)
        ext1.agents = vec!["claude".into(), "cursor".into()];
        store.sync_extensions(&[ext1, ext2]).unwrap();

        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 2);
    }

    #[test]
    fn test_extension_agents_backfill_from_migration() {
        // Simulate a v1 database by inserting directly then running migrate_v2
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("test.db");
        let store = Store::open(&db_path).unwrap();

        // The migration already ran in open(), but the table should be populated
        // Insert an extension and verify backfill works for freshly-opened DBs
        let mut ext = sample_extension();
        ext.agents = vec!["claude".into(), "codex".into()];
        store.insert_extension(&ext).unwrap();

        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 1);
        let codex = store.list_extensions(None, Some("codex")).unwrap();
        assert_eq!(codex.len(), 1);
    }

    #[test]
    fn test_sync_extensions_for_agent_uses_join_table() {
        let (store, _dir) = test_store();

        let mut ext1 = sample_extension();
        ext1.id = "agent-ext-1".into();
        ext1.name = "agent-ext-one".into();
        ext1.agents = vec!["claude".into()];

        let mut ext2 = sample_extension();
        ext2.id = "agent-ext-2".into();
        ext2.name = "agent-ext-two".into();
        ext2.agents = vec!["cursor".into()];

        // Sync for claude agent
        store.sync_extensions_for_agent("claude", &[ext1.clone()]).unwrap();
        // Sync for cursor agent separately
        store.sync_extensions_for_agent("cursor", &[ext2.clone()]).unwrap();

        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].id, "agent-ext-1");

        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
        assert_eq!(cursor[0].id, "agent-ext-2");

        // Remove ext1 from claude scan — it should be deleted
        store.sync_extensions_for_agent("claude", &[]).unwrap();
        let claude = store.list_extensions(None, Some("claude")).unwrap();
        assert!(claude.is_empty());

        // cursor extension should still exist
        let cursor = store.list_extensions(None, Some("cursor")).unwrap();
        assert_eq!(cursor.len(), 1);
    }

    #[test]
    fn test_count_latest_findings_by_severity() {
        let (store, _dir) = test_store();

        // Create two extensions
        let mut ext1 = sample_extension();
        ext1.id = "ext-1".into();
        ext1.name = "ext-one".into();
        store.insert_extension(&ext1).unwrap();

        let mut ext2 = sample_extension();
        ext2.id = "ext-2".into();
        ext2.name = "ext-two".into();
        store.insert_extension(&ext2).unwrap();

        // Insert audit results for ext1 (2 findings: 1 critical, 1 high)
        let audit1 = AuditResult {
            extension_id: "ext-1".into(),
            findings: vec![
                AuditFinding {
                    rule_id: "rule-a".into(),
                    severity: Severity::Critical,
                    message: "bad".into(),
                    location: "file:1".into(),
                },
                AuditFinding {
                    rule_id: "rule-b".into(),
                    severity: Severity::High,
                    message: "also bad".into(),
                    location: "file:2".into(),
                },
            ],
            trust_score: 60,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit1).unwrap();

        // Insert audit results for ext2 (1 finding: medium)
        let audit2 = AuditResult {
            extension_id: "ext-2".into(),
            findings: vec![AuditFinding {
                rule_id: "rule-c".into(),
                severity: Severity::Medium,
                message: "meh".into(),
                location: "file:3".into(),
            }],
            trust_score: 80,
            audited_at: Utc::now(),
        };
        store.insert_audit_result(&audit2).unwrap();

        let counts = store.count_latest_findings_by_severity().unwrap();
        assert_eq!(counts.get("critical").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("high").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("medium").copied().unwrap_or(0), 1);
        assert_eq!(counts.get("low").copied().unwrap_or(0), 0);
    }

    #[test]
    fn test_count_latest_findings_uses_only_latest_audit() {
        let (store, _dir) = test_store();

        let mut ext = sample_extension();
        ext.id = "ext-latest".into();
        ext.name = "ext-latest".into();
        store.insert_extension(&ext).unwrap();

        // Insert an old audit with 1 critical finding
        let old_audit = AuditResult {
            extension_id: "ext-latest".into(),
            findings: vec![AuditFinding {
                rule_id: "rule-old".into(),
                severity: Severity::Critical,
                message: "old issue".into(),
                location: "file:1".into(),
            }],
            trust_score: 50,
            audited_at: chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
                .unwrap().with_timezone(&Utc),
        };
        store.insert_audit_result(&old_audit).unwrap();

        // Insert a newer audit with 0 findings (resolved)
        let new_audit = AuditResult {
            extension_id: "ext-latest".into(),
            findings: vec![],
            trust_score: 100,
            audited_at: chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
                .unwrap().with_timezone(&Utc),
        };
        store.insert_audit_result(&new_audit).unwrap();

        // Only the latest audit (with 0 findings) should be counted
        let counts = store.count_latest_findings_by_severity().unwrap();
        assert_eq!(counts.get("critical").copied().unwrap_or(0), 0);
    }
}
