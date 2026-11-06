export const EXTENSION_KINDS = ["skill", "mcp", "plugin", "hook", "cli"] as const;
export type ExtensionKind = (typeof EXTENSION_KINDS)[number];

export interface Agent { id: string; name: string; supports: readonly ExtensionKind[] }

const ALL = EXTENSION_KINDS;

// Order = AGENT_ORDER in src/lib/types.ts; matrix = README "Full Suite Extension Management" table.
export const AGENTS: readonly Agent[] = [
  { id: "claude", name: "Claude Code", supports: ALL },
  { id: "codex", name: "Codex", supports: ALL },
  { id: "gemini", name: "Gemini CLI", supports: ALL },
  { id: "cursor", name: "Cursor", supports: ALL },
  { id: "antigravity", name: "Antigravity", supports: ["skill", "mcp", "cli"] },
  { id: "copilot", name: "Copilot", supports: ALL },
  { id: "windsurf", name: "Devin Desktop", supports: ["skill", "mcp", "hook", "cli"] },
  { id: "opencode", name: "OpenCode", supports: ["skill", "mcp", "plugin", "cli"] },
  { id: "hermes", name: "Hermes", supports: ALL },
  { id: "kiro", name: "Kiro", supports: ["skill", "mcp", "hook", "cli"] },
  { id: "omp", name: "Oh My Pi", supports: ["skill", "mcp", "plugin", "cli"] },
  { id: "dsh", name: "DeepSeek Harness", supports: ["skill", "mcp", "plugin", "cli"] },
  { id: "grok", name: "Grok Build", supports: ALL },
  { id: "qoder-cn", name: "Qoder CN", supports: ALL },
  { id: "openclaw", name: "OpenClaw", supports: ["skill", "mcp", "cli"] },
  { id: "penguin", name: "PenguinHarness", supports: ["skill"] },
  { id: "pi", name: "Pi", supports: ["skill", "mcp", "plugin", "cli"] },
];

export const AGENT_COUNT = AGENTS.length;
export const AUDIT_RULE_COUNT = 20; // crates/hk-core/src/auditor/rules.rs all_rules()
export const REPO = "RealZST/HarnessKit";
export const INSTALL_SH = "curl -fsSL https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.sh | sh";
export const INSTALL_PS1 = "irm https://raw.githubusercontent.com/RealZST/HarnessKit/main/install.ps1 | iex";
export const INSTALL_SCRIPT_URL = "https://github.com/RealZST/HarnessKit/blob/main/install.sh";
export const INSTALL_PS1_SCRIPT_URL = "https://github.com/RealZST/HarnessKit/blob/main/install.ps1";
export const RELEASES_URL = "https://github.com/RealZST/HarnessKit/releases";
