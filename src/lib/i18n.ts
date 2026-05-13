export type Language = "en" | "zh";

export const LANGUAGE_OPTIONS: { value: Language; label: string }[] = [
  { value: "en", label: "English" },
  { value: "zh", label: "中文" },
];

const translations = {
  en: {
    // Settings page
    "settings.title": "Settings",
    "settings.agentPaths": "Agent Paths",
    "settings.agentPaths.desc":
      "Auto-detected paths shown below. Click the edit button to choose a custom path.",
    "settings.projectPaths": "Project Paths",
    "settings.projectPaths.desc":
      "Add project directories to scan their local extensions (.claude/skills, .mcp.json, hooks).",
    "settings.appearance": "Appearance",
    "settings.language": "Language",
    "settings.theme": "Theme",
    "settings.mode": "Mode",
    "settings.appIcon": "App Icon",
    "settings.noProjects": "No projects yet",
    "settings.noProjects.desc":
      "Add a project directory to scan for local extensions.",
    "settings.add": "Add",
    "settings.cancel": "Cancel",
    "settings.save": "Save",
    "settings.enabled": "Enabled",
    "settings.disabled": "Disabled",
    "settings.loading": "Loading...",
    "settings.missing": "Missing",
    "settings.notDetected": "Not detected",
    "settings.browsePlaceholder": "Paste a project path or browse...",
    "settings.pastePlaceholder": "Paste a project path...",
    "settings.projectRemoved": "Project removed",
    "settings.projectAdded": "Project added",
    "settings.noProjectsFound": "No projects found in directory",
    "settings.discoverFailed": "Failed to discover projects",
    "settings.discoveredIntro":
      "The selected directory is not a project. Found {count} project(s) inside:",
    "settings.discoveredNone": "No projects found.",
    "settings.addSelected": "Add Selected",
    "settings.checkUpdates": "Check for Updates",
    "settings.checking": "Checking...",
    "settings.upToDate": "You're up to date",
    "settings.updating": "Updating...",
    "settings.updateTo": "Update to",
    "settings.iconFailed": "Failed to set icon",
    "settings.footer": "One home for every agent",

    // Mode options
    "mode.system": "System",
    "mode.light": "Light",
    "mode.dark": "Dark",

    // Sidebar / Navigation
    "nav.overview": "Overview",
    "nav.agents": "Agents",
    "nav.extensions": "Extensions",
    "nav.audit": "Audit",
    "nav.marketplace": "Marketplace",
    "nav.settings": "Settings",

    // Overview page
    "overview.welcome": "Welcome to HarnessKit",
    "overview.getStarted":
      "Get started by browsing the marketplace or running a scan.",
    "overview.tipOfTheDay": "Tip of the day",
    "overview.agentActivity": "Agent activity",
    "overview.recentlyInstalled": "Recently installed",
    "overview.noRecentConfig": "No recent config changes",
    "overview.noRecentInstall": "No recent installations",
    "overview.onePlace": "One place for all your extensions",
    "overview.viewExtensions": "View extensions",
    "overview.viewExtensions.desc":
      "Browse and manage extensions across your coding agents",
    "overview.browseMarketplace": "Browse marketplace",
    "overview.browseMarketplace.desc":
      "Discover and install skills, MCP servers, and plugins",
    "overview.runAudit": "Run audit",
    "overview.runAudit.desc": "Check your extensions for security issues",
    "overview.workspaceReady": "Your workspace is ready",
    "overview.workspaceReady.desc":
      "Browse the marketplace to discover skills, MCP servers, and agent-first CLIs.",
    "overview.quickActions": "Quick actions",
    "overview.viewAgents": "View Agents",
    "overview.viewAgents.sub": "Manage agent configs",
    "overview.runAuditAction": "Run Audit",
    "overview.runAuditAction.sub": "Scan for security issues",
    "overview.checkUpdates": "Check Updates",
    "overview.checkUpdates.sub": "Check for extension updates",
    "overview.marketplace": "Marketplace",
    "overview.marketplace.sub": "Discover skills, CLI and MCP",
    "overview.updatesAvailable": "{count} update(s) available",
    "overview.noUpdates": "No updates available",

    // Extensions page
    "extensions.title": "Extensions",
    "extensions.installNew": "Install New",
    "extensions.checkUpdates": "Check Updates",
    "extensions.checking": "Checking...",
    "extensions.updateAll": "Update All",
    "extensions.updating": "Updating...",
    "extensions.moreFromRepos": "{count} More from Repos",
    "extensions.selected": "{count} selected",
    "extensions.enable": "Enable",
    "extensions.disable": "Disable",
    "extensions.enabled": "{count} extension(s) enabled",
    "extensions.disabled": "{count} extension(s) disabled",
    "extensions.updated": "{count} extension(s) updated",
    "extensions.updatesAvailable": "{count} update(s) available",
    "extensions.noUpdates": "No updates available",
    "extensions.installed": "{count} skill(s) installed",

    // Audit page
    "audit.title": "Security Audit",
    "audit.runAudit": "Run Audit",
    "audit.auditing": "Auditing...",
    "audit.scanned": "{count} extensions scanned",
    "audit.lastRun": "Last run",
    "audit.ago": "ago",
    "audit.justNow": "Just now",
    "audit.trustScoreDesc":
      "Trust scores (0–100) reflect {count} security checks. 80+ is safe, 60–79 is low risk, below 60 needs review.",
    "audit.disclaimer":
      "Automated heuristic checks — not a substitute for professional security review.",
    "audit.searchPlaceholder": "Search extensions...",
    "audit.allTiers": "All Trust Tiers",
    "audit.safe": "Safe",
    "audit.lowRisk": "Low Risk",
    "audit.needsReview": "Needs Review",
    "audit.results": "{count} results",
    "audit.clearFilters": "Clear filters",
    "audit.running": "Running security audit...",
    "audit.running.desc": "Scanning your extensions for security issues.",
    "audit.ready": "Ready to audit",
    "audit.ready.desc":
      "Scan your extensions for vulnerabilities, dangerous commands, and trust scores.",
    "audit.noFindings": "No audit findings in {scope}",
    "audit.noFindings.desc": "Nothing is installed in this scope yet.",
    "audit.noMatch": "No extensions match your filters.",
    "audit.clean": "Clean",
    "audit.finding": "finding",
    "audit.findings": "findings",
    "audit.pass": "Pass",
    "audit.showFailuresOnly": "Show failures only",
    "audit.showAllRules": "Show all {count} rules ({passed} passed)",
    "audit.viewExtension": "View extension",

    // Marketplace page
    "marketplace.title": "Marketplace",
    "marketplace.installFromGit": "Install from Git",
    "marketplace.installFromLocal": "Install from Local",
    "marketplace.searchSkills": "Search skills...",
    "marketplace.searchMCP": "Search MCP servers...",
    "marketplace.searchCLI": "Search Agent-first CLIs...",
    "marketplace.hint":
      "Search for skills, MCP servers, and Agent-first CLIs to install across your Agents. Use 'Install from Git' to install from a Git URL, or 'Install from Local' to install from a local directory.",
    "marketplace.trending": "Trending",
    "marketplace.trendingSkills": "Trending Skills",
    "marketplace.trendingMCP": "Trending MCP Servers",
    "marketplace.trendingCLI": "Trending Agent-first CLI",
    "marketplace.noMatch": 'Nothing matched "{query}"',
    "marketplace.noMatch.desc":
      "Try different keywords or browse the trending items below.",
    "marketplace.installMCP": "Install this MCP server",
    "marketplace.installMCP.desc":
      "Visit Smithery for setup instructions, configuration options, and connection details.",
    "marketplace.setupOnSmithery": "Set up on Smithery",
    "marketplace.viewOnGitHub": "View on GitHub",
    "marketplace.installGuide": "Installation Guide",
    "marketplace.noReadme":
      "No README available. Check the GitHub repository for installation instructions.",
    "marketplace.securityAudit": "Security Audit",
    "marketplace.noAuditData": "No audit data available",
    "marketplace.installToAgent": "Install to Agent",
    "marketplace.closeDetails": "Close details",
    "marketplace.installBtn": "Install",
    "marketplace.installed": "Installed",
    "marketplace.notSupported": "Not supported",
    "marketplace.selectScope": "Select a scope to install",
    "marketplace.installFailed": "Installation failed",

    // Agents page
    "agents.loading": "Loading...",

    // Common
    "common.cancel": "Cancel",
    "common.loading": "Loading...",

    // Toast messages
    "toast.theme": "Theme: {name}",
    "toast.mode": "Mode: {name}",
    "toast.icon": "Icon: {name}",
    "toast.language": "Language: {name}",
  },
  zh: {
    // Settings page
    "settings.title": "设置",
    "settings.agentPaths": "Agent 路径",
    "settings.agentPaths.desc":
      "下方显示自动检测的路径。点击编辑按钮可选择自定义路径。",
    "settings.projectPaths": "项目路径",
    "settings.projectPaths.desc":
      "添加项目目录以扫描本地扩展（.claude/skills、.mcp.json、hooks）。",
    "settings.appearance": "外观",
    "settings.language": "语言",
    "settings.theme": "主题",
    "settings.mode": "模式",
    "settings.appIcon": "应用图标",
    "settings.noProjects": "暂无项目",
    "settings.noProjects.desc": "添加项目目录以扫描本地扩展。",
    "settings.add": "添加",
    "settings.cancel": "取消",
    "settings.save": "保存",
    "settings.enabled": "已启用",
    "settings.disabled": "已禁用",
    "settings.loading": "加载中...",
    "settings.missing": "缺失",
    "settings.notDetected": "未检测到",
    "settings.browsePlaceholder": "粘贴项目路径或浏览...",
    "settings.pastePlaceholder": "粘贴项目路径...",
    "settings.projectRemoved": "项目已移除",
    "settings.projectAdded": "项目已添加",
    "settings.noProjectsFound": "目录中未找到项目",
    "settings.discoverFailed": "发现项目失败",
    "settings.discoveredIntro":
      "所选目录不是项目。在其中发现了 {count} 个项目：",
    "settings.discoveredNone": "未找到项目。",
    "settings.addSelected": "添加所选",
    "settings.checkUpdates": "检查更新",
    "settings.checking": "检查中...",
    "settings.upToDate": "已是最新版本",
    "settings.updating": "更新中...",
    "settings.updateTo": "更新到",
    "settings.iconFailed": "设置图标失败",
    "settings.footer": "所有 Agent，一个家",

    // Mode options
    "mode.system": "跟随系统",
    "mode.light": "浅色",
    "mode.dark": "深色",

    // Sidebar / Navigation
    "nav.overview": "概览",
    "nav.agents": "Agents",
    "nav.extensions": "扩展",
    "nav.audit": "安全审计",
    "nav.marketplace": "市场",
    "nav.settings": "设置",

    // Overview page
    "overview.welcome": "欢迎使用 HarnessKit",
    "overview.getStarted": "浏览市场或运行扫描以开始使用。",
    "overview.tipOfTheDay": "每日提示",
    "overview.agentActivity": "Agent 活动",
    "overview.recentlyInstalled": "最近安装",
    "overview.noRecentConfig": "暂无最近的配置变更",
    "overview.noRecentInstall": "暂无最近的安装记录",
    "overview.onePlace": "所有扩展，集中管理",
    "overview.viewExtensions": "查看扩展",
    "overview.viewExtensions.desc": "浏览和管理各 Agent 的扩展",
    "overview.browseMarketplace": "浏览市场",
    "overview.browseMarketplace.desc":
      "发现并安装 Skills、MCP Servers 和 Plugins",
    "overview.runAudit": "运行审计",
    "overview.runAudit.desc": "检查扩展的安全问题",
    "overview.workspaceReady": "工作区已就绪",
    "overview.workspaceReady.desc":
      "浏览市场以发现 Skills、MCP Servers 和 Agent-first CLIs。",
    "overview.quickActions": "快捷操作",
    "overview.viewAgents": "查看 Agents",
    "overview.viewAgents.sub": "管理 Agent 配置",
    "overview.runAuditAction": "运行审计",
    "overview.runAuditAction.sub": "扫描安全问题",
    "overview.checkUpdates": "检查更新",
    "overview.checkUpdates.sub": "检查扩展更新",
    "overview.marketplace": "市场",
    "overview.marketplace.sub": "发现 Skills、CLI 和 MCP",
    "overview.updatesAvailable": "{count} 个更新可用",
    "overview.noUpdates": "没有可用更新",

    // Extensions page
    "extensions.title": "扩展",
    "extensions.installNew": "安装新扩展",
    "extensions.checkUpdates": "检查更新",
    "extensions.checking": "检查中...",
    "extensions.updateAll": "全部更新",
    "extensions.updating": "更新中...",
    "extensions.moreFromRepos": "来自仓库的 {count} 个更多",
    "extensions.selected": "已选 {count} 个",
    "extensions.enable": "启用",
    "extensions.disable": "禁用",
    "extensions.enabled": "已启用 {count} 个扩展",
    "extensions.disabled": "已禁用 {count} 个扩展",
    "extensions.updated": "已更新 {count} 个扩展",
    "extensions.updatesAvailable": "{count} 个更新可用",
    "extensions.noUpdates": "没有可用更新",
    "extensions.installed": "已安装 {count} 个 Skill",

    // Audit page
    "audit.title": "安全审计",
    "audit.runAudit": "运行审计",
    "audit.auditing": "审计中...",
    "audit.scanned": "已扫描 {count} 个扩展",
    "audit.lastRun": "上次运行",
    "audit.ago": "前",
    "audit.justNow": "刚刚",
    "audit.trustScoreDesc":
      "Trust Score（0–100）基于 {count} 项安全检查。80+ 为安全，60–79 为低风险，60 以下需要审查。",
    "audit.disclaimer": "自动化启发式检查——不能替代专业安全审查。",
    "audit.searchPlaceholder": "搜索扩展...",
    "audit.allTiers": "所有信任等级",
    "audit.safe": "安全",
    "audit.lowRisk": "低风险",
    "audit.needsReview": "需要审查",
    "audit.results": "{count} 个结果",
    "audit.clearFilters": "清除筛选",
    "audit.running": "正在运行安全审计...",
    "audit.running.desc": "正在扫描扩展的安全问题。",
    "audit.ready": "准备审计",
    "audit.ready.desc": "扫描扩展的漏洞、危险命令和 Trust Score。",
    "audit.noFindings": "{scope} 中没有审计发现",
    "audit.noFindings.desc": "此范围内尚未安装任何内容。",
    "audit.noMatch": "没有扩展匹配你的筛选条件。",
    "audit.clean": "无问题",
    "audit.finding": "个发现",
    "audit.findings": "个发现",
    "audit.pass": "通过",
    "audit.showFailuresOnly": "仅显示失败项",
    "audit.showAllRules": "显示全部 {count} 条规则（{passed} 条通过）",
    "audit.viewExtension": "查看扩展",

    // Marketplace page
    "marketplace.title": "市场",
    "marketplace.installFromGit": "从 Git 安装",
    "marketplace.installFromLocal": "从本地安装",
    "marketplace.searchSkills": "搜索 Skills...",
    "marketplace.searchMCP": "搜索 MCP Servers...",
    "marketplace.searchCLI": "搜索 Agent-first CLIs...",
    "marketplace.hint":
      "搜索 Skills、MCP Servers 和 Agent-first CLIs 以安装到你的 Agents。使用「从 Git 安装」通过 Git URL 安装，或使用「从本地安装」从本地目录安装。",
    "marketplace.trending": "热门",
    "marketplace.trendingSkills": "热门 Skills",
    "marketplace.trendingMCP": "热门 MCP Servers",
    "marketplace.trendingCLI": "热门 Agent-first CLI",
    "marketplace.noMatch": "没有匹配「{query}」的结果",
    "marketplace.noMatch.desc": "尝试不同的关键词或浏览下方的热门项目。",
    "marketplace.installMCP": "安装此 MCP Server",
    "marketplace.installMCP.desc":
      "访问 Smithery 获取设置说明、配置选项和连接详情。",
    "marketplace.setupOnSmithery": "在 Smithery 上设置",
    "marketplace.viewOnGitHub": "在 GitHub 上查看",
    "marketplace.installGuide": "安装指南",
    "marketplace.noReadme": "没有可用的 README。请查看 GitHub 仓库获取安装说明。",
    "marketplace.securityAudit": "安全审计",
    "marketplace.noAuditData": "没有可用的审计数据",
    "marketplace.installToAgent": "安装到 Agent",
    "marketplace.closeDetails": "关闭详情",
    "marketplace.installBtn": "安装",
    "marketplace.installed": "已安装",
    "marketplace.notSupported": "不支持",
    "marketplace.selectScope": "选择安装范围",
    "marketplace.installFailed": "安装失败",

    // Agents page
    "agents.loading": "加载中...",

    // Common
    "common.cancel": "取消",
    "common.loading": "加载中...",

    // Toast messages
    "toast.theme": "主题：{name}",
    "toast.mode": "模式：{name}",
    "toast.icon": "图标：{name}",
    "toast.language": "语言：{name}",
  },
} as const;

type TranslationKey = keyof (typeof translations)["en"];

// Allow both strict keys and dynamic string keys (for template literals)
type TranslationKeyInput = TranslationKey | (string & {});

let currentLanguage: Language = "en";

export function setCurrentLanguage(lang: Language) {
  currentLanguage = lang;
}

export function getCurrentLanguage(): Language {
  return currentLanguage;
}

/**
 * Get a translated string by key, with optional interpolation.
 * Usage: t("settings.discoveredIntro", { count: "3" })
 */
export function t(
  key: TranslationKeyInput,
  params?: Record<string, string>,
): string {
  const value: string =
    (translations[currentLanguage] as Record<string, string>)?.[key] ??
    (translations.en as Record<string, string>)[key] ??
    key;
  if (!params) return value;
  let result = value;
  for (const [k, v] of Object.entries(params)) {
    result = result.replace(`{${k}}`, v);
  }
  return result;
}
