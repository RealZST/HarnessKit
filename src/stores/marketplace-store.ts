import { create } from "zustand";
import { api } from "@/lib/invoke";
import type {
  ConfigScope,
  InstallResult,
  MarketplaceItem,
  SkillAuditInfo,
} from "@/lib/types";

type TabKind = "skill" | "mcp" | "cli";

const TRENDING_TTL = 5 * 60 * 1000; // 5 minutes

interface MarketplaceState {
  tab: TabKind;
  query: string;
  results: MarketplaceItem[];
  trending: MarketplaceItem[];
  trendingCache: Record<TabKind, MarketplaceItem[]>;
  trendingFetchedAt: Record<TabKind, number>;
  loading: boolean;
  trendingLoading: boolean;
  selectedItem: MarketplaceItem | null;
  previewContent: string | null;
  previewLoading: boolean;
  auditInfo: SkillAuditInfo | null;
  auditLoading: boolean;
  cliReadme: string | null;
  cliReadmeLoading: boolean;
  installing: string | null;
  /** Cache for skill preview content, keyed by item.id */
  previewCache: Map<string, string | null>;
  /** Cache for skill audit info, keyed by item.id */
  auditCache: Map<string, SkillAuditInfo | null>;
  /** Cache for CLI README content, keyed by source */
  cliReadmeCache: Map<string, string | null>;
  setTab: (tab: TabKind) => void;
  setQuery: (query: string) => void;
  search: () => Promise<void>;
  loadTrending: () => Promise<void>;
  selectItem: (item: MarketplaceItem) => void;
  closePreview: () => void;
  install: (
    item: MarketplaceItem,
    targetAgent: string | undefined,
    targetScope: ConfigScope,
  ) => Promise<InstallResult>;
}

/** Run async tasks with a concurrency limit */
async function withConcurrency<T>(
  items: T[],
  limit: number,
  fn: (item: T) => Promise<void>,
) {
  const executing: Promise<void>[] = [];
  for (const item of items) {
    const p = fn(item).then(() => {
      executing.splice(executing.indexOf(p), 1);
    });
    executing.push(p);
    if (executing.length >= limit) await Promise.race(executing);
  }
  await Promise.all(executing);
}

/** Background pre-fetch preview + audit for trending skill items */
function prefetchSkillData(
  items: MarketplaceItem[],
  get: () => MarketplaceState,
  set: (
    partial:
      | Partial<MarketplaceState>
      | ((s: MarketplaceState) => Partial<MarketplaceState>),
  ) => void,
) {
  const skillItems = items.filter((item) => {
    if (item.kind !== "skill") return false;
    const { previewCache, auditCache } = get();
    return !previewCache.has(item.id) || !auditCache.has(item.id);
  });

  const doFetch = (
    item: MarketplaceItem,
    source: string,
    skillId: string,
    gitUrl?: string | null,
  ) => {
    const promises: Promise<void>[] = [];
    if (!get().previewCache.has(item.id)) {
      promises.push(
        api
          .fetchSkillPreview(source, skillId, gitUrl)
          .then((content) => {
            set((s) => {
              const cache = new Map(s.previewCache);
              cache.set(item.id, content);
              return { previewCache: cache };
            });
          })
          .catch(() => {
            set((s) => {
              const cache = new Map(s.previewCache);
              cache.set(item.id, null);
              return { previewCache: cache };
            });
          }),
      );
    }
    if (!get().auditCache.has(item.id)) {
      promises.push(
        api
          .fetchSkillAudit(source, skillId)
          .then((info) => {
            set((s) => {
              const cache = new Map(s.auditCache);
              cache.set(item.id, info);
              return { auditCache: cache };
            });
          })
          .catch(() => {
            set((s) => {
              const cache = new Map(s.auditCache);
              cache.set(item.id, null);
              return { auditCache: cache };
            });
          }),
      );
    }
    return Promise.all(promises).then(() => {});
  };

  withConcurrency(skillItems, 3, async (item) => {
    if (item.source && item.skill_id && item.skill_id.length > 0) {
      await doFetch(item, item.source, item.skill_id, item.repo_url);
    } else {
      try {
        const results = await api.searchMarketplace(item.name, "skill", 5);
        const match =
          results.find(
            (r) => r.source === item.source && r.name === item.name,
          ) ??
          results.find((r) => r.source === item.source) ??
          results.find((r) => r.name === item.name);
        if (match) {
          await doFetch(item, match.source, match.skill_id, item.repo_url);
        } else if (item.source) {
          await doFetch(item, item.source, "", item.repo_url);
        }
      } catch {
        if (item.source) await doFetch(item, item.source, "", item.repo_url);
      }
    }
  });
}

