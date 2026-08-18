import type {
  ConfigScope,
  Extension,
  ExtensionKind,
  GroupedExtension,
} from "@/lib/types";
import {
  deriveExtensionUrl,
  extensionGroupKey,
  logicalExtensionName,
  scopeKey,
  sortAgentNames,
} from "@/lib/types";
import type { ScopeValue } from "@/stores/scope-store";

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

function deduplicatePermissions(
  perms: Extension["permissions"],
): Extension["permissions"] {
  const merged = new Map<string, Set<string>>();
  for (const p of perms) {
    const values =
      "paths" in p
        ? p.paths
        : "domains" in p
          ? p.domains
          : "commands" in p
            ? p.commands
            : "engines" in p
              ? p.engines
              : "keys" in p
                ? p.keys
                : [];
    const existing = merged.get(p.type) ?? new Set<string>();
    for (const v of values) existing.add(v);
    merged.set(p.type, existing);
  }
  const result: Extension["permissions"] = [];
  for (const [type, values] of merged) {
    const arr = [...values].sort();
    switch (type) {
      case "filesystem":
        result.push({ type, paths: arr });
        break;
      case "network":
        result.push({ type, domains: arr });
        break;
      case "shell":
        result.push({ type, commands: arr });
        break;
      case "database":
        result.push({ type, engines: arr });
        break;
      case "env":
        result.push({ type, keys: arr });
        break;
    }
  }
  return result;
}

/** Instances visible in `scope` (All mode = everything). The single scope
 *  projection every display surface builds on — badges, the agent filter,
 *  the detail panel's PATHS cards and DOCUMENTATION tabs — so they can
 *  never disagree about what exists in the active scope. Accepts
 *  ConfigScope too (ConfigScope ⊂ ScopeValue). */
export function instancesInScope(
  instances: Extension[],
  scope: ScopeValue | ConfigScope,
): Extension[] {
  if (scope.type === "all") return instances;
  const key = scope.type === "global" ? "global" : scope.path;
  return instances.filter((i) => scopeKey(i.scope) === key);
}

/** Agent names that have an instance of `group` in `scope`. All mode
 *  returns the full cross-scope union. Group identity (`group.agents`)
 *  stays scope-free — this is a DISPLAY/FILTER projection.
 *
 *  Pass `enabledAgents` to drop the ones the user switched off. A row that
 *  exists *only* on switched-off agents is gone from the list entirely, so
 *  leaving their badges on the rows that survive would be a half-truth: the
 *  same agent both absent and present depending on which row you look at.
 *  The file-level surfaces — PATHS, the delete dialog — deliberately do NOT
 *  take this projection; they must keep showing every copy on disk, so
 *  deleting a row can never quietly remove a file the user was never shown. */
export function agentsInScope(
  group: GroupedExtension,
  scope: ScopeValue,
  enabledAgents: ReadonlySet<string> | null = null,
): string[] {
  const keep = (names: string[]) =>
    enabledAgents ? names.filter((a) => enabledAgents.has(a)) : names;
  if (scope.type === "all") return keep(group.agents);
  return keep(
    sortAgentNames([
      ...new Set(
        instancesInScope(group.instances, scope).flatMap((i) => i.agents),
      ),
    ]),
  );
}

/** The agents a count should include, or null for "don't filter".
 *
 *  Null is what an unfetched agent list looks like: the backend always reports
 *  every adapter, so an empty array means the request hasn't landed, and
 *  reading that as "nothing is enabled" would blank every list on first paint.
 *  Callers pass the result straight to `groupHasEnabledAgent`. */
export function enabledAgentSet(
  agents: readonly { name: string; enabled: boolean }[],
): ReadonlySet<string> | null {
  if (agents.length === 0) return null;
  return new Set(agents.filter((a) => a.enabled).map((a) => a.name));
}

/** Does this group exist on at least one agent the user still has switched on?
 *
 *  Disabling an agent means "I don't use this" — its rows have no business
 *  padding the Extensions list or the audit report either. The rule is
 *  group-level on purpose: a skill installed on both Claude and a disabled
 *  Windsurf stays, with every instance intact, so deleting it still cleans up
 *  the Windsurf copy on disk. Only a group that lives *nowhere* enabled drops
 *  out. Agentless rows (rare, scanner-synthesised) always pass.
 *
 *  Every surface that reports an extension count routes through here — see
 *  `getCachedFiltered`, the Overview stats and the Audit page — so the three
 *  numbers can't drift apart again. */
export function groupHasEnabledAgent(
  group: GroupedExtension,
  enabledAgents: ReadonlySet<string>,
): boolean {
  return (
    group.agents.length === 0 || group.agents.some((a) => enabledAgents.has(a))
  );
}

/** Scope an Install-to-Agent action targets for a given active scope:
 *  the project itself in project mode, Global otherwise. All-mode callers
 *  use the explicit ScopeTargetField picker instead of calling this; the
 *  all→global branch remains as a safe fallback. */
