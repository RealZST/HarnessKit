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

/** Whether this plugin ships WITH its agent and so cannot be deleted.
 *
 *  Mirrors `AgentAdapter::plugin_removal` returning `Shipped`: the backend
 *  refuses these, and the same list arrives as
 *  `capabilities.vendor_baseline_packs`, so the greyed-out button and the
 *  refusal can never disagree. dsh is the only agent with a non-empty list
 *  today — its in-box bundles (`@deepseek-ai/dsh-base`, …) contribute most of
 *  its plugin rows, while a plugin the user installed carries the pack of the
 *  third-party bundle that brought it and stays deletable.
 *
 *  Keyed on the pack alone — never on the kind, and never on which agent owns
 *  the row. `getCachedFiltered` tests the same flat set of shipped packs, so
 *  the greyed-out button and the hide filter can never disagree about a row;
 *  an agent that later ships built-in skills instead of plugins is covered
 *  without a change here either. */
export function isVendorBaseline(
  pack: string | null | undefined,
  shippedPacks: Iterable<string>,
): boolean {
  return !!pack && new Set(shippedPacks).has(pack);
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
