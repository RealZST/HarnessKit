import { create } from "zustand";
import type { Extension, ExtensionKind } from "@/lib/types";
import { api } from "@/lib/invoke";

interface ExtensionState {
  extensions: Extension[];
  loading: boolean;
  kindFilter: ExtensionKind | null;
  agentFilter: string | null;
  sortBy: "installed_at" | "name" | "trust_score";
  fetch: () => Promise<void>;
  setKindFilter: (kind: ExtensionKind | null) => void;
  setAgentFilter: (agent: string | null) => void;
  setSortBy: (sort: "installed_at" | "name" | "trust_score") => void;
  toggle: (id: string, enabled: boolean) => Promise<void>;
}

export const useExtensionStore = create<ExtensionState>((set, get) => ({
  extensions: [],
  loading: false,
  kindFilter: null,
  agentFilter: null,
  sortBy: "installed_at",
  async fetch() {
    set({ loading: true });
    const extensions = await api.listExtensions(
      get().kindFilter ?? undefined,
      get().agentFilter ?? undefined,
    );
    set({ extensions, loading: false });
  },
  setKindFilter(kind) { set({ kindFilter: kind }); get().fetch(); },
  setAgentFilter(agent) { set({ agentFilter: agent }); get().fetch(); },
  setSortBy(sortBy) { set({ sortBy }); },
  async toggle(id, enabled) {
    await api.toggleExtension(id, enabled);
    get().fetch();
  },
}));
