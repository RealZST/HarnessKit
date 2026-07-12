import type { AgentInfo, ExtensionKind } from "@/lib/types";
import type { ScopeValue } from "@/stores/scope-store";

/** Whether `agent` can take an install of `kind` at `scope`.
 *
 *  Reads the backend-derived `AgentInfo.capabilities` (computed from the
 *  Rust adapter declarations in crates/hk-core/src/adapter/*.rs — see
 *  AgentCapabilities::from_adapter), so UI gating and backend deploy
 *  behavior share one source of truth and cannot drift.
 *
 *  Returns true for non-project scopes (Global / All) and false when the
 *  agent is unknown or its capabilities haven't loaded yet. */
export function canInstallAtScope(
  agent: AgentInfo | undefined,
  kind: ExtensionKind,
  scope: ScopeValue,
): boolean {
  if (scope.type !== "project") return true;
  const flags = agent?.capabilities?.project_install;
  if (!flags) return false;
  switch (kind) {
    case "skill":
      return flags.skill;
    case "mcp":
      return flags.mcp;
    case "hook":
      return flags.hook;
    case "cli":
      return flags.cli;
    default:
      return false;
  }
}
