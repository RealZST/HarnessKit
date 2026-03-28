export type ExtensionKind = "skill" | "mcp" | "plugin" | "hook";
export type SourceOrigin = "git" | "registry" | "agent" | "local";
export type Severity = "Critical" | "High" | "Medium" | "Low";
export type TrustTier = "Safe" | "LowRisk" | "HighRisk" | "Critical";

export interface Extension {
  id: string;
  kind: ExtensionKind;
  name: string;
  description: string;
  source: Source;
  agents: string[];
  tags: string[];
  category: string | null;
  permissions: Permission[];
  enabled: boolean;
  trust_score: number | null;
  installed_at: string;
  updated_at: string;
  last_used_at: string | null;
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
  | { status: "up_to_date" }
  | { status: "update_available"; remote_hash: string }
  | { status: "error"; message: string };

export interface AgentInfo {
  name: string;
  detected: boolean;
  extension_count: number;
}

export interface ExtensionContent {
  content: string;
  path: string | null;
}

export interface DashboardStats {
  total_extensions: number;
  skill_count: number;
  mcp_count: number;
  plugin_count: number;
  hook_count: number;
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
  kind: "skill" | "mcp";
  installs: number;
  icon_url: string | null;
  verified: boolean;
  categories: string[];
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
}

export interface DiscoveredProject {
  name: string;
  path: string;
}

export function trustTier(score: number): TrustTier {
  if (score >= 80) return "Safe";
  if (score >= 60) return "LowRisk";
  if (score >= 40) return "HighRisk";
  return "Critical";
}

export function trustColor(score: number): string {
  const tier = trustTier(score);
  switch (tier) {
    case "Safe": return "text-primary";
    case "LowRisk": return "text-chart-4";
    case "HighRisk": return "text-chart-5";
    case "Critical": return "text-destructive";
  }
}

export function severityColor(severity: Severity): string {
  switch (severity) {
    case "Critical": return "text-destructive";
    case "High": return "text-chart-5";
    case "Medium": return "text-chart-4";
    case "Low": return "text-muted-foreground";
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
