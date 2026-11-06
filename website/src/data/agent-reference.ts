import { AGENTS } from "./agents";

// Where each agent keeps its files, as HarnessKit's adapters read them. One record per agent, in
// AGENTS order. Every value was checked against crates/hk-core/src/adapter/<agent>.rs (2026-11-05).
// Paths: "~/" is the home folder, "<project>/" a registered project, and a trailing "/" a folder.
// Defaults only: environment overrides such as GROK_HOME are named in `notes`.

/** The file format HarnessKit parses. JSONC is JSON with comments; JSON5 adds unquoted keys and more. */
export type ConfigFormat = "JSON" | "JSONC" | "JSON5" | "TOML" | "YAML";

export interface McpLocation {
  /** The user-level file HarnessKit reads and writes. */
  file: string;
  format: ConfigFormat;
  /** Where the servers sit in the file. Absent when they are not under one key (DeepSeek Harness). */
  key?: string;
  /** The project-level file. Absent when HarnessKit has no project MCP config for the agent. */
  projectFile?: string;
}

export interface HookLocation {
  /** The user-level file HarnessKit adds hooks to. */
  file: string;
  /** The project-level file. Absent when HarnessKit has no project hooks for the agent. */
  projectFile?: string;
  /** File layout, named after the agent that defines it (crates/hk-core/src/adapter/mod.rs HookFormat). */
  nativeFormat: string;
  /** Whether the file uses Claude Code's event names or the agent's own. */
  eventNames: "Claude Code" | "own";
}

export interface AgentReference {
  id: string;
  /** The adapter's base folder. */
  homeDir: string;
  /** Skill folders HarnessKit scans. The first of each list is where it installs. */
  skillDirs: { global: readonly string[]; project: readonly string[] };
  /** HarnessKit lists skills in ~/.agents/skills/ under this agent. */
  readsSharedAgentsSkills: boolean;
  /** null: HarnessKit does not manage MCP servers for this agent. */
  mcp: McpLocation | null;
  /** Rule and instruction files, global then project. A set, not a precedence order. */
  rulesFiles: readonly string[];
  /** null: HarnessKit does not manage hooks for this agent. */
  hooks: HookLocation | null;
  /**
   * The agent's own off switch, used when HarnessKit disables a skill, an MCP server or a hook. A skill
   * without one is turned off by renaming SKILL.md to SKILL.md.disabled, or a single-file skill's
   * <name>.md to <name>.md.disabled (manager.rs toggle_skill). An MCP server or hook without one is
   * removed from the agent's file and kept as a snapshot in HarnessKit's database.
   */
  nativeDisable: { skill?: string; mcp?: string; hook?: string };
  /** Short facts that do not fit a field. Plain sentences; backticks mark code (src/lib/inline-code.ts). */
  notes?: readonly string[];
}

export const SHARED_SKILLS_DIR = "~/.agents/skills/";

/** What disabling a skill does when an agent has no skill off switch of its own. */
export const SKILL_DISABLE = "Renames `SKILL.md` to `SKILL.md.disabled`.";

/** What HarnessKit does on disable when an agent has no off switch of its own. */
export const SNAPSHOT_DISABLE = "Removes the entry and keeps a snapshot in HarnessKit's database.";

