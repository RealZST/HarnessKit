import { create } from "zustand";
import { api } from "@/lib/invoke";
import type {
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
    targetAgent?: string,
  ) => Promise<InstallResult>;
}

/** Background pre-fetch preview + audit for trending skill items */
function prefetchSkillData(
  items: MarketplaceItem[],
  get: () => MarketplaceState,
) {
  for (const item of items) {
    if (item.kind !== "skill") continue;
    const { previewCache, auditCache } = get();
    if (previewCache.has(item.id) && auditCache.has(item.id)) continue;

    const doFetch = (
      source: string,
      skillId: string,
      gitUrl?: string | null,
    ) => {
      if (!get().previewCache.has(item.id)) {
        api
          .fetchSkillPreview(source, skillId, gitUrl)
          .then((content) => {
            get().previewCache.set(item.id, content);
          })
          .catch(() => {
            get().previewCache.set(item.id, null);
          });
      }
      if (!get().auditCache.has(item.id)) {
        api
          .fetchSkillAudit(source, skillId)
          .then((info) => {
            get().auditCache.set(item.id, info);
          })
          .catch(() => {
            get().auditCache.set(item.id, null);
          });
      }
    };

    if (item.source && item.skill_id && item.skill_id.length > 0) {
      doFetch(item.source, item.skill_id, item.repo_url);
    } else {
      api
        .searchMarketplace(item.name, "skill", 5)
        .then((results) => {
          const match =
            results.find(
              (r) => r.source === item.source && r.name === item.name,
            ) ??
            results.find((r) => r.source === item.source) ??
            results.find((r) => r.name === item.name);
          if (match) {
            doFetch(match.source, match.skill_id, item.repo_url);
          } else if (item.source) {
            doFetch(item.source, "", item.repo_url);
          }
        })
        .catch(() => {
          if (item.source) doFetch(item.source, "", item.repo_url);
        });
    }
  }
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
    try {
      if (tab === "cli") {
        const trending = await api.listCliMarketplace();
        set({
          trending,
          trendingLoading: false,
          trendingCache: { ...get().trendingCache, cli: trending },
          trendingFetchedAt: { ...get().trendingFetchedAt, cli: Date.now() },
        });
        return;
      }
      const trending = await api.trendingMarketplace(tab, 10);
      set({
        trending,
        trendingLoading: false,
        trendingCache: { ...get().trendingCache, [tab]: trending },
        trendingFetchedAt: { ...get().trendingFetchedAt, [tab]: Date.now() },
      });
      // Pre-fetch preview + audit for skill items in background
      if (tab === "skill") {
        prefetchSkillData(trending, get);
      }
    } catch (e) {
      console.error("Failed to load marketplace trending data:", e);
      set({ trending: [], trendingLoading: false });
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
          get().cliReadmeCache.set(item.source, content);
          if (get().selectedItem?.id === expectedId) {
            set({ cliReadme: content, cliReadmeLoading: false });
          }
        })
        .catch(() => {
          get().cliReadmeCache.set(item.source, null);
          if (get().selectedItem?.id === expectedId) {
            set({ cliReadme: null, cliReadmeLoading: false });
          }
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
            get().previewCache.set(item.id, content);
            if (get().selectedItem?.id === expectedId) {
              set({ previewContent: content, previewLoading: false });
            }
          })
          .catch(() => {
            get().previewCache.set(item.id, null);
            if (get().selectedItem?.id === expectedId) {
              set({ previewContent: null, previewLoading: false });
            }
          });
      }
      if (!hasAudit) {
        api
          .fetchSkillAudit(source, skill_id)
          .then((auditInfo) => {
            get().auditCache.set(item.id, auditInfo);
            if (get().selectedItem?.id === expectedId) {
              set({ auditInfo, auditLoading: false });
            }
          })
          .catch(() => {
            get().auditCache.set(item.id, null);
            if (get().selectedItem?.id === expectedId) {
              set({ auditInfo: null, auditLoading: false });
            }
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
  async install(item, targetAgent) {
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
      );
      set({ installing: null });
      return result;
    } catch (e) {
      set({ installing: null });
      throw e;
    }
  },
}));
