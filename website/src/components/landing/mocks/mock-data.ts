import type { ExtensionKind } from "../../../data/agents";

// The Extensions and Security Audit windows (landing/mocks) show the same four sample extensions, skills and MCP
// servers only: the two kinds the landing copy names. mocks.test.tsx checks every finding against the kinds and
// severities its rule can report, and every score against the App's formula. The Problem block (MOCK_GATHER below)
// shows two of them with the same agents and scores; gather-data.test.ts checks that.

export type MockPerm = "filesystem" | "network" | "shell" | "database";

export interface MockExtension {
  name: string;
  /** The kinds that install to another agent (the App's INSTALLABLE_KINDS) the window shows: a skill, MCP servers, a hook. */
  kind: Extract<ExtensionKind, "skill" | "mcp" | "hook">;
  /** agent ids; each must support `kind` (src/data/agents.ts) */
  agents: readonly string[];
  /**
   * The agent the Extensions window installs it to from its detail panel, in one row only. It must support `kind`,
   * and not have it yet.
   */
  addTo?: string;
  enabled: boolean;
  /** As the App derives them: a stdio MCP server runs a command (shell), and npx or uvx fetch it (network); a hook
   * runs a command (shell) on the files an agent edits (filesystem). */
  perms: readonly MockPerm[];
  score: number;
}

export const MOCK_EXTENSIONS: readonly MockExtension[] = [
  { name: "PDF tools", kind: "skill", agents: ["claude", "cursor", "gemini"], enabled: true, perms: ["filesystem", "network", "shell"], score: 77 },
  { name: "GitHub", kind: "mcp", agents: ["claude", "codex", "copilot"], enabled: true, perms: ["shell", "network"], score: 85 },
  // The one the window installs. A hook that runs when the agent finishes: its event has a counterpart in every agent
  // with hooks (Gemini's AfterAgent and Cursor's stop become Codex's Stop, hk-core adapter/hook_events.rs) and it
  // names no tool, so it fires as written once installed. A hook on a tool (Lint on save on the edit tool) would carry
  // a tool name the other agent doesn't use.
  { name: "Notify when done", kind: "hook", agents: ["cursor", "gemini"], addTo: "codex", enabled: true, perms: ["shell"], score: 100 },
  { name: "Code review", kind: "skill", agents: ["claude", "codex"], enabled: false, perms: ["filesystem"], score: 100 },
];

export type MockSeverity = "High" | "Medium";
/** The rules that fire below, by their key in the App's audit locale (rules.<key>.label). */
export type MockRule = "permissionComboRisk" | "dangerousCommands";

export interface MockAuditItem {
  name: string;
  score: number;
  /**
   * Each finding names the rule that fired and where, in the App's rule order (src/pages/audit-utils.ts AUDIT_RULES);
   * an empty list is a clean result. The App gives a skill's findings as SKILL.md:line (shortened here to the file
   * name, as its onboarding does) and an MCP server's as the server's name: it has no file to point into.
   */
  findings: readonly { rule: MockRule; severity: MockSeverity; location: string }[];
}

// Lowest score first, then by name, as the App's Audit page sorts. The cursor opens row 0. PDF tools' SKILL.md has a
// `sudo` install line at 12 (Dangerous Commands reads lines outside code blocks) and its first ```bash block at 18
// (where Permission Combination Risk points for shell plus network).
export const MOCK_AUDIT: readonly MockAuditItem[] = [
  {
    name: "PDF tools",
    score: 77,
    findings: [
      { rule: "dangerousCommands", severity: "Medium", location: "SKILL.md:12" },
      { rule: "permissionComboRisk", severity: "High", location: "SKILL.md:18" },
    ],
  },
  { name: "GitHub", score: 85, findings: [{ rule: "permissionComboRisk", severity: "High", location: "github" }] },
  { name: "Code review", score: 100, findings: [] },
  // A notification hook runs one command, shell only: no network, nothing dangerous.
  { name: "Notify when done", score: 100, findings: [] },
];

export const SEVERITY_TONE: Record<MockSeverity, string> = {
  High: "var(--trust-high-risk)",
  Medium: "var(--trust-low-risk)",
};

/** Trust tier color for a score, same cut-offs as the App's mock rows. A tone for dots, never text. */
export function tierTone(score: number): string {
  if (score >= 80) return "var(--trust-safe)";
  if (score >= 60) return "var(--trust-low-risk)";
  return "var(--trust-high-risk)";
}

// The Problem block (landing/Gather.astro): a few agents, each with a loose pile of extensions, and the one list
// HarnessKit makes of them: one extension of each kind, in the App's order. PDF tools and GitHub are the Extensions
// and Security Audit windows' rows, with the same agents and scores; the other three are this block's own, scored
// as the App would score them (gather-data.test.ts): Commit helper runs a postinstall script (Plugin Lifecycle
// Scripts, Low); Deploy CLI has no known install method (CLI Binary Source, Medium) and calls many API domains (CLI
// Network Access, Medium).
export type MockGatherKind = ExtensionKind;

export interface MockGatherItem {
  name: string;
  kind: MockGatherKind;
  /** agent ids from MOCK_GATHER_AGENTS; each must support `kind` (src/data/agents.ts) */
  agents: readonly string[];
  score: number;
}

/** The agents with piles, in pile order. */
export const MOCK_GATHER_AGENTS: readonly string[] = ["claude", "codex", "cursor", "gemini", "copilot"];

export const MOCK_GATHER: readonly MockGatherItem[] = [
  { name: "PDF tools", kind: "skill", agents: ["claude", "cursor", "gemini"], score: 77 },
  { name: "GitHub", kind: "mcp", agents: ["claude", "codex", "copilot"], score: 85 },
  { name: "Commit helper", kind: "plugin", agents: ["cursor", "gemini"], score: 97 },
  { name: "Format on save", kind: "hook", agents: ["claude", "codex"], score: 100 },
  { name: "Deploy CLI", kind: "cli", agents: ["copilot"], score: 84 },
];
