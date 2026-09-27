//! Connection lifecycle: open, schema migrations v1-v9, version queries.

use super::*;

impl Store {
    pub fn open(path: &Path) -> Result<Self, HkError> {
        let conn = Connection::open(path)?;
        // secure_delete zeroes freed pages instead of leaving their bytes in the
        // freelist. `disabled_config` holds a disabled MCP entry verbatim,
        // secrets included, so its bytes must not outlive the row.
        //
        // Both pragmas are per-connection, not stored in the file: every
        // connection has to set them itself. This is the only place the
        // HarnessKit DB is opened (the other `Connection::open` calls in the
        // crate read *other* agents' databases), so setting them here covers
        // every delete. A second connection added later must repeat them.
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA secure_delete = ON")?;
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
    ///
    /// The copy inherits the secret-bearing `disabled_config` column, so it is
    /// as sensitive as the DB itself. No explicit chmod is needed: `fs::copy`
    /// carries over the source's permission bits, and any DB old enough to need
    /// a migration has been through `Store::open`, which sets 0o600 on Unix.
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
        if current_version < 3 { self.migrate_v3()?; }
        if current_version < 4 { self.migrate_v4()?; }
        if current_version < 5 { self.migrate_v5()?; }
        if current_version < 6 { self.migrate_v6()?; }
        if current_version < 7 { self.migrate_v7()?; }
        if current_version < 8 { self.migrate_v8()?; }
        if current_version < 9 { self.migrate_v9()?; }

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
    /// Schema v3: scope_json column on extensions for global vs project tracking.
    /// NULL is interpreted as global (legacy rows scanned before scope tracking).
    fn migrate_v3(&self) -> Result<(), HkError> {
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN scope_json TEXT");
        Ok(())
    }
    /// Schema v4: scope_json column on custom_config_paths so user-added
    /// custom paths surface under the scope they were added in. NULL is
    /// interpreted as Global (legacy rows added before scope tracking).
    fn migrate_v4(&self) -> Result<(), HkError> {
        self.migrate_add_column(
            "ALTER TABLE custom_config_paths ADD COLUMN scope_json TEXT",
        );
        Ok(())
    }
    /// Schema v5: Kits and Project Stacks.
    fn migrate_v5(&self) -> Result<(), HkError> {
        self.conn.execute_batch(
            "
        CREATE TABLE IF NOT EXISTS kits (
            id            TEXT PRIMARY KEY,
            name          TEXT NOT NULL UNIQUE,
            description   TEXT NOT NULL DEFAULT '',
            zip_path      TEXT NOT NULL,
            created_at    TEXT NOT NULL,
            updated_at    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS kit_assets (
            kit_id        TEXT NOT NULL REFERENCES kits(id) ON DELETE CASCADE,
            extension_id  TEXT NOT NULL,
            asset_name    TEXT NOT NULL,
            position      INTEGER NOT NULL,
            PRIMARY KEY (kit_id, extension_id)
        );

        CREATE TABLE IF NOT EXISTS kit_config_files (
            kit_id           TEXT NOT NULL REFERENCES kits(id) ON DELETE CASCADE,
            agent            TEXT NOT NULL,
            category         TEXT NOT NULL,
            source_path      TEXT NOT NULL,
            source_file_name TEXT NOT NULL,
            position         INTEGER NOT NULL,
            PRIMARY KEY (kit_id, agent, category, source_path)
        );

        CREATE TABLE IF NOT EXISTS kit_sync_records (
            id            TEXT PRIMARY KEY,
            kit_id        TEXT NOT NULL,
            project_path  TEXT NOT NULL,
            agent_name    TEXT NOT NULL,
            written_paths TEXT NOT NULL,
            synced_at     TEXT NOT NULL,
            UNIQUE (kit_id, project_path, agent_name)
        );

        CREATE INDEX IF NOT EXISTS idx_kit_sync_records_kit
            ON kit_sync_records(kit_id);
        CREATE INDEX IF NOT EXISTS idx_kit_sync_records_project
            ON kit_sync_records(project_path);
        "
        )?;
        Ok(())
    }
    /// Schema v7: extend `kit_config_files` PK to include `source_path` so
    /// a Kit can carry multiple files under the same (agent, category) pair
    /// — e.g. several CLAUDE.md files from different projects all stored
    /// under (claude, rules). Old PK forced one row per (agent, category).
    fn migrate_v7(&self) -> Result<(), HkError> {
        self.conn.execute_batch(
            "CREATE TABLE kit_config_files_new (
                kit_id           TEXT NOT NULL REFERENCES kits(id) ON DELETE CASCADE,
                agent            TEXT NOT NULL,
                category         TEXT NOT NULL,
                source_path      TEXT NOT NULL,
                source_file_name TEXT NOT NULL,
                position         INTEGER NOT NULL,
                PRIMARY KEY (kit_id, agent, category, source_path)
             );
             INSERT INTO kit_config_files_new
                 SELECT kit_id, agent, category, source_path, source_file_name, position
                   FROM kit_config_files;
             DROP TABLE kit_config_files;
             ALTER TABLE kit_config_files_new RENAME TO kit_config_files;",
        )?;
        Ok(())
    }
    /// Schema v8: drop the legacy `project_stacks` table. The v1 Stacks UI was
    /// removed and per-project install state is now derived from
    /// `kit_sync_records` (see [`crate::kits::install_records::list_project_install_records`]).
    fn migrate_v8(&self) -> Result<(), HkError> {
        self.conn.execute_batch(
            "DROP INDEX IF EXISTS idx_project_stacks_project;
             DROP TABLE IF EXISTS project_stacks;",
        )?;
        Ok(())
    }
    /// Schema v9: mcp_transport column on extensions (stdio/http/sse) so the
    /// UI can gate remote-MCP install targets. NULL for non-MCP rows and rows
    /// scanned before transport tracking.
    fn migrate_v9(&self) -> Result<(), HkError> {
        self.migrate_add_column("ALTER TABLE extensions ADD COLUMN mcp_transport TEXT");
        Ok(())
    }
    /// Schema v6: drop `kits.refreshed_at`. Kits are immutable snapshots.
    fn migrate_v6(&self) -> Result<(), HkError> {
        // Swallow "no such column" so the migration is idempotent on fresh DBs.
        let res = self
            .conn
            .execute_batch("ALTER TABLE kits DROP COLUMN refreshed_at;");
        if let Err(e) = res {
            let msg = e.to_string().to_lowercase();
            if !msg.contains("no such column") {
                return Err(e.into());
            }
        }
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
    #[cfg(test)]
    pub fn conn_for_test(&self) -> &rusqlite::Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::*;
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
    #[test]
    fn test_open_and_migrate() {
        let (store, _dir) = test_store();
        let exts = store.list_extensions(None, None).unwrap();
        assert!(exts.is_empty());
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
}
