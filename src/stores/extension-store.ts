import { create } from "zustand";
import { api } from "@/lib/invoke";
import type {
  Extension,
  ExtensionKind,
  GroupedExtension,
  NewRepoSkill,
  UpdateStatus,
} from "@/lib/types";
import {
  expandGroupKeys,
  findCliChildren,
  getCachedFiltered,
  getCachedGroups,
} from "./extension-helpers";
import { toast } from "./toast-store";

export { buildGroups } from "./extension-helpers";

interface PendingDelete {
  ids: Set<string>;
  extensions: Extension[];
  timer: ReturnType<typeof setTimeout>;
}

interface ExtensionState {
  extensions: Extension[];
  loading: boolean;
  hasFetched: boolean;
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
  packFilter: string | null;
  allPacks: string[];
  pendingDelete: PendingDelete | null;
  tableSorting: { id: string; desc: boolean }[];
  setTableSorting: (sorting: { id: string; desc: boolean }[]) => void;
  rescanAndFetch: () => Promise<void>;
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
  setPackFilter: (pack: string | null) => void;
  fetchTags: () => Promise<void>;
  updateTags: (groupKey: string, tags: string[]) => Promise<void>;
  updatePack: (groupKey: string, pack: string | null) => Promise<void>;
  fetchPacks: () => Promise<void>;
  installToAgent: (id: string, targetAgent: string) => Promise<void>;
  toggle: (groupKey: string, enabled: boolean) => Promise<void>;
  batchToggle: (enabled: boolean) => Promise<void>;
  undoDelete: () => void;
  confirmDelete: () => Promise<void>;
  checkingUpdates: boolean;
  updatingAll: boolean;
  newRepoSkills: NewRepoSkill[];
  checkUpdates: () => Promise<void>;
  updateExtension: (id: string) => Promise<boolean>;
  updateAll: () => Promise<number>;
  installNewRepoSkills: (
    url: string,
    skillIds: string[],
    targetAgents: string[],
  ) => Promise<void>;
  deleteFromAgents: (groupKey: string, agents: string[]) => Promise<void>;
  grouped: () => GroupedExtension[];
  filtered: () => GroupedExtension[];
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useExtensionStore = create<ExtensionState>((set, get) => ({
  extensions: [],
  loading: false,
  hasFetched: false,
  kindFilter: null,
  agentFilter: null,
  searchQuery: "",
  selectedId: null,
  selectedIds: new Set(),
  sortBy: "name",
  updateStatuses: new Map(),
  allTags: [],
  tagFilter: null,
  packFilter: null,
  allPacks: [],
  pendingDelete: null,
  checkingUpdates: false,
  updatingAll: false,
  newRepoSkills: [],
  tableSorting: [{ id: "name", desc: false }],
  setTableSorting: (sorting) => set({ tableSorting: sorting }),

  /** Full rescan + fetch — use after any operation that changes extensions on disk. */
  async rescanAndFetch() {
    await api.scanAndSync();
    await get().fetch();
  },

  async fetch() {
    set({ loading: true });
    try {
      // Always fetch all extensions — kind/agent filtering is applied client-side
      // so that detail panels can access child extensions across all types.
      const extensions = await api.listExtensions(undefined, undefined);
      set({ extensions, loading: false, hasFetched: true });
      get().fetchTags();
      get().fetchPacks();
      // Restore persisted update statuses from DB on first load
      if (get().updateStatuses.size === 0) {
        api
          .getCachedUpdateStatuses()
          .then((results) => {
            if (results.length > 0) {
              const map = new Map<string, UpdateStatus>();
              for (const [id, status] of results) {
                map.set(id, status);
              }
              set({ updateStatuses: map });
            }
          })
          .catch((e) =>
            console.error("Failed to load cached update statuses:", e),
          );
      }
    } catch (e) {
      console.error("Failed to fetch extensions:", e);
      set({ loading: false, hasFetched: true });
    }
  },

  setKindFilter(kind) {
    set({ kindFilter: kind });
  },
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
    const keys = new Set(
      get()
        .filtered()
        .map((g) => g.groupKey),
    );
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
  setPackFilter(pack) {
    set({ packFilter: pack });
  },

  async fetchTags() {
    const allTags = await api.getAllTags();
    set({ allTags });
  },

  async updateTags(groupKey, tags) {
    const group = get()
      .grouped()
      .find((g) => g.groupKey === groupKey);
    if (!group) return;
    const ids = group.instances.map((e) => e.id);
    await api.batchUpdateTags(ids, tags);
    const idSet = new Set(ids);
    set((s) => ({
      extensions: s.extensions.map((e) =>
        idSet.has(e.id) ? { ...e, tags } : e,
      ),
    }));
    get().fetchTags();
  },

  async updatePack(groupKey, pack) {
    const group = get()
      .grouped()
      .find((g) => g.groupKey === groupKey);
    if (!group) return;
    const ids = group.instances.map((e) => e.id);
    await api.batchUpdatePack(ids, pack);
    const idSet = new Set(ids);
    set((s) => ({
      extensions: s.extensions.map((e) =>
        idSet.has(e.id) ? { ...e, pack } : e,
      ),
    }));
    get().fetchPacks();
  },

  async fetchPacks() {
    const allPacks = await api.getAllPacks();
    const { packFilter } = get();
    set({
      allPacks,
      // Clear stale pack filter if the pack no longer exists
      ...(packFilter && !allPacks.includes(packFilter)
        ? { packFilter: null }
        : {}),
    });
  },

  async installToAgent(id, targetAgent) {
    await api.installToAgent(id, targetAgent);
    await get().rescanAndFetch();
  },

  async toggle(groupKey, enabled) {
    const group = get()
      .grouped()
      .find((g) => g.groupKey === groupKey);
    if (!group) return;

    // For CLI: also toggle all child extensions
    let allToToggle = [...group.instances];
    if (group.kind === "cli") {
      allToToggle = [
        ...allToToggle,
        ...findCliChildren(
          get().extensions,
          group.instances[0]?.id,
          group.pack,
        ),
      ];
    }

    const ids = new Set(allToToggle.map((e) => e.id));
    // Optimistic update
    set((s) => ({
      extensions: s.extensions.map((e) =>
        ids.has(e.id) ? { ...e, enabled } : e,
      ),
    }));

    const results = await Promise.allSettled(
      allToToggle.map((e) => api.toggleExtension(e.id, enabled)),
    );

    const failedIds = new Set<string>();
    results.forEach((result, index) => {
      if (result.status === "rejected") {
        failedIds.add(allToToggle[index].id);
      }
    });

    if (failedIds.size > 0) {
      set((s) => ({
        extensions: s.extensions.map((e) =>
          failedIds.has(e.id) ? { ...e, enabled: !enabled } : e,
        ),
      }));
      get().fetch();
    }
  },

  async batchToggle(enabled) {
    const ids = expandGroupKeys(get().grouped(), get().selectedIds);
    const results = await Promise.allSettled(
      ids.map((id) => api.toggleExtension(id, enabled)),
    );
    const failedIds = new Set<string>();
    results.forEach((result, index) => {
      if (result.status === "rejected") {
        failedIds.add(ids[index]);
      }
    });
    set({ selectedIds: new Set() });
    get().fetch();
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
    await Promise.all([...pending.ids].map((id) => api.deleteExtension(id)));
    // Remove CLI binary only on full uninstall (CLI parent is in the set, not just children)
    for (const ext of pending.extensions) {
      if (
        ext.kind === "cli" &&
        !ext.cli_parent_id &&
        ext.cli_meta?.binary_path
      ) {
        await api
          .uninstallCliBinary(ext.cli_meta.binary_path)
          .catch((e) => console.error("Failed to remove CLI binary:", e));
      }
    }
    // Rescan so partially-deleted CLIs are re-discovered with remaining agents
    await get().rescanAndFetch();
  },

  async checkUpdates() {
    set({ checkingUpdates: true });
    try {
      const result = await api.checkUpdates();
      const map = new Map<string, UpdateStatus>();
      for (const [id, status] of result.statuses) {
        map.set(id, status);
      }
      set({ updateStatuses: map, newRepoSkills: result.new_skills });
    } finally {
      set({ checkingUpdates: false });
    }
  },

  async updateExtension(id: string) {
    const result = await api.updateExtension(id);
    if (result.skipped) {
      toast.info(
        `${result.name} is no longer available in the remote repository`,
      );
      // Set removed_from_repo status for all siblings
      const statuses = new Map(get().updateStatuses);
      const group = get()
        .grouped()
        .find((g) => g.instances.some((i) => i.id === id));
      const removedStatus = { status: "removed_from_repo" as const };
      if (group) {
        for (const inst of group.instances) {
          statuses.set(inst.id, removedStatus);
        }
      } else {
        statuses.set(id, removedStatus);
      }
      set({ updateStatuses: statuses });
      return true;
    }
    // Remove update status for this extension and all siblings in the same group
    // (backend updates all siblings, so clear them all from the UI)
    const statuses = new Map(get().updateStatuses);
    const group = get()
      .grouped()
      .find((g) => g.instances.some((i) => i.id === id));
    if (group) {
      for (const inst of group.instances) {
        statuses.delete(inst.id);
      }
    } else {
      statuses.delete(id);
    }
    set({ updateStatuses: statuses });
    await get().rescanAndFetch();
    return false;
  },

  async updateAll() {
    // Deduplicate: only update one instance per group (same skill across agents)
    const groups = get().grouped();
    const updateStatuses = get().updateStatuses;
    const toUpdate: { groupName: string; id: string; siblingIds: string[] }[] =
      [];
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
      const skippedNames: string[] = [];
      for (const { groupName, id, siblingIds } of toUpdate) {
        try {
          const result = await api.updateExtension(id);
          if (result.skipped) {
            skippedNames.push(groupName);
            // Set removed_from_repo status for all siblings
            const statuses = new Map(get().updateStatuses);
            const removedStatus = { status: "removed_from_repo" as const };
            for (const sid of siblingIds) {
              statuses.set(sid, removedStatus);
            }
            set({ updateStatuses: statuses });
            continue;
          }
          // Remove update status for all instances in the group
          const statuses = new Map(get().updateStatuses);
          for (const sid of siblingIds) {
            statuses.delete(sid);
          }
          set({ updateStatuses: statuses });
          updated++;
        } catch (e: unknown) {
          console.error("Failed to update extension:", e);
          const msg = e instanceof Error ? e.message : String(e);
          toast.error(`Failed to update ${groupName}: ${msg}`);
          // continue with remaining updates
        }
      }
      if (skippedNames.length > 0) {
        toast.info(
          skippedNames.length === 1
            ? `${skippedNames[0]} is no longer available in the remote repository`
            : `${skippedNames.join(", ")} are no longer available in their remote repositories`,
        );
      }
      await get().rescanAndFetch();
    } finally {
      set({ updatingAll: false });
    }
    return updated;
  },

  async installNewRepoSkills(
    url: string,
    skillIds: string[],
    targetAgents: string[],
  ) {
    await api.installNewRepoSkills(url, skillIds, targetAgents);
    // Remove installed skills from newRepoSkills
    set({
      newRepoSkills: get().newRepoSkills.filter(
        (s) => !(s.repo_url === url && skillIds.includes(s.skill_id)),
      ),
    });
    await get().rescanAndFetch();
  },

  async deleteFromAgents(groupKey, agentNames) {
    const group = get()
      .grouped()
      .find((g) => g.groupKey === groupKey);
    if (!group) return;

    let toDelete: typeof group.instances;

    if (group.kind === "cli") {
      // CLI uninstall: no optimistic removal, no undo — execute directly
      // so the dialog stays visible with a spinner during the operation.
      const children = findCliChildren(
        get().extensions,
        group.instances[0]?.id,
        group.pack,
      );
      toDelete = [...children, ...group.instances];
      const ids = toDelete.map((e) => e.id);
      await Promise.all(ids.map((id) => api.deleteExtension(id)));
      // Remove CLI binary
      for (const ext of group.instances) {
        if (ext.cli_meta?.binary_path) {
          await api
            .uninstallCliBinary(ext.cli_meta.binary_path)
            .catch((e) => console.error("Failed to remove CLI binary:", e));
        }
      }
      await get().rescanAndFetch();
      return;
    }

    toDelete = group.instances.filter((e) =>
      e.agents.some((a) => agentNames.includes(a)),
    );

    if (toDelete.length === 0) return;
    const ids = new Set(toDelete.map((e) => e.id));
    // Optimistic removal
    const isFullDelete = toDelete.length === group.instances.length;
    set((s) => ({
      extensions: s.extensions.filter((e) => !ids.has(e.id)),
      selectedId: isFullDelete ? null : s.selectedId,
    }));
    const prev = get().pendingDelete;
    if (prev) {
      clearTimeout(prev.timer);
      try {
        await Promise.all([...prev.ids].map((id) => api.deleteExtension(id)));
      } catch (e) {
        console.error("Failed to finalize previous deletion:", e);
      }
    }
    const timer = setTimeout(() => {
      get().confirmDelete();
    }, 5000);
    set({ pendingDelete: { ids, extensions: toDelete, timer } });
  },

  grouped() {
    return getCachedGroups(get().extensions);
  },

  filtered() {
    const { searchQuery, tagFilter, packFilter, agentFilter, kindFilter } =
      get();
    return getCachedFiltered(
      get().grouped(),
      kindFilter,
      agentFilter,
      packFilter,
      tagFilter,
      searchQuery,
    );
  },
}));
