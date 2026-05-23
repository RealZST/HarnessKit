use crate::models::{ConfigCategory, ExtensionKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-`ExtensionKind` counts for the assets inside a Kit. Used by the Kits
/// folder-grid to render kind-colored "papers" without an N+1 fetch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KindCounts {
    pub skill: usize,
    pub mcp: usize,
    pub plugin: usize,
    pub hook: usize,
    pub cli: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub extension_count: usize,
    pub config_file_count: usize,
    pub sync_count: usize,
    pub kind_counts: KindCounts,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// True when the underlying zip is missing or unreadable.
    pub corrupt: bool,
    /// Pre-computed lowercased haystack for the Kits page search box —
    /// concatenates name, description, every asset_name, and every config
    /// source_file_name with spaces. Frontend does a single `includes` against
    /// the query (also lowercased) instead of N field comparisons per row.
    pub search_keywords: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitConfigFileRef {
    pub agent: String,
    pub category: ConfigCategory,
    /// Absolute path of the config file source on the creator's machine.
    /// `None` after import — the original sender's filesystem layout is unknown
    /// on the receiver's machine; only `source_file_name` is preserved.
    pub source_path: Option<String>,
    pub source_file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitExtensionRef {
    pub extension_id: String,
    pub asset_name: String,
    pub kind: ExtensionKind,
    /// Hash from the zip's manifest.
    pub content_hash: String,
    /// True when this entry is in the manifest's `secrets_stripped` list
    /// (set on MCP servers whose env values were blanked at pack time).
    /// UI surfaces a "Configure secrets after install" hint per spec §8.
    pub secrets_stripped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitSyncTarget {
    pub project_path: String,
    pub agent_name: String,
    pub synced_at: DateTime<Utc>,
    pub file_count: usize,
    /// Other agent names whose canonical project skill dir is the same as
    /// this target's — e.g. Codex and Antigravity both write to
    /// `.agents/skills`. Removing files for this agent will also remove
    /// them for these agents, so the UI surfaces a warning. Empty when the
    /// install path is exclusive to this agent.
    #[serde(default)]
    pub shared_with: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitDetails {
    pub summary: KitSummary,
    pub extensions: Vec<KitExtensionRef>,
    pub config_files: Vec<KitConfigFileRef>,
    pub sync_targets: Vec<KitSyncTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKitRequest {
    pub name: String,
    pub description: String,
    pub extension_ids: Vec<String>,
    pub config_files: Vec<KitConfigFileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKitRequest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub extension_ids: Vec<String>,
    pub config_files: Vec<KitConfigFileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewKitConflictsRequest {
    pub kit_id: String,
    pub project_path: String,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitExtensionConflict {
    pub extension_id: String,
    pub asset_name: String,
    pub target_path: String,
    pub conflict_reason: ConflictReason,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictReason {
    FileExists,
    DirExists,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitConfigConflict {
    pub agent: String,
    pub category: ConfigCategory,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitConflictPreview {
    pub extension_conflicts: Vec<KitExtensionConflict>,
    pub config_conflicts: Vec<KitConfigConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncKitRequest {
    pub kit_id: String,
    pub project_path: String,
    pub agent_name: String,
    #[serde(default)]
    pub force_overwrite_extension_ids: Vec<String>,
    /// Keys formatted as `"<agent>:<category>"`, e.g. `"claude:rules"`.
    #[serde(default)]
    pub force_overwrite_config_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsyncKitRequest {
    pub kit_id: String,
    pub project_path: String,
    pub agent_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitSyncResult {
    pub installed_count: usize,
    pub skipped_conflict_count: usize,
    pub skipped_paths: Vec<String>,
    pub written_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KitAssetCandidates {
    pub extensions: Vec<crate::models::Extension>,
    pub config_files: Vec<crate::models::AgentConfigFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectKitInstallEntry {
    pub kit_id: String,
    pub kit_name: String,
    pub agent_name: String,
    pub position: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInstallRecords {
    pub project_path: String,
    pub entries: Vec<ProjectKitInstallEntry>,
}

