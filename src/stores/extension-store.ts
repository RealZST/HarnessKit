import { create } from "zustand";
import type { Extension, ExtensionKind, GroupedExtension, UpdateStatus } from "@/lib/types";
import { extensionGroupKey, sortAgentNames } from "@/lib/types";
import { api } from "@/lib/invoke";

interface PendingDelete {
  ids: Set<string>;
  extensions: Extension[];
  timer: ReturnType<typeof setTimeout>;
}

interface ExtensionState {
  extensions: Extension[];
  loading: boolean;
  kindFilter: ExtensionKind | null;
  agentFilter: string | null;
  searchQuery: string;
  /** Stores a groupKey (not a raw extension id). */
  selectedId: string | null;
  /** Stores groupKeys (not raw extension ids). */
  selectedIds: Set<string>;
  sortBy: "installed_at" | "name" | "trust_score";
  updateStatuses: Map<string, UpdateStatus>;
  allTags: string[];
  tagFilter: string | null;
  categoryFilter: string | null;
  pendingDelete: PendingDelete | null;
  tableSorting: { id: string; desc: boolean }[];
  setTableSorting: (sorting: { id: string; desc: boolean }[]) => void;
  fetch: () => Promise<void>;
  setKindFilter: (kind: ExtensionKind | null) => void;
  setAgentFilter: (agent: string | null) => void;
  setSearchQuery: (query: string) => void;
  setSelectedId: (id: string | null) => void;
  toggleSelected: (groupKey: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
  setSortBy: (sort: "installed_at" | "name" | "trust_score") => void;
  setTagFilter: (tag: string | null) => void;
  setCategoryFilter: (category: string | null) => void;
  fetchTags: () => Promise<void>;
  updateTags: (groupKey: string, tags: string[]) => Promise<void>;
  updateCategory: (groupKey: string, category: string | null) => Promise<void>;
  deployToAgent: (id: string, targetAgent: string) => Promise<void>;
  toggle: (groupKey: string, enabled: boolean) => Promise<void>;
  batchToggle: (enabled: boolean) => Promise<void>;
  batchDelete: () => void;
  undoDelete: () => void;
  confirmDelete: () => Promise<void>;
  checkingUpdates: boolean;
  updatingAll: boolean;
  checkUpdates: () => Promise<void>;
  updateExtension: (id: string) => Promise<void>;
  updateAll: () => Promise<number>;
  deleteFromAgents: (groupKey: string, agents: string[]) => Promise<void>;
  childSkillsOf: (cliId: string) => Extension[];
  grouped: () => GroupedExtension[];
  filtered: () => GroupedExtension[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
      category: instances.find((e) => e.category)?.category ?? null,
      permissions: deduplicatePermissions(instances.flatMap((e) => e.permissions)),
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
        (earliest, e) => (e.installed_at < earliest ? e.installed_at : earliest),
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

function deduplicatePermissions(
  perms: Extension["permissions"],
): Extension["permissions"] {
  const merged = new Map<string, Set<string>>();
  for (const p of perms) {
    const values = "paths" in p ? p.paths : "domains" in p ? p.domains : "commands" in p ? p.commands : "engines" in p ? p.engines : "keys" in p ? p.keys : [];
    const existing = merged.get(p.type) ?? new Set<string>();
    for (const v of values) existing.add(v);
    merged.set(p.type, existing);
  }
  const result: Extension["permissions"] = [];
  for (const [type, values] of merged) {
    const arr = [...values].sort();
    switch (type) {
      case "filesystem": result.push({ type, paths: arr }); break;
      case "network": result.push({ type, domains: arr }); break;
      case "shell": result.push({ type, commands: arr }); break;
      case "database": result.push({ type, engines: arr }); break;
      case "env": result.push({ type, keys: arr }); break;
    }
  }
  return result;
}

// Simple reference-equality memoization for grouped() —
// recomputes only when the extensions array reference changes.
let _cachedGroups: GroupedExtension[] = [];
let _cachedExtRef: Extension[] = [];

/** Expand selected groupKeys into the underlying extension IDs. */
function expandGroupKeys(
  groups: GroupedExtension[],
  keys: Set<string>,
): string[] {
  return groups
    .filter((g) => keys.has(g.groupKey))
    .flatMap((g) => g.instances.map((e) => e.id));
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useExtensionStore = create<ExtensionState>((set, get) => ({
  extensions: [],
  loading: false,
  kindFilter: null,
  agentFilter: null,
  searchQuery: "",
  selectedId: null,
  selectedIds: new Set(),
  sortBy: "installed_at",
  updateStatuses: new Map(),
  allTags: [],
  tagFilter: null,
  categoryFilter: null,
  pendingDelete: null,
  checkingUpdates: false,
  updatingAll: false,
  tableSorting: [],
  setTableSorting: (sorting) => set({ tableSorting: sorting }),

  async fetch() {
    set({ loading: true });
    try {
      // Agent filter is applied client-side so we always fetch all agents.
      const extensions = await api.listExtensions(
        get().kindFilter ?? undefined,
        undefined,
      );
      set({ extensions, loading: false });
      get().fetchTags();
      // Restore persisted update statuses from DB on first load
      if (get().updateStatuses.size === 0) {
        api.getCachedUpdateStatuses().then((results) => {
          if (results.length > 0) {
            const map = new Map<string, UpdateStatus>();
            for (const [id, status] of results) {
              map.set(id, status);
            }
            set({ updateStatuses: map });
          }
        }).catch(() => {});
      }
    } catch {
      set({ loading: false });
    }
  },

  setKindFilter(kind) {
    set({ kindFilter: kind });
    get().fetch();
  },
  // No re-fetch needed — agent filtering is client-side.
  setAgentFilter(agent) {
    set({ agentFilter: agent });
  },
  setSearchQuery(query) {
    set({ searchQuery: query });
  },
  setSelectedId(id) {
    set({ selectedId: id });
  },
  toggleSelected(groupKey) {
    const s = new Set(get().selectedIds);
    if (s.has(groupKey)) s.delete(groupKey);
    else s.add(groupKey);
    set({ selectedIds: s });
  },
  selectAll() {
    const keys = new Set(get().filtered().map((g) => g.groupKey));
    set({ selectedIds: keys });
  },
  clearSelection() {
    set({ selectedIds: new Set() });
  },
  setSortBy(sortBy) {
    set({ sortBy });
  },
  setTagFilter(tag) {
    set({ tagFilter: tag });
  },
  setCategoryFilter(category) {
    set({ categoryFilter: category });
  },

  async fetchTags() {
    const allTags = await api.getAllTags();
    set({ allTags });
  },

  async updateTags(groupKey, tags) {
    const group = get().grouped().find((g) => g.groupKey === groupKey);
    if (!group) return;
    await Promise.all(group.instances.map((e) => api.updateTags(e.id, tags)));
    const ids = new Set(group.instances.map((e) => e.id));
    set((s) => ({
      extensions: s.extensions.map((e) => (ids.has(e.id) ? { ...e, tags } : e)),
    }));
    get().fetchTags();
  },

  async updateCategory(groupKey, category) {
    const group = get().grouped().find((g) => g.groupKey === groupKey);
    if (!group) return;
    await Promise.all(
      group.instances.map((e) => api.updateCategory(e.id, category)),
    );
    const ids = new Set(group.instances.map((e) => e.id));
    set((s) => ({
      extensions: s.extensions.map((e) =>
        ids.has(e.id) ? { ...e, category } : e,
      ),
    }));
  },

  async deployToAgent(id, targetAgent) {
    await api.deployToAgent(id, targetAgent);
    get().fetch();
  },

  async toggle(groupKey, enabled) {
    const group = get().grouped().find((g) => g.groupKey === groupKey);
    if (!group) return;
    // Optimistic update — avoids full re-fetch which resets scroll position
    const ids = new Set(group.instances.map((e) => e.id));
    set((s) => ({
      extensions: s.extensions.map((e) =>
        ids.has(e.id) ? { ...e, enabled } : e,
      ),
    }));
    try {
      await Promise.all(
        group.instances.map((e) => api.toggleExtension(e.id, enabled)),
      );
    } catch {
      // Revert optimistic update on failure and re-fetch actual state
      set((s) => ({
        extensions: s.extensions.map((e) =>
          ids.has(e.id) ? { ...e, enabled: !enabled } : e,
        ),
      }));
      get().fetch();
    }
  },

  async batchToggle(enabled) {
    const ids = expandGroupKeys(get().grouped(), get().selectedIds);
    await Promise.all(ids.map((id) => api.toggleExtension(id, enabled)));
    set({ selectedIds: new Set() });
    get().fetch();
  },

  batchDelete() {
    const groups = get().grouped();
    const keys = get().selectedIds;
    const ids = new Set(expandGroupKeys(groups, keys));
    const removed = get().extensions.filter((e) => ids.has(e.id));
    // Optimistically hide from UI; clear detail panel if its group is being deleted
    const currentSelected = get().selectedId;
    const clearDetail = currentSelected != null && groups.some((g) => keys.has(g.groupKey) && g.groupKey === currentSelected);
    set((s) => ({
      extensions: s.extensions.filter((e) => !ids.has(e.id)),
      selectedIds: new Set(),
      selectedId: clearDetail ? null : s.selectedId,
    }));
    // Cancel any existing pending delete and hard-delete those first
    const prev = get().pendingDelete;
    if (prev) {
      clearTimeout(prev.timer);
      Promise.all([...prev.ids].map((id) => api.deleteExtension(id))).catch(() => {});
    }
    const timer = setTimeout(() => {
      get().confirmDelete();
    }, 5000);
    set({ pendingDelete: { ids, extensions: removed, timer } });
  },

  undoDelete() {
    const pending = get().pendingDelete;
    if (!pending) return;
    clearTimeout(pending.timer);
    set((s) => ({
      extensions: [...s.extensions, ...pending.extensions],
      pendingDelete: null,
    }));
  },

  async confirmDelete() {
    const pending = get().pendingDelete;
    if (!pending) return;
    clearTimeout(pending.timer);
    set({ pendingDelete: null });
    await Promise.all(
      [...pending.ids].map((id) => api.deleteExtension(id)),
    );
    get().fetch();
  },

  async checkUpdates() {
    set({ checkingUpdates: true });
    try {
      const results = await api.checkUpdates();
      const map = new Map<string, UpdateStatus>();
      for (const [id, status] of results) {
        map.set(id, status);
      }
      set({ updateStatuses: map });
    } finally {
      set({ checkingUpdates: false });
    }
  },

  async updateExtension(id: string) {
    await api.updateExtension(id);
    // Remove update status for this extension and all siblings in the same group
    // (backend updates all siblings, so clear them all from the UI)
    const statuses = new Map(get().updateStatuses);
    const group = get().grouped().find((g) => g.instances.some((i) => i.id === id));
    if (group) {
      for (const inst of group.instances) {
        statuses.delete(inst.id);
      }
    } else {
      statuses.delete(id);
    }
    set({ updateStatuses: statuses });
    // Re-fetch extensions to reflect new state
    await get().fetch();
  },

  async updateAll() {
    // Deduplicate: only update one instance per group (same skill across agents)
    const groups = get().grouped();
    const updateStatuses = get().updateStatuses;
    const toUpdate: { groupName: string; id: string; siblingIds: string[] }[] = [];
    for (const group of groups) {
      const updatableInst = group.instances.find(
        (inst) => updateStatuses.get(inst.id)?.status === "update_available",
      );
      if (updatableInst) {
        toUpdate.push({
          groupName: group.name,
          id: updatableInst.id,
          siblingIds: group.instances.map((i) => i.id),
        });
      }
    }
    if (toUpdate.length === 0) return 0;
    set({ updatingAll: true });
    let updated = 0;
    try {
      for (const { id, siblingIds } of toUpdate) {
        try {
          await api.updateExtension(id);
          // Remove update status for all instances in the group
          const statuses = new Map(get().updateStatuses);
          for (const sid of siblingIds) {
            statuses.delete(sid);
          }
          set({ updateStatuses: statuses });
          updated++;
        } catch {
          // continue with remaining updates
        }
      }
      await get().fetch();
    } finally {
      set({ updatingAll: false });
    }
    return updated;
  },

  async deleteFromAgents(groupKey, agentNames) {
    const group = get().grouped().find((g) => g.groupKey === groupKey);
    if (!group) return;
    const toDelete = group.instances.filter((e) =>
      e.agents.some((a) => agentNames.includes(a)),
    );
    if (toDelete.length === 0) return;
    const ids = new Set(toDelete.map((e) => e.id));
    // Optimistic removal
    set((s) => ({
      extensions: s.extensions.filter((e) => !ids.has(e.id)),
      selectedId: toDelete.length === group.instances.length ? null : s.selectedId,
    }));
    const prev = get().pendingDelete;
    if (prev) {
      clearTimeout(prev.timer);
      Promise.all([...prev.ids].map((id) => api.deleteExtension(id))).catch(() => {});
    }
    const timer = setTimeout(() => {
      get().confirmDelete();
    }, 5000);
    set({ pendingDelete: { ids, extensions: toDelete, timer } });
  },

  childSkillsOf(cliId: string) {
    return get().extensions.filter((e) => e.cli_parent_id === cliId);
  },

  grouped() {
    const exts = get().extensions;
    if (exts !== _cachedExtRef) {
      _cachedExtRef = exts;
      _cachedGroups = buildGroups(exts);
    }
    return _cachedGroups;
  },

  filtered() {
    const { searchQuery, tagFilter, categoryFilter, agentFilter } = get();
    let result = get().grouped();
    if (agentFilter) {
      result = result.filter((g) => g.agents.includes(agentFilter));
    }
    if (categoryFilter) {
      result = result.filter((g) => g.category === categoryFilter);
    }
    if (tagFilter) {
      result = result.filter((g) => g.tags.includes(tagFilter));
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (g) =>
          g.name.toLowerCase().includes(q) ||
          g.description.toLowerCase().includes(q),
      );
    }
    return result;
  },
}));
