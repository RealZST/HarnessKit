import {
  type InstallBlockReason,
  installBlockReason,
} from "@/lib/agent-capabilities";
import {
  type AgentInfo,
  type ConfigScope,
  type Extension,
  type GroupedExtension,
  logicalExtensionName,
  scopeKey,
} from "@/lib/types";
import {
  findCliChildren,
  instancesInScope,
  pickSourceInstance,
} from "@/stores/extension-helpers";

/** What a plan row is about: a group, optionally one of its CLI children,
 *  and the agent it goes to. */
export interface InstallItemRef {
  groupKey: string;
  /** Set when the item is a child of a CLI bundle, so rows can name it. */
  itemName?: string;
  targetAgent: string;
}

export interface InstallPair extends InstallItemRef {
  sourceId: string;
  isHermesSkill: boolean;
}

export type InstallSkipReason =
  | "already-installed"
  | InstallBlockReason
  /** Under Overwrite: this agent holds the copy everyone else gets. */
  | "source"
  /** Planned through other selected groups instead: a CLI bundle whose
   *  every item is selected as its own row, or the same item reached
   *  twice (two bundles sharing a child, two same-named groups). */
  | "duplicate"
  | "no-source";

export interface InstallSkip extends InstallItemRef {
  reason: InstallSkipReason;
  /** For `duplicate`: the selected groups that already cover this item. */
  via?: string[];
}

export interface InstallPlan {
  pairs: InstallPair[];
  skipped: InstallSkip[];
}

export interface InstallPlanOptions {
  /** Bring every target up to the newest copy: agents that already hold a
   *  copy in the target scope are re-installed from it, and the agent
   *  holding the newest copy itself is left alone. */
  overwrite?: boolean;
}

/** Expand groups × target agents into the installs to run plus the pairs
 *  to skip, with the reason for each. The detail panel runs it for one
 *  group and one agent, the bulk dialog for many of each; both go through
 *  the same gate as the tiles (`installBlockReason`), so a run never
 *  attempts what a tile would have greyed out. A CLI bundle that passes
 *  the gate expands to its children and takes or skips each one.
 *
 *  An agent is never asked to copy a file onto itself: several agents can
 *  share one skill folder (`~/.agents/skills`), and a project-scope CLI
 *  install prefers the copy already in that project — which may be the
 *  target's own. Copying a path onto itself empties the files. */
