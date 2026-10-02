use crate::HkError;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

use crate::models::*;

/// Latest schema version supported by this binary.
const LATEST_SCHEMA_VERSION: i64 = 9;

/// One row of `custom_config_paths`: (id, path, label, category, scope_json).
/// `scope_json` is `None` for legacy rows that predate v4 schema migration.
pub type CustomConfigPathRow = (i64, String, String, String, Option<String>);

#[derive(Debug, Clone)]
pub struct KitRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub zip_path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct KitAssetRow {
    pub kit_id: String,
    pub extension_id: String,
    pub asset_name: String,
    pub position: i64,
}

#[derive(Debug, Clone)]
pub struct KitConfigFileRow {
    pub kit_id: String,
    pub agent: String,
    pub category: ConfigCategory,
    pub source_path: String,
    pub source_file_name: String,
    pub position: i64,
}

#[derive(Debug, Clone)]
pub struct SyncRecordRow {
    pub id: String,
    pub kit_id: String,
    pub project_path: String,
    pub agent_name: String,
    /// written_paths uses kind-prefixed encoding; see service.rs for format
    pub written_paths: Vec<String>,
    pub synced_at: chrono::DateTime<chrono::Utc>,
}

fn parse_dt(s: String) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(&s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|e| {
            eprintln!("[hk] parse_dt: invalid RFC3339 {:?}: {}", s, e);
            chrono::Utc::now()
        })
}

/// The on-disk entry to stat for symlink detection: the containing directory
/// for a `<dir>/SKILL.md` skill, or the standalone `.md` file itself. The
/// scanner always records a dir-skill's `source_path` as `<dir>/SKILL.md`, even
/// when the on-disk file is `SKILL.md.disabled`, so only `SKILL.md` is matched.
fn skill_entry_path(source_path: &str) -> &Path {
    let p = Path::new(source_path);
    match p.file_name().and_then(|f| f.to_str()) {
        Some("SKILL.md") => p.parent().unwrap_or(p),
        _ => p,
    }
}

/// Column list for every SELECT that feeds `row_to_extension`, which reads
/// columns by positional index — order and completeness are load-bearing
/// (missing tail columns don't error, they silently yield defaults).
const EXTENSION_COLUMNS: &str = "id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, install_type, install_url, install_url_resolved, install_branch, install_subpath, install_revision, remote_revision, checked_at, check_error, pack, scope_json, mcp_transport";

/// Upsert SQL for scanner-derived extensions (18 columns, no install meta).
/// Used by `sync_extensions` and `sync_extensions_for_agent`.
const UPSERT_EXTENSION_SQL: &str =
    "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, pack, scope_json, mcp_transport)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
     ON CONFLICT(id) DO UPDATE SET
       kind = excluded.kind,
       name = excluded.name,
       description = excluded.description,
       source_json = excluded.source_json,
       agents_json = excluded.agents_json,
       permissions_json = excluded.permissions_json,
       enabled = CASE WHEN extensions.disabled_config IS NULL THEN excluded.enabled ELSE extensions.enabled END,
       installed_at = extensions.installed_at,
       updated_at = excluded.updated_at,
       pack = COALESCE(extensions.pack, excluded.pack),
       source_path = excluded.source_path,
       cli_parent_id = excluded.cli_parent_id,
       cli_meta_json = excluded.cli_meta_json,
       scope_json = excluded.scope_json,
       mcp_transport = excluded.mcp_transport
       /* install meta columns intentionally excluded — preserved across re-scans */";

/// Full upsert SQL for `insert_extension` (27 columns, includes install meta).
const UPSERT_EXTENSION_FULL_SQL: &str =
    "INSERT INTO extensions (id, kind, name, description, source_json, agents_json, tags_json, permissions_json, enabled, trust_score, installed_at, updated_at, category, source_path, cli_parent_id, cli_meta_json, install_type, install_url, install_url_resolved, install_branch, install_subpath, install_revision, remote_revision, checked_at, check_error, pack, scope_json, mcp_transport)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28)
     ON CONFLICT(id) DO UPDATE SET
       kind = excluded.kind,
       name = excluded.name,
       description = excluded.description,
       source_json = excluded.source_json,
       agents_json = excluded.agents_json,
       permissions_json = excluded.permissions_json,
       enabled = CASE WHEN extensions.disabled_config IS NULL THEN excluded.enabled ELSE extensions.enabled END,
       installed_at = extensions.installed_at,
       updated_at = excluded.updated_at,
       pack = COALESCE(extensions.pack, excluded.pack),
       source_path = excluded.source_path,
       cli_parent_id = excluded.cli_parent_id,
       cli_meta_json = excluded.cli_meta_json,
       scope_json = excluded.scope_json,
       mcp_transport = excluded.mcp_transport";

pub struct Store {
    conn: Connection,
    /// Path to the database file, used for pre-migration backups.
    db_path: PathBuf,
}

mod audit;
mod extensions;
mod install_meta;
mod kits;
mod projects;
mod schema;
mod settings;
mod sync;
#[cfg(test)]
mod test_support;