export function resolveInstallTargetScope(scope: ScopeValue): ConfigScope {
  return scope.type === "project"
    ? { type: "project", name: scope.name, path: scope.path }
    : { type: "global" };
}

/** Instance a cross-agent install copies from: prefer the copy already in
 *  the target scope, then a global copy, then anything. The backend reads
 *  the source through the instance's own scope, so any instance works. */
export function pickSourceInstance(
  instances: Extension[],
  targetScope: ConfigScope,
): Extension | undefined {
  const targetKey = scopeKey(targetScope);
  return (
    instances.find((i) => scopeKey(i.scope) === targetKey) ??
    instances.find((i) => i.scope.type === "global") ??
    instances[0]
  );
}

export function buildGroups(extensions: Extension[]): GroupedExtension[] {
  // Pre-pass: index URL-keyed groups by (kind, logical name, scope) so a
  // sourceless instance (e.g. an agent-discovered copy that lacks pack
  // metadata) can attach to its marketplace-installed sibling instead of
  // forming a separate row. Only redirect when there is exactly one such
  // sibling — multiple distinct developers in the same scope means we can't
  // tell which one a sourceless row belongs to.
  const urlSiblings = new Map<string, Set<string>>();
  for (const ext of extensions) {
    if (deriveExtensionUrl(ext) == null) continue;
    const sk = `${ext.kind}\0${logicalExtensionName(ext)}\0${scopeKey(ext.scope)}`;
    const keys = urlSiblings.get(sk) ?? new Set<string>();
    keys.add(extensionGroupKey(ext));
    urlSiblings.set(sk, keys);
  }

  const map = new Map<string, Extension[]>();
  for (const ext of extensions) {
    let key = extensionGroupKey(ext);
    if (deriveExtensionUrl(ext) == null) {
      const sk = `${ext.kind}\0${logicalExtensionName(ext)}\0${scopeKey(ext.scope)}`;
      const siblings = urlSiblings.get(sk);
      if (siblings?.size === 1) {
        key = siblings.values().next().value as string;
      }
    }
    const list = map.get(key);
    if (list) list.push(ext);
    else map.set(key, [ext]);
  }
  const groups: GroupedExtension[] = [];
  for (const [key, instances] of map) {
    const first = instances[0];
    groups.push({
      groupKey: key,
      name: first.name,
      kind: first.kind,
      description: first.description,
      source: first.source,
      agents: sortAgentNames([...new Set(instances.flatMap((e) => e.agents))]),
      tags: [...new Set(instances.flatMap((e) => e.tags))],
      pack: instances.find((e) => e.pack)?.pack ?? null,
      permissions: deduplicatePermissions(
        instances.flatMap((e) => e.permissions),
      ),
      enabled: instances.some((e) => e.enabled),
      trust_score: instances.reduce<number | null>(
        (min, e) =>
          e.trust_score != null
            ? min != null
              ? Math.min(min, e.trust_score)
              : e.trust_score
            : min,
        null,
      ),
      installed_at: instances.reduce(
        (earliest, e) =>
          e.installed_at < earliest ? e.installed_at : earliest,
        first.installed_at,
      ),
      updated_at: instances.reduce(
        (latest, e) => (e.updated_at > latest ? e.updated_at : latest),
        first.updated_at,
      ),
      instances,
    });
  }
  return groups;
}

/** `instance id -> the groupKey buildGroups actually assigned it`.
 *
 *  For anything that starts from a bare extension ID — audit results, deep
 *  links — this is the only correct way to reach a row. Recomputing
 *  `extensionGroupKey(ext)` from the instance skips the sibling merge above,
 *  so a sourceless copy and its URL-carrying twin resolve to two different
 *  keys and show up as two rows for one extension. */
export function groupKeyById(groups: GroupedExtension[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const g of groups) {
    for (const inst of g.instances) map.set(inst.id, g.groupKey);
  }
  return map;
}

/** Find all child extensions of a CLI group (by cli_parent_id or matching pack).
 *  When one instance of a group matches, all sibling instances are included
 *  so that toggle/delete affects every agent the extension is installed on. */
export function findCliChildren(
  extensions: Extension[],
  cliId: string | undefined,
  cliPack: string | null,
): Extension[] {
  // First pass: find groupKeys of matching extensions
  const matchedGroupKeys = new Set<string>();
  for (const e of extensions) {
    if (e.kind === "cli") continue;
    if (
      (cliId && e.cli_parent_id === cliId) ||
      (cliPack && e.pack === cliPack)
    ) {
      matchedGroupKeys.add(extensionGroupKey(e));
    }
  }
  // Second pass: return ALL extensions belonging to matched groups
  return extensions.filter(
    (e) => e.kind !== "cli" && matchedGroupKeys.has(extensionGroupKey(e)),
  );
}

/** Expand selected groupKeys into the underlying extension IDs. */
export function expandGroupKeys(
  groups: GroupedExtension[],
  keys: Set<string>,
): string[] {
  return groups
    .filter((g) => keys.has(g.groupKey))
    .flatMap((g) => g.instances.map((e) => e.id));
}

