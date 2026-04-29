export type ExtensionKind = "skill" | "mcp" | "plugin" | "hook" | "cli";
export type SourceOrigin = "git" | "registry" | "agent" | "local";
export type Severity = "Critical" | "High" | "Medium" | "Low";
export type TrustTier = "Safe" | "LowRisk" | "NeedsReview";

export interface InstallMeta {
  install_type: string;
  url: string | null;
  url_resolved: string | null;
  branch: string | null;
  subpath: string | null;
  revision: string | null;
  remote_revision: string | null;
  checked_at: string | null;
  check_error: string | null;
}

export interface Extension {
  id: string;
  kind: ExtensionKind;
  name: string;
  description: string;
  source: Source;
  agents: string[];
  tags: string[];
  pack: string | null;
  permissions: Permission[];
  enabled: boolean;
  trust_score: number | null;
  installed_at: string;
  updated_at: string;
  source_path: string | null;
  cli_parent_id: string | null;
  cli_meta: CliMeta | null;
  install_meta: InstallMeta | null;
  /** Whether this extension is installed globally or in a specific project.
   *  Defaults to global on rows scanned before scope tracking (DB v3+). */
  scope: ConfigScope;
}

export interface Source {
  origin: SourceOrigin;
  url: string | null;
  version: string | null;
  commit_hash: string | null;
}

export type Permission =
  | { type: "filesystem"; paths: string[] }
  | { type: "network"; domains: string[] }
  | { type: "shell"; commands: string[] }
  | { type: "database"; engines: string[] }
  | { type: "env"; keys: string[] };

export interface CliMeta {
  binary_name: string;
  binary_path: string | null;
  install_method: string | null;
  credentials_path: string | null;
  version: string | null;
  api_domains: string[];
}

/** An extension group merging the same skill across multiple agents. */
export interface GroupedExtension {
  groupKey: string;
  name: string;
  kind: ExtensionKind;
  description: string;
  source: Source;
  agents: string[];
  tags: string[];
  pack: string | null;
  permissions: Permission[];
  enabled: boolean;
  trust_score: number | null;
  installed_at: string;
  updated_at: string;
  instances: Extension[];
}

/** Extract owner/repo from a source URL (e.g. "github.com/alice/repo" → "alice/repo"). */
function extractDeveloper(url: string | null): string {
  if (!url) return "";
  const match = url.match(/github\.com\/([^/]+\/[^/]+)/);
  if (match) return match[1].replace(/\.git$/, "");
  return url;
}

/** Stable grouping key: same kind + name + developer → same group.
 *  Origin is intentionally excluded so the same logical skill installed in
 *  different scopes (e.g. global registry copy + project-local copy) or via
 *  different install methods (git clone vs. marketplace) folds into one row;
 *  the merged row exposes both instances.
 *
 *  For hooks, group by command only (ignore event name) so the same command
 *  deployed to agents with different event names merges into one row.
 *
 *  URL resolution: marketplace-installed skills end up with `source.url=null`
 *  because the scanner re-discovers them as files in agent skill dirs and has
 *  no way to know they came from a marketplace. The authoritative "where did
 *  this come from" record lives in `install_meta.url` (written by HK at
 *  install time). Fall back to it so the 6 marketplace copies of the same
 *  skill group together and stay separate from a same-named hand-written
 *  project skill (which has neither field set). */
export function extensionGroupKey(ext: Extension): string {
  let name = ext.name;
  if (ext.kind === "hook") {
    // name format is "event:matcher:command" — extract just the command part
    const parts = name.split(":");
    if (parts.length >= 3) {
      name = parts.slice(2).join(":");
    }
  }
  const url = ext.source.url ?? ext.install_meta?.url ?? null;
  return `${ext.kind}\0${name}\0${extractDeveloper(url)}`;
}

/** Sort agent name strings by canonical display order. */
export function sortAgentNames(
  names: string[],
  order: readonly string[] = AGENT_ORDER,
): string[] {
  const idx = new Map<string, number>(order.map((n, i) => [n, i]));
  return [...names].sort((a, b) => (idx.get(a) ?? 99) - (idx.get(b) ?? 99));
}

export interface AuditResult {
  extension_id: string;
  findings: AuditFinding[];
  trust_score: number;
  audited_at: string;
}

export interface AuditFinding {
  rule_id: string;
  severity: Severity;
  message: string;
  location: string;
}

export type UpdateStatus =
  | { status: "up_to_date"; remote_hash: string }
  | { status: "update_available"; remote_hash: string }
  | { status: "removed_from_repo" }
  | { status: "error"; message: string };

export interface NewRepoSkill {
  repo_url: string;
  pack: string | null;
  skill_id: string;
  name: string;
  description: string;
}

export interface CheckUpdatesResult {
  statuses: [string, UpdateStatus][];
  new_skills: NewRepoSkill[];
}

export interface AgentInfo {
  name: string;
  detected: boolean;
  extension_count: number;
  path: string;
  enabled: boolean;
}

export type ConfigCategory =
  | "rules"
  | "memory"
  | "settings"
  | "workflow"
  | "ignore";

export type ConfigScope =
  | { type: "global" }
  | { type: "project"; name: string; path: string };

/** Stable identifier for a scope, suitable for use as a Map key or filter value.
 *  "global" for the global scope; the project path for project scopes. */
