import i18n from "@/lib/i18n";

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
/** Logical name used for grouping. For hooks the wire name is
 *  `event:matcher:command`; we group by command only so the same command
 *  deployed to agents with different events merges into one row. */
export function logicalExtensionName(ext: Extension): string {
  if (ext.kind === "hook") {
    const parts = ext.name.split(":");
    if (parts.length >= 3) return parts.slice(2).join(":");
  }
  return ext.name;
}

/** Display version for a single extension instance.
 *  Prefers semver (source.version, set by registry/marketplace installs) over
 *  git short hash (install_meta.revision, set by git installs). Returns null
 *  for sources that don't track a version (local file skills, agent-bundled
 *  defaults, etc.) so the caller can decide whether to show a placeholder. */
export function instanceVersion(inst: Extension): string | null {
  if (inst.source.version) return inst.source.version;
  if (inst.install_meta?.revision)
    return inst.install_meta.revision.slice(0, 7);
  return null;
}

/** Directory containing this instance's primary file. For skills, `source_path`
 *  points at the SKILL.md file (or SKILL.md.disabled); strip that suffix to
 *  get the dir. For other kinds source_path is already a dir-like path so the
 *  regex is a no-op. Returns null when source_path is missing. */
export function instanceDir(inst: Extension): string | null {
  return inst.source_path
    ? inst.source_path.replace(/\/SKILL\.md(\.disabled)?$/, "")
    : null;
}

/** Authoritative "where did this come from" URL for grouping purposes.
 *  Resolution order: source.url → install_meta.url → pack (synthesized to a
 *  GitHub URL so extractDeveloper handles it uniformly). `pack` is a
 *  user-editable field on the detail panel; treating it as a tiebreaker
 *  means a user can merge two unlinked rows into one group by typing the
 *  owner/repo identifier (e.g. arxiv-search where only one of four copies
 *  carries install_meta from the original install). Returns `null` when an
 *  extension is truly sourceless (hand-written project skill, agent-bundled
 *  global skill the user never linked, etc.). */
export function deriveExtensionUrl(ext: Extension): string | null {
  return (
    ext.source.url ??
    ext.install_meta?.url ??
    (ext.pack ? `https://github.com/${ext.pack}` : null)
  );
}

/** Resolve a group to a GitHub `owner/repo` string for the detail panel's
 *  source pill. Returns `null` when no source is known across the group's
 *  source / install_meta / pack fields, in which case the UI shows the
 *  "Bind source" CTA. Must accept input from any of those three places
 *  because real-world rows mix them: marketplace-installed skills only
 *  carry install_meta; manually-bound skills only carry pack; locally-
 *  cloned git repos may only carry source.url.
 *
 *  For CLI groups, the parent row often lacks install_meta of its own —
 *  the source URL lives on child skill rows. Callers pass `allExtensions`
 *  so we can walk to those children; pass `[]` from contexts that don't
 *  care about CLIs (e.g. unit tests for skill rows). */
export function groupOwnerRepo(
  group: GroupedExtension,
  allExtensions: Extension[],
): string | null {
  if (group.pack) return group.pack;
  let url: string | null =
    group.source.url ??
    group.instances.find((i) => i.install_meta?.url)?.install_meta?.url ??
    null;
  if (!url && group.kind === "cli") {
    url =
      allExtensions.find(
        (e) =>
          e.cli_parent_id === group.instances[0]?.id && e.install_meta?.url,
      )?.install_meta?.url ?? null;
  }
  if (!url) return null;
  const m = url.match(/github\.com\/([^/]+\/[^/]+)/);
  return m ? m[1].replace(/\.git$/, "") : null;
}

/** Whether `pack` matches the GitHub `owner/repo` shape that the backend's
 *  `service::bind_pack` will synthesize an install_meta URL from. Must stay
 *  in sync with `is_valid_pack_format` in `crates/hk-core/src/service.rs`:
 *  exactly one `/`; owner half is alnum / `-` / `_` (no `.`, so non-github
 *  paste forms like `gitlab.com/foo` get rejected before we synthesize a
 *  wrong github URL); repo half also allows `.` (legitimate repo names). */
