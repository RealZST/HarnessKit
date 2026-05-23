import type {
  AgentConfigFile,
  ConfigCategory,
  Extension,
  ExtensionKind,
} from "@/lib/types";

export interface KindCounts {
  skill: number;
  mcp: number;
  plugin: number;
  hook: number;
  cli: number;
}

export interface KitSummary {
  id: string;
  name: string;
  description: string;
  extension_count: number;
  config_file_count: number;
  sync_count: number;
  kind_counts: KindCounts;
  created_at: string;
  updated_at: string;
  corrupt: boolean;
  /** Lowercased haystack built by the backend: name + description + every
   *  asset_name + every config source_file_name. The Kits page header
   *  search does a single case-insensitive `includes` against this field. */
  search_keywords: string;
}

export interface KitConfigFileRef {
  agent: string;
  category: ConfigCategory;
  /** Absolute path on the creator's machine; `null` after import. */
  source_path: string | null;
  source_file_name: string;
}

export interface KitExtensionRef {
  extension_id: string;
  asset_name: string;
  kind: ExtensionKind;
  content_hash: string;
  secrets_stripped: boolean;
}

export interface KitSyncTarget {
  project_path: string;
  agent_name: string;
  synced_at: string;
  file_count: number;
  /** Other agents whose canonical project skill dir is the same as this
   *  target's (e.g. Codex + Antigravity both write to .agents/skills).
   *  Removing files for this agent will also affect these. */
  shared_with: string[];
}

export interface KitDetails {
  summary: KitSummary;
  extensions: KitExtensionRef[];
  config_files: KitConfigFileRef[];
  sync_targets: KitSyncTarget[];
}

export interface CreateKitRequest {
  name: string;
  description: string;
  extension_ids: string[];
  config_files: KitConfigFileRef[];
}

export interface UpdateKitRequest extends CreateKitRequest {
  id: string;
}

export interface PreviewKitConflictsRequest {
  kit_id: string;
  project_path: string;
  agent_name: string;
}

export type ConflictReason = "file_exists" | "dir_exists";

export interface KitExtensionConflict {
  extension_id: string;
  asset_name: string;
  target_path: string;
  conflict_reason: ConflictReason;
}

export interface KitConfigConflict {
  agent: string;
  category: ConfigCategory;
  target_path: string;
}

export interface KitConflictPreview {
  extension_conflicts: KitExtensionConflict[];
  config_conflicts: KitConfigConflict[];
}

export interface SyncKitRequest {
  kit_id: string;
  project_path: string;
  agent_name: string;
  force_overwrite_extension_ids?: string[];
  force_overwrite_config_keys?: string[];
}

export interface UnsyncKitRequest {
  kit_id: string;
  project_path: string;
  agent_name: string;
}

export interface KitSyncResult {
  installed_count: number;
  skipped_conflict_count: number;
  skipped_paths: string[];
  written_paths: string[];
}

export interface KitAssetCandidates {
  extensions: Extension[];
  config_files: AgentConfigFile[];
}

export interface ProjectKitInstallEntry {
  kit_id: string;
  kit_name: string;
  agent_name: string;
  position: number;
  last_synced_at: string | null;
}

export interface ProjectInstallRecords {
  project_path: string;
  entries: ProjectKitInstallEntry[];
}
