import { invoke } from "@tauri-apps/api/core";
import type { Extension, ExtensionContent, AgentInfo, DashboardStats, AuditResult, UpdateStatus, MarketplaceItem, SkillAuditInfo, Project, DiscoveredProject } from "./types";

export const api = {
  listExtensions(kind?: string, agent?: string): Promise<Extension[]> {
    return invoke("list_extensions", { kind, agent });
  },

  listAgents(): Promise<AgentInfo[]> {
    return invoke("list_agents");
  },

  getDashboardStats(): Promise<DashboardStats> {
    return invoke("get_dashboard_stats");
  },

  toggleExtension(id: string, enabled: boolean): Promise<void> {
    return invoke("toggle_extension", { id, enabled });
  },

  listAuditResults(): Promise<AuditResult[]> {
    return invoke("list_audit_results");
  },

  runAudit(): Promise<AuditResult[]> {
    return invoke("run_audit");
  },

  scanAndSync(): Promise<number> {
    return invoke("scan_and_sync");
  },

  deleteExtension(id: string): Promise<void> {
    return invoke("delete_extension", { id });
  },

  getExtensionContent(id: string): Promise<ExtensionContent> {
    return invoke("get_extension_content", { id });
  },

  checkUpdates(): Promise<[string, UpdateStatus][]> {
    return invoke("check_updates");
  },

  installFromGit(url: string, targetAgent?: string, skillId?: string): Promise<string> {
    return invoke("install_from_git", { url, targetAgent, skillId });
  },

  updateTags(id: string, tags: string[]): Promise<void> {
    return invoke("update_tags", { id, tags });
  },

  getAllTags(): Promise<string[]> {
    return invoke("get_all_tags");
  },

  updateCategory(id: string, category: string | null): Promise<void> {
    return invoke("update_category", { id, category });
  },

  searchMarketplace(query: string, kind: string, limit?: number): Promise<MarketplaceItem[]> {
    return invoke("search_marketplace", { query, kind, limit });
  },

  trendingMarketplace(kind: string, limit?: number): Promise<MarketplaceItem[]> {
    return invoke("trending_marketplace", { kind, limit });
  },

  fetchSkillPreview(source: string, skillId: string): Promise<string> {
    return invoke("fetch_skill_preview", { source, skillId });
  },

  fetchSkillAudit(source: string, skillId: string): Promise<SkillAuditInfo | null> {
    return invoke("fetch_skill_audit", { source, skillId });
  },

  installFromMarketplace(source: string, skillId: string, targetAgent?: string): Promise<string> {
    return invoke("install_from_marketplace", { source, skillId, targetAgent });
  },

  deployToAgent(id: string, targetAgent: string): Promise<string> {
    return invoke("deploy_to_agent", { id, targetAgent });
  },

  listProjects(): Promise<Project[]> {
    return invoke("list_projects");
  },

  addProject(path: string): Promise<Project> {
    return invoke("add_project", { path });
  },

  removeProject(id: string): Promise<void> {
    return invoke("remove_project", { id });
  },

  discoverProjects(rootPath: string): Promise<DiscoveredProject[]> {
    return invoke("discover_projects", { rootPath });
  },

  getProjectExtensions(projectPath: string): Promise<Extension[]> {
    return invoke("get_project_extensions", { projectPath });
  },

  updateAgentPath(name: string, path: string | null): Promise<void> {
    return invoke("update_agent_path", { name, path });
  },

  setAgentEnabled(name: string, enabled: boolean): Promise<void> {
    return invoke("set_agent_enabled", { name, enabled });
  },
};