// ---------------------------------------------------------------------------
// Memoized accessors (module-level cache)
// ---------------------------------------------------------------------------

// Simple reference-equality memoization for grouped() —
// recomputes only when the extensions array reference changes.
let _cachedGroups: GroupedExtension[] = [];
let _cachedExtRef: Extension[] = [];

// Memoization for filtered() — avoids re-filtering on every render call.
let _cachedFiltered: GroupedExtension[] = [];
let _cachedFilterKey = "";
let _cachedFilterGroupsRef: GroupedExtension[] = [];

export function getCachedGroups(extensions: Extension[]): GroupedExtension[] {
  if (extensions !== _cachedExtRef) {
    _cachedExtRef = extensions;
    _cachedGroups = buildGroups(extensions);
  }
  return _cachedGroups;
}

/** Synthetic `packFilter` value: one agent's whole shipped baseline, as a
 *  single "source". dsh ships its baseline across three bundles, and listing
 *  each as its own dropdown row is noise — they are one provenance from the
 *  user's side. Prefixed so it can never collide with a real pack name. */
export const BUILTIN_PACK_PREFIX = "__hk_builtin__:";

export function getCachedFiltered(
  groups: GroupedExtension[],
  kindFilter: ExtensionKind | null,
  agentFilter: string | null,
  packFilter: string | null,
  tagFilter: string | null,
  searchQuery: string,
  scope: ScopeValue,
  /** `agent name -> packs that ship with it`, from
   *  `capabilities.vendor_baseline_packs`. Empty for every agent whose
   *  baseline is compiled in and so never appears as an extension. */
  vendorBaselineByAgent: Record<string, string[]> = {},
  /** Drop every shipped-baseline row. The list is about what the user
   *  configured, and an agent that ships its own internals as extensions
   *  buries that — dsh contributes ~130 rows against ~20 from every other
   *  agent combined. Off by default: the baseline stays visible unless asked. */
  hideVendorBaseline = false,
  /** Agents the user has switched on. Rows that exist only on switched-off
   *  agents are dropped before any user filter runs — they are not part of
   *  the list at all, so no "Clear filters" affordance can bring them back.
   *  Defaults to null (no agent is disabled) for callers that don't care. */
  enabledAgents: ReadonlySet<string> | null = null,
): GroupedExtension[] {
  // Memoize: skip recomputation if inputs haven't changed
  const scopeKeyForCache =
    scope.type === "all"
      ? "all"
      : scope.type === "global"
        ? "global"
        : `project:${scope.path}`;
  const shippedPacks = new Set(Object.values(vendorBaselineByAgent).flat());
  const enabledKey = enabledAgents ? [...enabledAgents].sort().join(",") : "*";
  const key = `${groups.length}|${kindFilter}|${agentFilter}|${packFilter}|${tagFilter}|${searchQuery}|${scopeKeyForCache}|${[...shippedPacks].join(",")}|${hideVendorBaseline}|${enabledKey}`;
  if (key === _cachedFilterKey && groups === _cachedFilterGroupsRef) {
    return _cachedFiltered;
  }
  let result = groups;
  if (enabledAgents) {
    result = result.filter((g) => groupHasEnabledAgent(g, enabledAgents));
  }
  if (kindFilter) {
    result = result.filter((g) => g.kind === kindFilter);
  }
  if (agentFilter) {
    // Match against the active scope's agents (same projection the badge
    // column renders) so the filter can never surface a card whose visible
    // badges don't contain the filtered agent.
    result = result.filter((g) =>
      agentsInScope(g, scope, enabledAgents).includes(agentFilter),
    );
  }
  // Applied before packFilter so "hide built-ins" and an explicit built-in
  // source selection can't contradict each other on screen: the toggle wins
  // and the list goes empty, which is the honest answer.
  if (hideVendorBaseline && shippedPacks.size > 0) {
    result = result.filter((g) => !g.pack || !shippedPacks.has(g.pack));
  }
  if (packFilter?.startsWith(BUILTIN_PACK_PREFIX)) {
    const agent = packFilter.slice(BUILTIN_PACK_PREFIX.length);
    const packs = new Set(vendorBaselineByAgent[agent] ?? []);
    result = result.filter((g) => !!g.pack && packs.has(g.pack));
  } else if (packFilter) {
    result = result.filter((g) => g.pack === packFilter);
  }
  if (tagFilter) {
    result = result.filter((g) => g.tags.includes(tagFilter));
  }
  if (scope.type !== "all") {
    // Match if any instance is in the requested scope. After Phase C dedup,
    // a single group can span multiple scopes, so we look across instances.
    result = result.filter(
      (g) => instancesInScope(g.instances, scope).length > 0,
    );
  }
  if (searchQuery.trim()) {
    const q = searchQuery.toLowerCase();
    result = result.filter(
      (g) =>
        g.name.toLowerCase().includes(q) ||
        g.description.toLowerCase().includes(q),
    );
  }
  _cachedFilterKey = key;
  _cachedFilterGroupsRef = groups;
  _cachedFiltered = result;
  return result;
}