export const useMarketplaceStore = create<MarketplaceState>((set, get) => ({
  tab: "skill",
  query: "",
  results: [],
  trending: [],
  trendingCache: { skill: [], mcp: [], cli: [] },
  trendingFetchedAt: { skill: 0, mcp: 0, cli: 0 },
  loading: false,
  trendingLoading: false,
  selectedItem: null,
  previewContent: null,
  previewLoading: false,
  auditInfo: null,
  auditLoading: false,
  cliReadme: null,
  cliReadmeLoading: false,
  installing: null,
  previewCache: new Map(),
  auditCache: new Map(),
  cliReadmeCache: new Map(),
  setTab(tab) {
    const { trendingCache } = get();
    set({
      tab,
      results: [],
      query: "",
      selectedItem: null,
      trending: trendingCache[tab],
    });
    get().loadTrending();
  },
  setQuery(query) {
    set({ query });
  },
  async search() {
    const { query, tab } = get();
    if (query.length < 2) {
      set({ results: [] });
      return;
    }
    set({ loading: true });
    try {
      if (tab === "cli") {
        const all = await api.listCliMarketplace();
        const q = query.toLowerCase();
        const results = all.filter(
          (i) =>
            i.name.toLowerCase().includes(q) ||
            i.description.toLowerCase().includes(q),
        );
        set({ results, loading: false });
        return;
      }
      const results = await api.searchMarketplace(query, tab);
      set({ results, loading: false });
    } catch (e) {
      console.error("Failed to search marketplace:", e);
      set({ results: [], loading: false });
    }
  },
  async loadTrending() {
    const { tab, trendingFetchedAt } = get();
    if (Date.now() - trendingFetchedAt[tab] < TRENDING_TTL) return;
    // Clear detail caches on refresh so stale data doesn't linger
    set({
      trendingLoading: true,
      previewCache: new Map(),
      auditCache: new Map(),
    });

    const cacheKey = `hk_trending_${tab}`;
    const saveToDisk = (items: MarketplaceItem[]) => {
      try {
        localStorage.setItem(cacheKey, JSON.stringify(items));
      } catch {}
    };
    const loadFromDisk = (): MarketplaceItem[] => {
      try {
        return JSON.parse(localStorage.getItem(cacheKey) ?? "[]");
      } catch {
        return [];
      }
    };

    try {
      if (tab === "cli") {
        const trending = await api.listCliMarketplace();
        saveToDisk(trending);
        set({
          trending,
          trendingLoading: false,
          trendingCache: { ...get().trendingCache, cli: trending },
          trendingFetchedAt: { ...get().trendingFetchedAt, cli: Date.now() },
        });
        return;
      }
      const trending = await api.trendingMarketplace(tab, 10);
      saveToDisk(trending);
      set({
        trending,
        trendingLoading: false,
        trendingCache: { ...get().trendingCache, [tab]: trending },
        trendingFetchedAt: { ...get().trendingFetchedAt, [tab]: Date.now() },
      });
      // Pre-fetch preview + audit for skill items in background
      if (tab === "skill") {
        prefetchSkillData(trending, get, set);
      }
    } catch (e) {
      console.error("Failed to load marketplace trending data:", e);
      // Fallback to last successful fetch from disk
      const cached = loadFromDisk();
      set({ trending: cached, trendingLoading: false });
    }
  },
  selectItem(item) {
    const { previewCache, auditCache, cliReadmeCache } = get();
    const hasPreview = previewCache.has(item.id);
    const hasAudit = auditCache.has(item.id);
    const hasCliReadme = cliReadmeCache.has(item.source);

    set({
      selectedItem: item,
      previewContent: hasPreview ? (previewCache.get(item.id) ?? null) : null,
      previewLoading: item.kind === "skill" && !hasPreview,
      auditInfo: hasAudit ? (auditCache.get(item.id) ?? null) : null,
      auditLoading: item.kind === "skill" && !hasAudit,
      cliReadme: hasCliReadme
        ? (cliReadmeCache.get(item.source) ?? null)
        : null,
      cliReadmeLoading: item.kind === "cli" && !hasCliReadme,
    });

    // Fetch CLI readme if needed
    if (item.kind === "cli" && !hasCliReadme && item.source) {
      const expectedId = item.id;
      api
        .fetchCliReadme(item.source)
        .then((content) => {
          set((s) => {
            const cache = new Map(s.cliReadmeCache);
            cache.set(item.source, content);
            const update: Partial<MarketplaceState> = { cliReadmeCache: cache };
            if (s.selectedItem?.id === expectedId) {
              update.cliReadme = content;
              update.cliReadmeLoading = false;
            }
            return update;
          });
        })
        .catch(() => {
          set((s) => {
            const cache = new Map(s.cliReadmeCache);
            cache.set(item.source, null);
            const update: Partial<MarketplaceState> = { cliReadmeCache: cache };
            if (s.selectedItem?.id === expectedId) {
              update.cliReadme = null;
              update.cliReadmeLoading = false;
            }
            return update;
          });
        });
    }

    // If both cached or not a skill, nothing more to do
    if (item.kind !== "skill" || (hasPreview && hasAudit)) return;

    const expectedId = item.id;
    const resolve = (
      source: string,
      skill_id: string,
      gitUrl?: string | null,
    ) => {
      if (!hasPreview) {
        api
          .fetchSkillPreview(source, skill_id, gitUrl)
          .then((content) => {
            set((s) => {
              const cache = new Map(s.previewCache);
              cache.set(item.id, content);
              const update: Partial<MarketplaceState> = { previewCache: cache };
              if (s.selectedItem?.id === expectedId) {
                update.previewContent = content;
                update.previewLoading = false;
              }
              return update;
            });
          })
          .catch(() => {
            set((s) => {
              const cache = new Map(s.previewCache);
              cache.set(item.id, null);
              const update: Partial<MarketplaceState> = { previewCache: cache };
              if (s.selectedItem?.id === expectedId) {
                update.previewContent = null;
                update.previewLoading = false;
              }
              return update;
            });
          });
      }
      if (!hasAudit) {
        api
          .fetchSkillAudit(source, skill_id)
          .then((auditInfo) => {
            set((s) => {
              const cache = new Map(s.auditCache);
              cache.set(item.id, auditInfo);
              const update: Partial<MarketplaceState> = { auditCache: cache };
              if (s.selectedItem?.id === expectedId) {
                update.auditInfo = auditInfo;
                update.auditLoading = false;
              }
              return update;
            });
          })
          .catch(() => {
            set((s) => {
              const cache = new Map(s.auditCache);
              cache.set(item.id, null);
              const update: Partial<MarketplaceState> = { auditCache: cache };
              if (s.selectedItem?.id === expectedId) {
                update.auditInfo = null;
                update.auditLoading = false;
              }
              return update;
            });
          });
      }
    };

    if (item.source && item.skill_id && item.skill_id.length > 0) {
      resolve(item.source, item.skill_id, item.repo_url);
    } else {
      api
        .searchMarketplace(item.name, "skill", 5)
        .then((results) => {
          if (get().selectedItem?.id !== expectedId) return;
          const match =
            results.find(
              (r) => r.source === item.source && r.name === item.name,
            ) ??
            results.find((r) => r.source === item.source) ??
            results.find((r) => r.name === item.name);
          if (match) {
            resolve(match.source, match.skill_id, item.repo_url);
          } else if (item.source) {
            // Fallback: try fetching directly with item.source and empty skill_id
            resolve(item.source, "", item.repo_url);
          } else {
            set({ previewLoading: false, auditLoading: false });
          }
        })
        .catch(() => {
          if (get().selectedItem?.id === expectedId) {
            if (item.source) {
              resolve(item.source, "", item.repo_url);
            } else {
              set({ previewLoading: false, auditLoading: false });
            }
          }
        });
    }
  },
  closePreview() {
    set({
      selectedItem: null,
      previewContent: null,
      auditInfo: null,
      cliReadme: null,
    });
  },
  async install(item, targetAgent, targetScope) {
    set({ installing: `${item.id}:${targetAgent ?? ""}` });
    try {
      let { source, skill_id } = item;
      // If skill_id is empty (trending items), resolve via skills.sh first
      if (!skill_id || skill_id.length === 0) {
        const results = await api.searchMarketplace(item.name, "skill", 5);
        const match =
          results.find(
            (r) => r.source === item.source && r.name === item.name,
          ) ??
          results.find((r) => r.source === item.source) ??
          results.find((r) => r.name === item.name);
        if (match) {
          source = match.source;
          skill_id = match.skill_id;
        } else {
          throw new Error(
            "Could not resolve skill details for this trending item. Try searching for it directly.",
          );
        }
      }
      const result = await api.installFromMarketplace(
        source,
        skill_id,
        targetAgent,
        targetScope,
      );
      set({ installing: null });
      return result;
    } catch (e) {
      set({ installing: null });
      throw e;
    }
  },
}));
