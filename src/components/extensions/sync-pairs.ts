import {
  canInstallAtScope,
  canReceiveMcpTransport,
} from "@/lib/agent-capabilities";
import type { AgentInfo, Extension, GroupedExtension } from "@/lib/types";
import {
  findCliChildren,
  instancesInScope,
  pickSourceInstance,
} from "@/stores/extension-helpers";

export interface SyncPair {
  groupKey: string;
  sourceId: string;
  targetAgent: string;
  isHermesSkill: boolean;
}

export type SyncSkipReason =
  | "already-installed"
  | "hook-unsupported"
  | "global-hook-blocked"
  | "transport-unsupported"
  | "no-source";

export interface SyncSkip {
  groupKey: string;
  targetAgent: string;
  reason: SyncSkipReason;
}

/** Global scope every bulk-sync install targets. */
const GLOBAL_SCOPE = { type: "global" } as const;

/**
 * Expand selected groups × target agents into installable pairs plus
 * preflight skips. Predicates mirror `extension-detail.tsx` verbatim —
 * same capability flags, same order, same CLI child dedupe.
 */
export function buildSyncPairs(
  groups: GroupedExtension[],
  targetAgents: AgentInfo[],
  extensions: Extension[],
): { pairs: SyncPair[]; skipped: SyncSkip[] } {
  const pairs: SyncPair[] = [];
  const skipped: SyncSkip[] = [];
  if (groups.length === 0 || targetAgents.length === 0) {
    return { pairs, skipped };
  }

  for (const group of groups) {
    for (const agent of targetAgents) {
      // Already has a copy at Global scope — mirrors `agentsInTargetScope`
      // (extension-detail.tsx:114-120).
      const installedAgents = instancesInScope(
        group.instances,
        GLOBAL_SCOPE,
      ).flatMap((i) => i.agents);
      if (installedAgents.includes(agent.name)) {
        skipped.push({
          groupKey: group.groupKey,
          targetAgent: agent.name,
          reason: "already-installed",
        });
        continue;
      }

      if (group.kind === "cli") {
        const children = findCliChildren(
          extensions,
          group.instances[0]?.id,
          group.pack,
        );
        const seen = new Set<string>();
        for (const child of children) {
          const dedupeKey = child.name + child.kind;
          if (seen.has(dedupeKey)) continue;
          seen.add(dedupeKey);
          if (
            !canInstallAtScope(agent, child.kind, GLOBAL_SCOPE) ||
            (child.kind === "mcp" &&
              !canReceiveMcpTransport(agent, child.mcp_transport))
          ) {
            skipped.push({
              groupKey: group.groupKey,
              targetAgent: agent.name,
              reason: "transport-unsupported",
            });
            continue;
          }
          pairs.push({
            groupKey: group.groupKey,
            sourceId: child.id,
            targetAgent: agent.name,
            isHermesSkill: child.kind === "skill" && agent.name === "hermes",
          });
        }
        continue;
      }

      const hookUnsupported =
        group.kind === "hook" && !agent.capabilities.hooks_supported;
      if (hookUnsupported) {
        skipped.push({
          groupKey: group.groupKey,
          targetAgent: agent.name,
          reason: "hook-unsupported",
        });
        continue;
      }
      const globalHookBlocked =
        group.kind === "hook" && !agent.capabilities.global_hook_install;
      if (globalHookBlocked) {
        skipped.push({
          groupKey: group.groupKey,
          targetAgent: agent.name,
          reason: "global-hook-blocked",
        });
        continue;
      }
      const transportUnsupported =
        group.kind === "mcp" &&
        !canReceiveMcpTransport(agent, group.instances[0]?.mcp_transport);
      if (transportUnsupported) {
        skipped.push({
          groupKey: group.groupKey,
          targetAgent: agent.name,
          reason: "transport-unsupported",
        });
        continue;
      }

      const source = pickSourceInstance(group.instances, GLOBAL_SCOPE);
      if (!source) {
        skipped.push({
          groupKey: group.groupKey,
          targetAgent: agent.name,
          reason: "no-source",
        });
        continue;
      }
      pairs.push({
        groupKey: group.groupKey,
        sourceId: source.id,
        targetAgent: agent.name,
        isHermesSkill: group.kind === "skill" && agent.name === "hermes",
      });
    }
  }

  return { pairs, skipped };
}
