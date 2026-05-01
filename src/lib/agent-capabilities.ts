import type { ExtensionKind } from "@/lib/types";
import type { ScopeValue } from "@/stores/scope-store";

// Mirrors the per-adapter project_skill_dirs / project_mcp_config_relpath /
// project_hook_config_relpath declarations in crates/hk-core/src/adapter/*.rs.
//
// All 7 agents support project-level skill via the Universal Agent Skills
// standard (SKILL.md, December 2025). Task 7 declares project_skill_dirs on
// each adapter so this row is always ✓ for skill in v1.
//
// "mcp" / "hook" / "cli" rows are forward-compat for v2 cross-agent deploy
// (see follow-up roadmap). Several adapters need MCP/hook completion before
// those columns become accurate; v1 install pipeline doesn't consume them.
//
// Keep in sync when adapters change project-level declarations.
const PROJECT_INSTALL_SUPPORT: Record<string, Set<ExtensionKind>> = {
  claude: new Set(["skill", "mcp", "hook", "cli"]),
  codex: new Set(["skill"]), // MCP/hook adapter completion deferred (v2)
  cursor: new Set(["skill", "mcp", "hook"]),
  windsurf: new Set(["skill", "mcp", "hook"]),
  gemini: new Set(["skill"]), // MCP/hook adapter completion deferred (v2)
  antigravity: new Set(["skill"]), // MCP/hook adapter completion deferred (v2)
  copilot: new Set(["skill"]), // MCP adapter completion deferred (v2)
};

/** Whether the agent's adapter declares project-level support for this kind.
 *  Returns true for non-project scopes (Global / All). */
export function canInstallAtScope(
  agent: string,
  kind: ExtensionKind,
  scope: ScopeValue,
): boolean {
  if (scope.type !== "project") return true;
  return PROJECT_INSTALL_SUPPORT[agent]?.has(kind) ?? false;
}
