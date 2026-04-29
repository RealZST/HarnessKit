import type { Extension, ExtensionKind, GroupedExtension } from "@/lib/types";
import { extensionGroupKey, scopeKey, sortAgentNames } from "@/lib/types";

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

export function buildGroups(extensions: Extension[]): GroupedExtension[] {
  const map = new Map<string, Extension[]>();
  for (const ext of extensions) {
    const key = extensionGroupKey(ext);
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

export function getCachedFiltered(
  groups: GroupedExtension[],
  kindFilter: ExtensionKind | null,
  agentFilter: string | null,
  packFilter: string | null,
  tagFilter: string | null,
  searchQuery: string,
  scopeFilter: string | null,
): GroupedExtension[] {
  // Memoize: skip recomputation if inputs haven't changed
  const key = `${groups.length}|${kindFilter}|${agentFilter}|${packFilter}|${tagFilter}|${searchQuery}|${scopeFilter}`;
  if (key === _cachedFilterKey && groups === _cachedFilterGroupsRef) {
    return _cachedFiltered;
  }
  let result = groups;
  if (kindFilter) {
    result = result.filter((g) => g.kind === kindFilter);
  }
  if (agentFilter) {
    result = result.filter((g) => g.agents.includes(agentFilter));
  }
  if (packFilter) {
    result = result.filter((g) => g.pack === packFilter);
  }
  if (tagFilter) {
    result = result.filter((g) => g.tags.includes(tagFilter));
  }
  if (scopeFilter) {
    // Match if any instance is in the requested scope. After Phase C dedup,
    // a single group can span multiple scopes, so we look across instances.
    result = result.filter((g) =>
      g.instances.some((i) => scopeKey(i.scope) === scopeFilter),
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
