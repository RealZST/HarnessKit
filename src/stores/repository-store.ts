import { create } from "zustand";
import { humanizeError } from "@/lib/errors";
import { api } from "@/lib/invoke";
import type {
  ConfigScope,
  InstallResult,
  RepoSyncResult,
  Repository,
  RepositoryWithItems,
} from "@/lib/types";
import { toast } from "@/stores/toast-store";
import { useExtensionStore } from "./extension-store";

interface RepositoryState {
  repositories: RepositoryWithItems[];
  loading: boolean;
  syncingRepoIds: Set<string>;
  isSyncingAll: boolean;

  fetch(): Promise<void>;
  addRepo(
    url: string,
    name?: string,
    branch?: string,
    authToken?: string,
  ): Promise<Repository>;
  deleteRepo(id: string): Promise<boolean>;
  syncRepo(id: string): Promise<RepoSyncResult>;
  syncAll(): Promise<void>;
  setItemStatus(
    repoId: string,
    itemId: string,
    status: "available" | "ignored",
    kind?: string,
  ): Promise<void>;
  installItem(
    repoId: string,
    itemId: string,
    targetAgents?: string[],
    targetScope?: ConfigScope,
    hermesCategory?: string,
  ): Promise<InstallResult[]>;
}

export const useRepositoryStore = create<RepositoryState>((set, get) => ({
  repositories: [],
  loading: false,
  syncingRepoIds: new Set<string>(),
  isSyncingAll: false,

  async fetch() {
    set({ loading: true });
    try {
      const repos = await api.listRepositories();
      set({ repositories: repos });
    } finally {
      set({ loading: false });
    }
  },

  async addRepo(
    url: string,
    name?: string,
    branch?: string,
    authToken?: string,
  ) {
    const repo = await api.addRepository(url, name, branch, authToken);
    // Background sync the newly added repo
    get()
      .syncRepo(repo.id)
      .catch((e) => {
        console.error("Initial repo sync failed:", e);
        toast.error(humanizeError(e));
      });
    await get().fetch();
    return repo;
  },

  async deleteRepo(id: string) {
    const success = await api.deleteRepository(id);
    if (success) {
      set((s) => ({
        repositories: s.repositories.filter((r) => r.repository.id !== id),
      }));
    }
    return success;
  },

  async syncRepo(id: string) {
    set((s) => {
      const next = new Set(s.syncingRepoIds);
      next.add(id);
      return { syncingRepoIds: next };
    });

    try {
      const result = await api.syncRepository(id);
      await get().fetch();
      return result;
    } finally {
      set((s) => {
        const next = new Set(s.syncingRepoIds);
        next.delete(id);
        return { syncingRepoIds: next };
      });
    }
  },

  async syncAll() {
    set({ isSyncingAll: true });
    try {
      await api.syncAllRepositories();
      await get().fetch();
    } finally {
      set({ isSyncingAll: false });
    }
  },

  async setItemStatus(
    repoId: string,
    itemId: string,
    status: "available" | "ignored",
    kind?: string,
  ) {
    await api.setRepositoryItemStatus(repoId, itemId, status, kind);
    set((s) => ({
      repositories: s.repositories.map((r) => {
        if (r.repository.id !== repoId) return r;
        return {
          ...r,
          items: r.items.map((item) => {
            if (item.item_id !== itemId) return item;
            return { ...item, status };
          }),
        };
      }),
    }));
  },

  async installItem(
    repoId: string,
    itemId: string,
    targetAgents?: string[],
    targetScope?: ConfigScope,
    hermesCategory?: string,
  ) {
    const results = await api.installFromRepository(
      repoId,
      itemId,
      targetAgents,
      targetScope,
      hermesCategory,
    );
    // Re-fetch extensions and repositories to update install status badges
    useExtensionStore
      .getState()
      .fetch()
      .catch((e) => console.error("Post install fetch failed:", e));
    await get().fetch();
    return results;
  },
}));