export function scopeKey(scope: ConfigScope): string {
  return scope.type === "global" ? "global" : scope.path;
}

/** Human-readable label for a scope (e.g. "Global" or "myapp"). */
export function scopeLabel(scope: ConfigScope): string {
  return scope.type === "global" ? "Global" : scope.name;
}

export interface AgentConfigFile {
  path: string;
  agent: string;
  category: ConfigCategory;
  scope: ConfigScope;
  file_name: string;
  size_bytes: number;
  modified_at: string | null;
  is_dir: boolean;
  exists: boolean;
  custom_id?: number;
  custom_label?: string;
}

export interface ExtensionCounts {
  skill: number;
  mcp: number;
  plugin: number;
  hook: number;
  cli: number;
}

export interface AgentDetail {
  name: string;
  detected: boolean;
  config_files: AgentConfigFile[];
  extension_counts: ExtensionCounts;
}

export const CONFIG_CATEGORY_LABELS: Record<ConfigCategory, string> = {
  rules: "Rules",
  memory: "Memory",
  settings: "Settings",
  workflow: "Workflows",
  ignore: "Ignore",
};

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  children: FileEntry[] | null;
}

/** Canonical display order for agents across all UI surfaces. */
export const AGENT_ORDER = [
  "claude",
  "codex",
  "gemini",
  "cursor",
  "antigravity",
  "copilot",
  "windsurf",
] as const;

/** Sort an array of agents (or agent-like objects with a `name` field) by a given order. */
export function sortAgents<T extends { name: string }>(
  agents: T[],
  order: readonly string[] = AGENT_ORDER,
): T[] {
  const idx = new Map<string, number>(order.map((n, i) => [n, i]));
  return [...agents].sort(
    (a, b) => (idx.get(a.name) ?? 99) - (idx.get(b.name) ?? 99),
  );
}

/** Human-readable display names for agents. */
const AGENT_DISPLAY_NAMES: Record<string, string> = {
  claude: "Claude Code",
  codex: "Codex",
  gemini: "Gemini CLI",
  cursor: "Cursor",
  antigravity: "Antigravity",
  copilot: "Copilot",
  windsurf: "Windsurf",
};

/** Get the display name for an agent (e.g. "claude" → "Claude Code"). */
export function agentDisplayName(name: string): string {
  return (
    AGENT_DISPLAY_NAMES[name] ?? name.charAt(0).toUpperCase() + name.slice(1)
  );
}

export interface InstallResult {
  name: string;
  was_update: boolean;
  skipped?: boolean;
}

export interface DiscoveredSkill {
  skill_id: string;
  name: string;
  description: string;
  path: string;
}

export type ScanResult =
  | { type: "Installed"; result: InstallResult }
  | { type: "MultipleSkills"; clone_id: string; skills: DiscoveredSkill[] }
  | { type: "NoSkills" };

export interface ExtensionContent {
  content: string;
  path: string | null;
  symlink_target: string | null;
}

export interface DashboardStats {
  total_extensions: number;
  skill_count: number;
  mcp_count: number;
  plugin_count: number;
  hook_count: number;
  cli_count: number;
  critical_issues: number;
  high_issues: number;
  medium_issues: number;
  low_issues: number;
  updates_available: number;
}

export interface MarketplaceItem {
  id: string;
  name: string;
  description: string;
  /** For skills: GitHub "owner/repo". For MCP: Smithery qualified name. */
  source: string;
  /** Skill ID within the repo (for subdirectory lookup) */
  skill_id: string;
  kind: "skill" | "mcp" | "cli";
  installs: number;
  icon_url: string | null;
  verified: boolean;
  categories: string[];
  /** GitHub stars count (CLI items only) */
  stars?: number | null;
  /** Direct URL to the GitHub repo (CLI items only) */
  repo_url?: string | null;
}

export interface SkillAuditInfo {
  ath: AuditPartner | null;
  socket: AuditPartner | null;
  snyk: AuditPartner | null;
}

export interface AuditPartner {
  risk: string | null;
  score: number | null;
  alerts: number | null;
  analyzedAt: string | null;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  created_at: string;
  exists: boolean;
}

export interface DiscoveredProject {
  name: string;
  path: string;
}

export function trustTier(score: number): TrustTier {
  if (score >= 80) return "Safe";
  if (score >= 60) return "LowRisk";
  return "NeedsReview";
}

export function trustColor(score: number): string {
  const tier = trustTier(score);
  switch (tier) {
    case "Safe":
      return "text-trust-safe";
    case "LowRisk":
      return "text-trust-low-risk";
    case "NeedsReview":
      return "text-trust-high-risk";
  }
}

export function severityColor(severity: Severity): string {
  switch (severity) {
    case "Critical":
      return "text-trust-critical";
    case "High":
      return "text-trust-high-risk";
    case "Medium":
      return "text-trust-low-risk";
    case "Low":
      return "text-muted-foreground";
  }
}

export function formatRelativeTime(iso: string): string {
  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffDay > 30) return `${Math.floor(diffDay / 30)}mo ago`;
  if (diffDay > 0) return `${diffDay}d ago`;
  if (diffHour > 0) return `${diffHour}h ago`;
  if (diffMin > 0) return `${diffMin}m ago`;
  return "Just now";
}