export function isValidPackFormat(pack: string): boolean {
  const parts = pack.split("/");
  if (parts.length !== 2) return false;
  const owner = /^[\w-]+$/;
  const repo = /^[\w.-]+$/;
  return owner.test(parts[0]) && repo.test(parts[1]);
}

/** Reduce common GitHub source identifiers to the canonical `owner/repo`
 *  form so users can paste raw repo URLs into the detail panel's source
 *  field. Returns the input trimmed-and-unchanged when no recognized pattern
 *  matches; `isValidPackFormat` then decides whether to accept it.
 *
 *  Mirrors `normalize_pack` in `crates/hk-core/src/service.rs` — the backend
 *  applies the same normalization defensively for any non-UI client. */
export function normalizePack(input: string): string {
  const trimmed = input.trim();
  // SSH clone URL: git@github.com:owner/repo[.git]
  const ssh = trimmed.match(
    /^git@github\.com:([^/\s]+)\/([^/\s]+?)(?:\.git)?\/?$/,
  );
  if (ssh) return `${ssh[1]}/${ssh[2]}`;
  // HTTPS / HTTP / schemeless github.com URL. Captures the first two path
  // segments and discards anything after (tree/main, /issues/…, etc.).
  const http = trimmed.match(
    /^(?:https?:\/\/)?github\.com\/([^/\s]+)\/([^/\s]+?)(?:\.git)?(?:\/.*)?$/,
  );
  if (http) return `${http[1]}/${http[2]}`;
  return trimmed;
}

export function extensionGroupKey(ext: Extension): string {
  // When the URL is null, fall back to scopeKey so a project-level
  // "code-review" doesn't accidentally merge with an unrelated global
  // "code-review" of the same name. A future install-to-project of a
  // marketplace skill will set install_meta and the URL branch above wins,
  // so it correctly merges with same-source siblings in other scopes.
  //
  // MCP is the exception: an MCP server's name IS its identity (it's the
  // top-level key in mcpServers / [mcp_servers]), so the same name across
  // scopes/agents always refers to the same logical server. Dropping the
  // scope-fallback for MCP collapses Global + Project copies into one
  // group, matching how Skills merge once they share a URL.
  const url = deriveExtensionUrl(ext);
  const developer =
    ext.kind === "mcp"
      ? url
        ? extractDeveloper(url)
        : ""
      : url
        ? extractDeveloper(url)
        : `(${scopeKey(ext.scope)})`;
  return `${ext.kind}\0${logicalExtensionName(ext)}\0${developer}`;
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
  | "subagents"
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

/** Canonical visual order for config categories across all UI surfaces.
 * Single source of truth — the agent detail render order and the
 * section-anchor rail catalog both derive from this. */
export const CONFIG_CATEGORY_ORDER: ConfigCategory[] = [
  "settings",
  "workflow",
  "rules",
  "subagents",
  "memory",
  "ignore",
];

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
  "opencode",
  "hermes",
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
  opencode: "OpenCode",
  hermes: "Hermes",
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

export function isJustNow(iso: string): boolean {
  return Date.now() - new Date(iso).getTime() < 60_000;
}

export function formatRelativeTime(iso: string, locale?: string): string {
  const lang = locale ?? i18n.resolvedLanguage ?? "en";

  if (isJustNow(iso)) {
    return lang.startsWith("zh") ? "刚刚" : "Just now";
  }

  const diffMs = Date.now() - new Date(iso).getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  const rtf = new Intl.RelativeTimeFormat(lang, {
    numeric: "always",
    style: "narrow",
  });

  if (diffDay > 30) return rtf.format(-Math.floor(diffDay / 30), "month");
  if (diffDay > 0) return rtf.format(-diffDay, "day");
  if (diffHour > 0) return rtf.format(-diffHour, "hour");
  return rtf.format(-diffMin, "minute");
}