export function buildInstallPlan(
  groups: GroupedExtension[],
  targetAgents: AgentInfo[],
  extensions: Extension[],
  target: ConfigScope,
  { overwrite = false }: InstallPlanOptions = {},
): InstallPlan {
  const pairs: InstallPair[] = [];
  const skipped: InstallSkip[] = [];
  // item key → the group that planned it first.
  const planned = new Map<string, string>();
  // A child selected as its own row is that row's business: the bundle
  // it belongs to leaves it out, so each selected row is one line. Keyed
  // by row id, so an unrelated extension that merely shares a name does
  // not claim anything.
  const claimedRows = new Map<string, string>(
    groups
      .filter((g) => g.kind !== "cli")
      .flatMap((g) => g.instances.map((i) => [i.id, g.groupKey] as const)),
  );
  const claimer = (copies: Extension[]) =>
    copies.map((c) => claimedRows.get(c.id)).find((g) => g !== undefined);
  // Every row in the target scope: the holder check looks across groups,
  // since the same file can sit in two (two rows over one shared folder
  // with different source metadata). `extensions` is the full list, so
  // every group's own copies are in here too.
  const inTarget = instancesInScope(extensions, target);
  // Which copy an item is installed from. Overwrite means "match the
  // newest copy" — the one the user most recently edited, wherever it
  // lives — so that copy's holder is the source and is never overwritten.
  // Otherwise the usual preference: target scope, then Global, then any.
  const sourceOf = (copies: Extension[]) =>
    overwrite ? newest(copies) : pickSourceInstance(copies, target);

  for (const group of groups) {
    // One item per distinct child of a CLI bundle (a merged group lists
    // one child per agent copy); the group itself otherwise.
    const children =
      group.kind === "cli"
        ? [
            ...groupBy(
              findCliChildren(extensions, group.instances[0]?.id, group.pack),
              (c) => `${c.kind}\0${c.name}`,
            ),
          ]
        : null;
    const claimedBy = [
      ...new Set(
        (children ?? [])
          .map(([, copies]) => claimer(copies))
          .filter((g): g is string => g !== undefined),
      ),
    ];
    const items: { copies: Extension[]; item?: Extension }[] = children
      ? children
          .filter(([, copies]) => claimer(copies) === undefined)
          .map(([, copies]) => ({ copies, item: copies[0] }))
      : [{ copies: group.instances }];

    for (const agent of targetAgents) {
      const skip = (
        reason: InstallSkipReason,
        item?: Extension,
        via?: string[],
      ) =>
        skipped.push({
          groupKey: group.groupKey,
          itemName: item?.name,
          targetAgent: agent.name,
          reason,
          ...(via !== undefined && { via }),
        });

      const reason = installBlockReason(
        agent,
        group.kind,
        target,
        group.instances[0]?.mcp_transport,
      );
      // Block reasons win over "already installed": a kind nobody can
      // take reads the same on every agent, including the one that has it.
      if (reason) {
        skip(reason);
        continue;
      }
      if (items.length === 0) {
        // Every child is selected as its own row, or (a CLI found only by
        // its binary) there is nothing to copy.
        if (claimedBy.length > 0) skip("duplicate", undefined, claimedBy);
        else skip("no-source");
        continue;
      }

      for (const { copies, item } of items) {
        const source = sourceOf(copies);
        if (!source) {
          skip("no-source", item);
          continue;
        }
        const holdsSourceFile = inTarget.some(
          (e) => e.agents.includes(agent.name) && sameFile(e, source),
        );
        if (holdsSourceFile) {
          // Holds the very copy that would be written: nothing to do, and
          // under Overwrite it is the source everyone else gets.
          skip(overwrite ? "source" : "already-installed", item);
          continue;
        }
        // Any same-named copy counts, even one in another group (a skill
        // without a source URL is grouped per scope): replacing it is
        // what Overwrite is for.
        const holdsACopy = inTarget.some(
          (e) =>
            e.kind === source.kind &&
            logicalExtensionName(e) === logicalExtensionName(source) &&
            e.agents.includes(agent.name),
        );
        if (holdsACopy && !overwrite) {
          skip("already-installed", item);
          continue;
        }
        if (item) {
          const childReason = installBlockReason(
            agent,
            item.kind,
            target,
            item.mcp_transport,
          );
          if (childReason) {
            skip(childReason, item);
            continue;
          }
        }
        const key = `${source.kind}\0${source.name}\0${agent.name}`;
        const via = planned.get(key);
        if (via !== undefined) {
          skip("duplicate", item, [via]);
          continue;
        }
        planned.set(key, group.groupKey);
        pairs.push({
          groupKey: group.groupKey,
          itemName: item?.name,
          sourceId: source.id,
          targetAgent: agent.name,
          isHermesSkill: source.kind === "skill" && agent.name === "hermes",
        });
      }
    }
  }

  return { pairs, skipped };
}

/** Two rows that are the same file on disk: the same row, or two agents'
 *  rows over one shared folder. */
function sameFile(a: Extension, b: Extension): boolean {
  return a.id === b.id || (!!a.source_path && a.source_path === b.source_path);
}

/** The most recently modified copy (the scanner stamps `updated_at` from
 *  the file's mtime); ties keep list order. */
function newest(copies: Extension[]): Extension | undefined {
  return copies.reduce<Extension | undefined>(
    (best, c) => (!best || c.updated_at > best.updated_at ? c : best),
    undefined,
  );
}

export function groupBy<T>(
  items: readonly T[],
  key: (item: T) => string,
): Map<string, T[]> {
  const out = new Map<string, T[]>();
  for (const item of items) {
    const k = key(item);
    out.set(k, [...(out.get(k) ?? []), item]);
  }
  return out;
}

/** Stable id for a copy the UI shows before a rescan confirms it. */
export function pendingCopyId(pair: InstallPair, target: ConfigScope): string {
  return `pending:${pair.sourceId}:${pair.targetAgent}:${scopeKey(target)}`;
}