export const AGENT_REFERENCE: readonly AgentReference[] = [
  {
    id: "claude",
    homeDir: "~/.claude/",
    skillDirs: { global: ["~/.claude/skills/"], project: ["<project>/.claude/skills/"] },
    readsSharedAgentsSkills: false,
    mcp: { file: "~/.claude.json", format: "JSON", key: "mcpServers", projectFile: "<project>/.mcp.json" },
    rulesFiles: [
      "~/.claude/CLAUDE.md",
      "~/.claude/rules/",
      "<project>/CLAUDE.md",
      "<project>/.claude/CLAUDE.md",
      "<project>/.claude/rules/",
    ],
    hooks: {
      file: "~/.claude/settings.json",
      projectFile: "<project>/.claude/settings.json",
      nativeFormat: "Claude Code JSON",
      eventNames: "Claude Code",
    },
    nativeDisable: {},
    notes: [
      "HarnessKit reads only the top-level `mcpServers` in `~/.claude.json`. `claude mcp add` saves to its default local scope, under `projects` in the same file, and HarnessKit doesn't list those servers yet.",
    ],
  },
  {
    id: "codex",
    homeDir: "~/.codex/",
    skillDirs: { global: ["~/.agents/skills/", "~/.codex/skills/"], project: ["<project>/.agents/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: {
      file: "~/.codex/config.toml",
      format: "TOML",
      key: "mcp_servers",
      projectFile: "<project>/.codex/config.toml",
    },
    rulesFiles: [
      "~/.codex/AGENTS.md",
      "~/.codex/AGENTS.override.md",
      "~/.codex/TEAM_GUIDE.md",
      "~/.codex/.agents.md",
      "<project>/AGENTS.md",
      "<project>/AGENTS.override.md",
      "<project>/TEAM_GUIDE.md",
      "<project>/.agents.md",
    ],
    hooks: {
      file: "~/.codex/hooks.json",
      projectFile: "<project>/.codex/hooks.json",
      nativeFormat: "Claude Code JSON",
      eventNames: "Claude Code",
    },
    nativeDisable: {},
    notes: [
      "Codex is the one agent whose global skills go into the shared `~/.agents/skills/` folder.",
      "If `config.toml` sets `project_doc_fallback_filenames`, HarnessKit looks for those file names in place of the four rules names above.",
      "HarnessKit doesn't read Codex's own skill switch, `[[skills.config]]` in `config.toml`, so a skill turned off there still shows as on.",
    ],
  },
  {
    id: "gemini",
    homeDir: "~/.gemini/",
    skillDirs: { global: ["~/.gemini/skills/", "~/.agents/skills/"], project: ["<project>/.gemini/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: {
      file: "~/.gemini/settings.json",
      format: "JSON",
      key: "mcpServers",
      projectFile: "<project>/.gemini/settings.json",
    },
    rulesFiles: ["~/.gemini/GEMINI.md", "<project>/GEMINI.md"],
    hooks: { file: "~/.gemini/settings.json", nativeFormat: "Claude Code JSON", eventNames: "own" },
    nativeDisable: {},
    notes: ["HarnessKit also lists `GEMINI.md` files up to two folders deep inside a project."],
  },
  {
    id: "cursor",
    homeDir: "~/.cursor/",
    skillDirs: { global: ["~/.cursor/skills/", "~/.agents/skills/"], project: ["<project>/.cursor/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.cursor/mcp.json", format: "JSON", key: "mcpServers", projectFile: "<project>/.cursor/mcp.json" },
    rulesFiles: ["<project>/.cursor/rules/", "<project>/.cursorrules", "<project>/AGENTS.md"],
    hooks: {
      file: "~/.cursor/hooks.json",
      projectFile: "<project>/.cursor/hooks.json",
      nativeFormat: "Cursor JSON",
      eventNames: "own",
    },
    nativeDisable: {},
    notes: [
      "Cursor has rules only at project level in HarnessKit.",
      "In `.cursor/rules/`, HarnessKit lists `.mdc` files, subfolders included.",
    ],
  },
  {
    id: "antigravity",
    homeDir: "~/.gemini/antigravity/",
    skillDirs: {
      global: ["~/.gemini/antigravity/skills/"],
      project: ["<project>/.agents/skills/", "<project>/.agent/skills/"],
    },
    readsSharedAgentsSkills: false,
    mcp: { file: "~/.gemini/antigravity/mcp_config.json", format: "JSON", key: "mcpServers" },
    rulesFiles: ["~/.gemini/GEMINI.md", "<project>/.agents/rules/", "<project>/.agent/rules/"],
    hooks: null,
    nativeDisable: {},
    notes: [
      "Folders listed in `~/.gemini/antigravity/skills.txt` are scanned for skills too.",
      "Antigravity and Gemini CLI both read `~/.gemini/GEMINI.md`.",
    ],
  },
  {
    id: "copilot",
    homeDir: "~/.copilot/",
    skillDirs: { global: ["~/.copilot/skills/", "~/.agents/skills/"], project: ["<project>/.github/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: {
      file: "~/Library/Application Support/Code/User/mcp.json",
      format: "JSON",
      key: "servers",
      projectFile: "<project>/.vscode/mcp.json",
    },
    rulesFiles: [
      "~/.copilot/copilot-instructions.md",
      "~/.copilot/instructions/",
      "<project>/.github/copilot-instructions.md",
      "<project>/.github/instructions/",
      "<project>/AGENTS.md",
    ],
    hooks: { file: "~/.copilot/hooks/hooks.json", nativeFormat: "Copilot JSON", eventNames: "Claude Code" },
    nativeDisable: {},
    notes: [
      "The MCP file is VS Code's user `mcp.json`. The path above is the macOS one. On Linux it is `~/.config/Code/User/mcp.json`, and on Windows `~/AppData/Roaming/Code/User/mcp.json`.",
      "HarnessKit reads every `.json` file in `~/.copilot/hooks/` and adds new hooks to `hooks.json`.",
      "In the `instructions/` folders, HarnessKit lists files ending in `.instructions.md`, subfolders included.",
      "HarnessKit doesn't read Copilot CLI's `~/.copilot/mcp-config.json` or a `.mcp.json` at the project root yet.",
    ],
  },
  {
    id: "windsurf",
    homeDir: "~/.codeium/windsurf/",
    skillDirs: {
      global: ["~/.codeium/windsurf/skills/", "~/.agents/skills/"],
      project: ["<project>/.devin/skills/", "<project>/.windsurf/skills/"],
    },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.codeium/windsurf/mcp_config.json", format: "JSON", key: "mcpServers" },
    rulesFiles: [
      "~/.codeium/windsurf/memories/global_rules.md",
      "~/.codeium/windsurf/global_rules.md",
      "<project>/.devin/rules/",
      "<project>/.windsurfrules",
      "<project>/.windsurf/rules/",
    ],
    hooks: {
      file: "~/.codeium/windsurf/hooks.json",
      projectFile: "<project>/.windsurf/hooks.json",
      nativeFormat: "Devin Desktop JSON",
      eventNames: "own",
    },
    nativeDisable: {},
    notes: [
      "Devin Desktop was called Windsurf, so its global folder and some project files still carry that name.",
      "`~/.codeium/windsurf/global_rules.md` is an older location that HarnessKit still lists.",
      "HarnessKit manages Devin Desktop's Cascade agent. Devin CLI and Devin Local keep their settings in `~/.config/devin/`, which HarnessKit doesn't support yet.",
    ],
  },
  {
    id: "opencode",
    homeDir: "~/.config/opencode/",
    skillDirs: {
      global: ["~/.config/opencode/skills/", "~/.agents/skills/"],
      project: ["<project>/.opencode/skills/"],
    },
    readsSharedAgentsSkills: true,
    mcp: {
      file: "~/.config/opencode/opencode.json",
      format: "JSONC",
      key: "mcp",
      projectFile: "<project>/opencode.json",
    },
    rulesFiles: ["~/.config/opencode/AGENTS.md", "<project>/AGENTS.md"],
    hooks: null,
    nativeDisable: {},
    notes: [
      "When `opencode.json` and `opencode.jsonc` both exist, HarnessKit reads MCP servers from both, and the `.jsonc` entry wins on a shared name, as in OpenCode. It writes to `opencode.jsonc` when that file exists and to `opencode.json` otherwise, globally and in projects.",
    ],
  },
  {
    id: "hermes",
    homeDir: "~/.hermes/",
    skillDirs: { global: ["~/.hermes/skills/local/", "~/.hermes/skills/"], project: [] },
    readsSharedAgentsSkills: false,
    mcp: { file: "~/.hermes/config.yaml", format: "YAML", key: "mcp_servers" },
    rulesFiles: ["~/.hermes/SOUL.md"],
    hooks: { file: "~/.hermes/config.yaml", nativeFormat: "Hermes YAML", eventNames: "own" },
    nativeDisable: {
      skill: "Adds the skill's name to `skills.disabled` in `~/.hermes/config.yaml`.",
      mcp: "Sets `enabled: false` on the server.",
    },
    notes: [
      "HarnessKit lists every `SKILL.md` under `~/.hermes/skills/`, in subfolders too. Category folders there only organize skills. When you install one skill, HarnessKit asks which category to put it in. A bulk install puts every skill in the first category in alphabetical order, or in `local/` when there is none.",
      "Folders listed under `skills.external_dirs` in `config.yaml` are scanned for skills too.",
      "Hermes has no project scope in HarnessKit. Hermes itself also loads skills from `<project>/.hermes/skills/` and `<project>/.agents/skills/` in projects you trust, and HarnessKit doesn't list those.",
    ],
  },
  {
    id: "kiro",
    homeDir: "~/.kiro/",
    skillDirs: { global: ["~/.kiro/skills/"], project: ["<project>/.kiro/skills/"] },
    readsSharedAgentsSkills: false,
    mcp: {
      file: "~/.kiro/settings/mcp.json",
      format: "JSON",
      key: "mcpServers",
      projectFile: "<project>/.kiro/settings/mcp.json",
    },
    rulesFiles: ["~/.kiro/steering/", "<project>/.kiro/steering/"],
    hooks: {
      file: "~/.kiro/hooks/harnesskit.json",
      projectFile: "<project>/.kiro/hooks/harnesskit.json",
      nativeFormat: "Kiro hook JSON",
      eventNames: "Claude Code",
    },
    nativeDisable: { mcp: "Sets `disabled: true` on the server.", hook: "Sets `enabled: false` on the hook." },
    notes: [
      "HarnessKit reads every `.json` file in `~/.kiro/hooks/` and `<project>/.kiro/hooks/`, and adds new hooks to `harnesskit.json`.",
    ],
  },
  {
    id: "omp",
    homeDir: "~/.omp/",
    skillDirs: { global: ["~/.omp/agent/skills/"], project: ["<project>/.omp/skills/"] },
    readsSharedAgentsSkills: false,
    mcp: { file: "~/.omp/agent/mcp.json", format: "JSON", key: "mcpServers", projectFile: "<project>/.omp/mcp.json" },
    rulesFiles: [
      "~/.omp/agent/AGENTS.md",
      "~/.omp/agent/RULES.md",
      "~/.omp/agent/rules/",
      "<project>/.omp/AGENTS.md",
      "<project>/.omp/RULES.md",
      "<project>/.omp/rules/",
    ],
    hooks: null,
    nativeDisable: {
      mcp: "Sets `enabled: false` on the server, and takes its name off `enabledServers` in `~/.omp/agent/mcp.json`.",
    },
    notes: [
      "HarnessKit reads the default profile only, not the ones under `~/.omp/profiles/`.",
      "Oh My Pi itself also reads `~/.agents/skills/` and `~/.claude/skills/`. HarnessKit does not list those folders under Oh My Pi.",
    ],
  },
  {
    id: "dsh",
    homeDir: "~/.dsh/",
    skillDirs: { global: ["~/.dsh/skills/", "~/.agents/skills/"], project: ["<project>/.dsh/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.dsh/cordis.patch.yml", format: "YAML" },
    rulesFiles: [
      "~/.dsh/AGENTS.md",
      "<project>/AGENTS.md",
      "<project>/CLAUDE.md",
      "<project>/AGENTS.local.md",
      "<project>/CLAUDE.local.md",
    ],
    hooks: null,
    nativeDisable: {
      mcp: "Writes `disabled: true` for the server in a block that HarnessKit manages at the end of `cordis.patch.yml`.",
    },
    notes: [
      "Each MCP server is a plugin row named `@deepseek-ai/dsh-mcp-client`. HarnessKit reads the home-level patch file only, not the per-profile ones.",
      "`DSH_HOME` moves the home folder, and `DSH_AGENTS_HOME` moves the shared `~/.agents/` folder.",
    ],
  },
  {
    id: "grok",
    homeDir: "~/.grok/",
    skillDirs: { global: ["~/.grok/skills/", "~/.agents/skills/"], project: ["<project>/.grok/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: {
      file: "~/.grok/config.toml",
      format: "TOML",
      key: "mcp_servers",
      projectFile: "<project>/.grok/config.toml",
    },
    rulesFiles: [
      "~/.grok/AGENTS.md",
      "~/.grok/rules/",
      "<project>/AGENTS.md",
      "<project>/Agents.md",
      "<project>/AGENT.md",
      "<project>/.grok/rules/",
    ],
    hooks: {
      file: "~/.grok/hooks/harnesskit.json",
      projectFile: "<project>/.grok/hooks/harnesskit.json",
      nativeFormat: "Claude Code JSON",
      eventNames: "Claude Code",
    },
    nativeDisable: {
      mcp: "Adds the server's name to `disabled_mcp_servers` in `~/.grok/config.toml`. A server defined in that file also gets `enabled = false`.",
      hook: "Adds the hook to `~/.grok/disabled-hooks`.",
    },
    notes: [
      "Skills in nested folders count too, not only the top level.",
      "HarnessKit reads every `.json` file in `~/.grok/hooks/` and `<project>/.grok/hooks/`, and adds new hooks to `harnesskit.json`.",
      "`GROK_HOME` moves the home folder.",
    ],
  },
  {
    id: "qoder-cn",
    homeDir: "~/.qoder-cn/",
    skillDirs: { global: ["~/.qoder-cn/skills/", "~/.agents/skills/"], project: ["<project>/.agents/skills/"] },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.qoder-cn/settings.json", format: "JSON", key: "mcpServers" },
    rulesFiles: [
      "~/.qoder-cn/AGENTS.md",
      "~/.qoder-cn/rules/",
      "<project>/AGENTS.md",
      "<project>/AGENTS.local.md",
      "<project>/.qoder/rules/",
    ],
    hooks: {
      file: "~/.qoder-cn/settings.json",
      projectFile: "<project>/.qoder/settings.json",
      nativeFormat: "Claude Code JSON",
      eventNames: "Claude Code",
    },
    nativeDisable: {},
    notes: [
      "HarnessKit installs project skills into the shared `<project>/.agents/skills/` folder, which it also lists under Codex, Antigravity and Pi. Skills you put in `<project>/.qoder/skills/` yourself aren't listed.",
      "HarnessKit doesn't manage Qoder CN's project-level MCP servers.",
      "HarnessKit doesn't read `QODERCN_CONFIG_DIR`, so it looks for Qoder CN in `~/.qoder-cn/` only.",
      "HarnessKit doesn't read Qoder CN's own `skills.disabled` list in `settings.json`, so a skill turned off there still shows as on.",
    ],
  },
  {
    id: "openclaw",
    homeDir: "~/.openclaw/",
    skillDirs: {
      global: [
        "~/.openclaw/skills/",
        "~/.openclaw/workspace/skills/",
        "~/.openclaw/workspace/.agents/skills/",
        "~/.agents/skills/",
      ],
      project: [],
    },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.openclaw/openclaw.json", format: "JSON5", key: "mcp.servers" },
    rulesFiles: [
      "~/.openclaw/workspace/AGENTS.md",
      "~/.openclaw/workspace/SOUL.md",
      "~/.openclaw/workspace/IDENTITY.md",
    ],
    hooks: null,
    nativeDisable: { mcp: "Sets `enabled: false` on the server." },
    notes: [
      "Each agent workspace named in `openclaw.json` has its own `skills/`, `.agents/skills/` and rules files. The paths above are for the default workspace.",
      "HarnessKit reads `~/.agents/skills/` for OpenClaw only while OpenClaw uses its default state folder, `~/.openclaw/`.",
      "OpenClaw has no project scope in HarnessKit.",
      "`OPENCLAW_STATE_DIR` moves the state folder, `~/.openclaw/`. `OPENCLAW_CONFIG_PATH` moves `openclaw.json`, and when `OPENCLAW_STATE_DIR` isn't set, the `skills/` folder HarnessKit installs into moves with it. `OPENCLAW_WORKSPACE_DIR` moves the default workspace when `openclaw.json` doesn't name one. HarnessKit doesn't read `OPENCLAW_PROFILE`.",
    ],
  },
  {
    id: "penguin",
    homeDir: "~/.penguin/data/",
    skillDirs: { global: ["~/.penguin/data/<penguin-project>/agents/<agent>/agent_state/skills/"], project: [] },
    readsSharedAgentsSkills: false,
    mcp: null,
    rulesFiles: ["~/.penguin/data/<penguin-project>/agents/<agent>/agent_state/AGENTS.md"],
    hooks: null,
    nativeDisable: {},
    notes: [
      "Every PenguinHarness agent has its own skills folder and `AGENTS.md`. HarnessKit reads all of them.",
      "HarnessKit installs a skill into one PenguinHarness agent's folder: the first in alphabetical order, by project and then by agent. With no PenguinHarness agent yet, there is nowhere to install.",
      "HarnessKit does not read PenguinHarness MCP servers or hooks yet.",
      "PenguinHarness has no project scope in HarnessKit.",
      "`PENGUIN_HOME` moves the data folder.",
    ],
  },
  {
    id: "pi",
    homeDir: "~/.pi/",
    skillDirs: {
      global: ["~/.pi/agent/skills/", "~/.agents/skills/"],
      project: ["<project>/.pi/skills/", "<project>/.agents/skills/"],
    },
    readsSharedAgentsSkills: true,
    mcp: { file: "~/.pi/agent/mcp.json", format: "JSON", key: "mcpServers", projectFile: "<project>/.pi/mcp.json" },
    rulesFiles: [
      "~/.pi/agent/AGENTS.md",
      "~/.pi/agent/AGENTS.override.md",
      "~/.pi/agent/AGENTS.MD",
      "~/.pi/agent/CLAUDE.md",
      "~/.pi/agent/CLAUDE.MD",
      "<project>/AGENTS.md",
      "<project>/AGENTS.override.md",
      "<project>/AGENTS.MD",
      "<project>/CLAUDE.md",
      "<project>/CLAUDE.MD",
    ],
    hooks: null,
    nativeDisable: { mcp: "Sets `enabled: false` on the server." },
    notes: [
      "`PI_CODING_AGENT_DIR` moves the agent folder, `~/.pi/agent/`.",
      "Pi loads skills from nested folders. HarnessKit lists only the skills directly inside each skills folder.",
      "HarnessKit doesn't read the `skills` filter in Pi's `settings.json`, so a skill excluded there still shows as on.",
    ],
  },
];

/** The record for one agent id. Throws on an unknown id so a typo in a page fails the build. */
export function referenceFor(id: string): AgentReference {
  const reference = AGENT_REFERENCE.find((entry) => entry.id === id);
  if (!reference) throw new Error(`No agent reference for "${id}"`);
  return reference;
}

/** The agent's display name, from agents.ts. */
export function agentNameFor(id: string): string {
  const agent = AGENTS.find((entry) => entry.id === id);
  if (!agent) throw new Error(`No agent named "${id}" in agents.ts`);
  return agent.name;
}

/** The id Starlight gives a "## <name>" heading (github-slugger, for names of letters, digits and spaces). */
export function anchorFor(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9 -]/g, "").replace(/ /g, "-");
}
