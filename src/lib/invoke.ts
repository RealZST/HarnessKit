import { transport } from "./transport";
import type {
  AgentDetail,
  AgentInfo,
  AuditResult,
  CheckUpdatesResult,
  DashboardStats,
  DiscoveredProject,
  Extension,
  ExtensionContent,
  FileEntry,
  InstallResult,
  MarketplaceItem,
  Project,
  ScanResult,
  SkillAuditInfo,
  UpdateStatus,
} from "./types";

function validateGitUrl(url: string): void {
  if (
    !url.startsWith("https://") &&
    !url.startsWith("git://") &&
    !url.startsWith("git@")
  ) {
    throw new Error("Invalid git URL — must be https://, git://, or git@");
  }
}

function validateNonEmpty(value: string, label: string): void {
  if (!value || !value.trim()) {
    throw new Error(`${label} cannot be empty`);
  }
}

export const api = {
  listExtensions(kind?: string, agent?: string): Promise<Extension[]> {
    return transport("list_extensions", { kind, agent });
  },

  listAgents(): Promise<AgentInfo[]> {
    return transport("list_agents");
  },

  getDashboardStats(): Promise<DashboardStats> {
    return transport("get_dashboard_stats");
  },

  toggleExtension(id: string, enabled: boolean): Promise<void> {
    validateNonEmpty(id, "Extension ID");
    return transport("toggle_extension", { id, enabled });
  },

  listAuditResults(): Promise<AuditResult[]> {
    return transport("list_audit_results");
  },

  runAudit(): Promise<AuditResult[]> {
    return transport("run_audit");
  },

  scanAndSync(): Promise<number> {
    return transport("scan_and_sync");
  },

  deleteExtension(id: string): Promise<void> {
    validateNonEmpty(id, "Extension ID");
    return transport("delete_extension", { id });
  },

  uninstallCliBinary(binaryPath: string): Promise<void> {
    return transport("uninstall_cli_binary", { binaryPath });
  },

  getExtensionContent(id: string): Promise<ExtensionContent> {
    return transport("get_extension_content", { id });
  },

  getCachedUpdateStatuses(): Promise<[string, UpdateStatus][]> {
    return transport("get_cached_update_statuses");
  },

  getSkillLocations(name: string): Promise<[string, string, string | null][]> {
    return transport("get_skill_locations", { name });
  },

  checkUpdates(): Promise<CheckUpdatesResult> {
    return transport("check_updates");
  },

  updateExtension(id: string): Promise<InstallResult> {
    return transport("update_extension", { id });
  },

  installFromLocal(
    path: string,
    targetAgents: string[],
  ): Promise<InstallResult> {
    return transport("install_from_local", { path, targetAgents });
  },

  installFromGit(
    url: string,
    targetAgent?: string,
    skillId?: string,
  ): Promise<InstallResult> {
    validateGitUrl(url);
    return transport("install_from_git", { url, targetAgent, skillId });
  },

  scanGitRepo(url: string, targetAgents: string[]): Promise<ScanResult> {
    return transport("scan_git_repo", { url, targetAgents });
  },

  installScannedSkills(
    cloneId: string,
    skillIds: string[],
    targetAgents: string[],
  ): Promise<InstallResult[]> {
    return transport("install_scanned_skills", {
      cloneId,
      skillIds,
      targetAgents,
    });
  },

  installNewRepoSkills(
    url: string,
    skillIds: string[],
    targetAgents: string[],
  ): Promise<InstallResult[]> {
    return transport("install_new_repo_skills", {
      url,
      skillIds,
      targetAgents,
    });
  },

  updateTags(id: string, tags: string[]): Promise<void> {
    return transport("update_tags", { id, tags });
  },

  getAllTags(): Promise<string[]> {
    return transport("get_all_tags");
  },

  updatePack(id: string, pack: string | null): Promise<void> {
    return transport("update_pack", { id, pack });
  },

  batchUpdateTags(ids: string[], tags: string[]): Promise<void> {
    return transport("batch_update_tags", { ids, tags });
  },

  batchUpdatePack(ids: string[], pack: string | null): Promise<void> {
    return transport("batch_update_pack", { ids, pack });
  },

  getAllPacks(): Promise<string[]> {
    return transport("get_all_packs");
  },

  toggleByPack(pack: string, enabled: boolean): Promise<string[]> {
    return transport("toggle_by_pack", { pack, enabled });
  },

  searchMarketplace(
    query: string,
    kind: string,
    limit?: number,
  ): Promise<MarketplaceItem[]> {
    return transport("search_marketplace", { query, kind, limit });
  },

  trendingMarketplace(
    kind: string,
    limit?: number,
  ): Promise<MarketplaceItem[]> {
    return transport("trending_marketplace", { kind, limit });
  },

  fetchSkillPreview(
    source: string,
    skillId: string,
    gitUrl?: string | null,
  ): Promise<string> {
    return transport("fetch_skill_preview", {
      source,
      skillId,
      gitUrl: gitUrl ?? null,
    });
  },

  fetchCliReadme(source: string): Promise<string> {
    return transport("fetch_cli_readme", { source });
  },

  fetchSkillAudit(
    source: string,
    skillId: string,
  ): Promise<SkillAuditInfo | null> {
    return transport("fetch_skill_audit", { source, skillId });
  },

  installFromMarketplace(
    source: string,
    skillId: string,
    targetAgent?: string,
  ): Promise<InstallResult> {
    return transport("install_from_marketplace", { source, skillId, targetAgent });
  },

  installToAgent(extensionId: string, targetAgent: string): Promise<string> {
    return transport("install_to_agent", { extensionId, targetAgent });
  },

  listProjects(): Promise<Project[]> {
    return transport("list_projects");
  },

  addProject(path: string): Promise<Project> {
    return transport("add_project", { path });
  },

  removeProject(id: string): Promise<void> {
    return transport("remove_project", { id });
  },

  discoverProjects(rootPath: string): Promise<DiscoveredProject[]> {
    return transport("discover_projects", { rootPath });
  },

  updateAgentPath(name: string, path: string | null): Promise<void> {
    return transport("update_agent_path", { name, path });
  },

  setAgentEnabled(name: string, enabled: boolean): Promise<void> {
    return transport("set_agent_enabled", { name, enabled });
  },

  listSkillFiles(path: string): Promise<FileEntry[]> {
    return transport("list_skill_files", { path });
  },

  openInSystem(path: string): Promise<void> {
    return transport("open_in_system", { path });
  },

  revealInFileManager(path: string): Promise<void> {
    return transport("reveal_in_file_manager", { path });
  },

  listAgentConfigs(): Promise<AgentDetail[]> {
    return transport("list_agent_configs");
  },

  readConfigFilePreview(path: string, maxLines?: number): Promise<string> {
    return transport("read_config_file_preview", { path, maxLines });
  },

  addCustomConfigPath(
    agent: string,
    path: string,
    label: string,
    category: string,
  ): Promise<number> {
    return transport("add_custom_config_path", { agent, path, label, category });
  },

  updateCustomConfigPath(
    id: number,
    path: string,
    label: string,
    category: string,
  ): Promise<void> {
    return transport("update_custom_config_path", { id, path, label, category });
  },

  removeCustomConfigPath(id: number): Promise<void> {
    return transport("remove_custom_config_path", { id });
  },

  updateAgentOrder(names: string[]): Promise<void> {
    return transport("update_agent_order", { names });
  },

  getCliWithChildren(cliId: string): Promise<[Extension, Extension[]]> {
    return transport("get_cli_with_children", { cliId });
  },

  listCliMarketplace(): Promise<MarketplaceItem[]> {
    return transport("list_cli_marketplace");
  },

  installCli(binaryName: string, targetAgents: string[]): Promise<void> {
    return transport("install_cli", { binaryName, targetAgents });
  },

  setAppIcon(name: string): Promise<void> {
    return transport("set_app_icon", { name });
  },
};
