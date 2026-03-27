import { create } from "zustand";
import type { Extension, ExtensionKind, UpdateStatus } from "@/lib/types";
import { api } from "@/lib/invoke";

interface ExtensionState {
  extensions: Extension[];
  loading: boolean;
  kindFilter: ExtensionKind | null;
  agentFilter: string | null;
  searchQuery: string;
  selectedId: string | null;
  selectedIds: Set<string>;
  sortBy: "installed_at" | "name" | "trust_score";
  updateStatuses: Map<string, UpdateStatus>;
  allTags: string[];
  tagFilter: string | null;
  categoryFilter: string | null;
  fetch: () => Promise<void>;
  setKindFilter: (kind: ExtensionKind | null) => void;
  setAgentFilter: (agent: string | null) => void;
  setSearchQuery: (query: string) => void;
  setSelectedId: (id: string | null) => void;
  toggleSelected: (id: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
  setSortBy: (sort: "installed_at" | "name" | "trust_score") => void;
  setTagFilter: (tag: string | null) => void;
  setCategoryFilter: (category: string | null) => void;
  fetchTags: () => Promise<void>;
  updateTags: (id: string, tags: string[]) => Promise<void>;
  updateCategory: (id: string, category: string | null) => Promise<void>;
  deployToAgent: (id: string, targetAgent: string) => Promise<void>;
  toggle: (id: string, enabled: boolean) => Promise<void>;
  batchToggle: (enabled: boolean) => Promise<void>;
  batchDelete: () => Promise<void>;
  checkUpdates: () => Promise<void>;
  filtered: () => Extension[];
}

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
  async fetch() {
    set({ loading: true });
    try {
      const extensions = await api.listExtensions(
        get().kindFilter ?? undefined,
        get().agentFilter ?? undefined,
      );
      set({ extensions, loading: false });
      get().fetchTags();
    } catch {
      set({ loading: false });
    }
  },
  setKindFilter(kind) { set({ kindFilter: kind }); get().fetch(); },
  setAgentFilter(agent) { set({ agentFilter: agent }); get().fetch(); },
  setSearchQuery(query) { set({ searchQuery: query }); },
  setSelectedId(id) { set({ selectedId: id }); },
  toggleSelected(id) {
    const s = new Set(get().selectedIds);
    if (s.has(id)) s.delete(id); else s.add(id);
    set({ selectedIds: s });
  },
  selectAll() {
    const ids = new Set(get().filtered().map((e) => e.id));
    set({ selectedIds: ids });
  },
  clearSelection() { set({ selectedIds: new Set() }); },
  setSortBy(sortBy) { set({ sortBy }); },
  setTagFilter(tag) { set({ tagFilter: tag }); },
  setCategoryFilter(category) { set({ categoryFilter: category }); },
  async fetchTags() {
    const allTags = await api.getAllTags();
    set({ allTags });
  },
  async updateTags(id, tags) {
    await api.updateTags(id, tags);
    set((s) => ({
      extensions: s.extensions.map((e) => e.id === id ? { ...e, tags } : e),
    }));
    get().fetchTags();
  },
  async updateCategory(id, category) {
    await api.updateCategory(id, category);
    set((s) => ({
      extensions: s.extensions.map((e) => e.id === id ? { ...e, category } : e),
    }));
  },
  async deployToAgent(id, targetAgent) {
    await api.deployToAgent(id, targetAgent);
    get().fetch();
  },
  async toggle(id, enabled) {
    await api.toggleExtension(id, enabled);
    get().fetch();
  },
  async batchToggle(enabled) {
    for (const id of get().selectedIds) {
      await api.toggleExtension(id, enabled);
    }
    set({ selectedIds: new Set() });
    get().fetch();
  },
  async batchDelete() {
    for (const id of get().selectedIds) {
      await api.deleteExtension(id);
    }
    set({ selectedIds: new Set() });
    get().fetch();
  },
  async checkUpdates() {
    const results = await api.checkUpdates();
    const map = new Map<string, UpdateStatus>();
    for (const [id, status] of results) {
      map.set(id, status);
    }
    set({ updateStatuses: map });
  },
  filtered() {
    const { extensions, searchQuery, tagFilter, categoryFilter } = get();
    let result = extensions;
    if (categoryFilter) {
      result = result.filter((e) => e.category === categoryFilter);
    }
    if (tagFilter) {
      result = result.filter((e) => e.tags.includes(tagFilter));
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (e) =>
          e.name.toLowerCase().includes(q) ||
          e.description.toLowerCase().includes(q) ||
          e.agents.some((a) => a.toLowerCase().includes(q)) ||
          e.tags.some((t) => t.toLowerCase().includes(q))
      );
    }
    return result;
  },
}));
