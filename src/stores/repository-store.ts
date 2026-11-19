import { create } from "zustand";
import { withConcurrency } from "@/lib/concurrency";
import { api } from "@/lib/invoke";
import type {
  InstallRepositoryItemRequest,
  RepositoryWithItems,
} from "@/lib/types";
import { useExtensionStore } from "./extension-store";

interface RepositoryState {
  repositories: RepositoryWithItems[];
  /** False until the first fetch has returned. */
  loaded: boolean;
  syncingIds: Set<string>;
  /** Repositories waiting for their turn in a sync of all. */
  queuedIds: Set<string>;
  /** How far the running sync of all has come. */
  syncAllProgress: { done: number; total: number } | null;

  fetch(): Promise<void>;
  /** Fetch, then sync the repositories that were not synced for a day. For
   *  opening the page: rejects only when the fetch does, and sync failures
   *  show on their repositories. */
  fetchAndSyncStale(): Promise<void>;
  /** Track a repository. Resolves with its id once its first sync has
   *  listed its skills; a repository that cannot be synced is not kept. */
  add(url: string): Promise<string>;
  sync(id: string): Promise<void>;
  /** Sync every repository, or only those `only` picks, that is not syncing
   *  already, a few at a time. Resolves with the outcome, or null when there
   *  was nothing to start. */
  syncAll(
    only?: (repo: RepositoryWithItems) => boolean,
  ): Promise<{ total: number; failed: number } | null>;
  remove(id: string): Promise<void>;
  setIgnored(repoId: string, itemId: string, ignored: boolean): Promise<void>;
  install(req: InstallRepositoryItemRequest): Promise<void>;
}

/** Enough to finish dozens of repositories in seconds. Below the six
 *  connections a browser opens to one host, so the rest of the web UI keeps
 *  working meanwhile, and far from what a Git host takes for abuse. */
const SYNC_ALL_LIMIT = 4;

/** Syncs run side by side, so an older list can arrive after a newer one. */
let requestedFetch = 0;
let appliedFetch = 0;

/** When each repository was last synced from here, so one that keeps failing
 *  is not tried again on every visit. */
const attemptedAt = new Map<string, number>();

function isStale(repo: RepositoryWithItems) {
  const day = 24 * 60 * 60 * 1000;
  const synced = repo.last_synced_at ? Date.parse(repo.last_synced_at) : 0;
  return Date.now() - Math.max(synced, attemptedAt.get(repo.id) ?? 0) > day;
}

function withId(ids: Set<string>, id: string, present: boolean) {
  const next = new Set(ids);
  if (present) next.add(id);
  else next.delete(id);
  return next;
}

export const useRepositoryStore = create<RepositoryState>((set, get) => ({
  repositories: [],
  loaded: false,
  syncingIds: new Set(),
  queuedIds: new Set(),
  syncAllProgress: null,

  async fetch() {
    const request = ++requestedFetch;
    const repositories = await api.listRepositories();
    if (request > appliedFetch) {
      appliedFetch = request;
      set({ repositories, loaded: true });
    }
  },

  async fetchAndSyncStale() {
    await get().fetch();
    await get().syncAll(isStale);
  },

  async add(url) {
    const id = await api.addRepository(url);
    await get().fetch();
    return id;
  },

  async sync(id) {
    attemptedAt.set(id, Date.now());
    set((s) => ({ syncingIds: withId(s.syncingIds, id, true) }));
    try {
      await api.syncRepository(id);
    } finally {
      set((s) => ({ syncingIds: withId(s.syncingIds, id, false) }));
      // The sync's own outcome is the one to report.
      await get()
        .fetch()
        .catch(() => {});
    }
  },

  async syncAll(only = () => true) {
    const { repositories, syncingIds, syncAllProgress } = get();
    const ids = repositories
      .filter(only)
      .map((repo) => repo.id)
      .filter((id) => !syncingIds.has(id));
    if (syncAllProgress || ids.length === 0) return null;
    set({
      queuedIds: new Set(ids),
      syncAllProgress: { done: 0, total: ids.length },
    });
    let failed = 0;
    try {
      await withConcurrency(ids, SYNC_ALL_LIMIT, async (id) => {
        set((s) => ({ queuedIds: withId(s.queuedIds, id, false) }));
        await get()
          .sync(id)
          .catch(() => {
            failed += 1;
          });
        set((s) => ({
          syncAllProgress: s.syncAllProgress && {
            ...s.syncAllProgress,
            done: s.syncAllProgress.done + 1,
          },
        }));
      });
    } finally {
      set({ queuedIds: new Set(), syncAllProgress: null });
    }
    return { total: ids.length, failed };
  },

  async remove(id) {
    await api.deleteRepository(id);
    await get().fetch();
  },

  async setIgnored(repoId, itemId, ignored) {
    await api.ignoreRepositoryItem(repoId, itemId, ignored);
    await get().fetch();
  },

  async install(req) {
    await api.installRepositoryItem(req);
    await Promise.all([get().fetch(), useExtensionStore.getState().fetch()]);
  },
}));
