import type { AgentInfo, ExtensionKind, McpTransport } from "@/lib/types";
import type { ScopeValue } from "@/stores/scope-store";

/** Whether `agent` can receive an MCP server of the given transport.
 *
 *  Same source of truth as `canInstallAtScope`: the backend derives
 *  `capabilities.mcp_remote` from each adapter's RemoteMcpSchema, and the
 *  deployer enforces the same rule (e.g. Codex takes Streamable HTTP but
 *  not SSE). Stdio always passes; an absent transport (legacy rows) is
 *  treated as stdio; absent capabilities (agent unknown / old backend)
 *  gate remote transports off. */
export function canReceiveMcpTransport(
  agent: AgentInfo | undefined,
  transport: McpTransport | undefined,
): boolean {
  if (!transport || transport === "stdio") return true;
  const flags = agent?.capabilities?.mcp_remote;
  if (!flags) return false;
  return transport === "http" ? flags.http : flags.sse;
}

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
