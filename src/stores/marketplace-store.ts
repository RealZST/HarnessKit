import { create } from "zustand";
import type { MarketplaceItem, SkillAuditInfo } from "@/lib/types";
import { api } from "@/lib/invoke";

type TabKind = "skill" | "mcp";

interface MarketplaceState {
  tab: TabKind;
  query: string;
  results: MarketplaceItem[];
  trending: MarketplaceItem[];
  loading: boolean;
  trendingLoading: boolean;
  selectedItem: MarketplaceItem | null;
  previewContent: string | null;
  previewLoading: boolean;
  auditInfo: SkillAuditInfo | null;
  auditLoading: boolean;
  installing: string | null;
  setTab: (tab: TabKind) => void;
  setQuery: (query: string) => void;
  search: () => Promise<void>;
  loadTrending: () => Promise<void>;
  selectItem: (item: MarketplaceItem) => void;
  closePreview: () => void;
  install: (item: MarketplaceItem, targetAgent?: string) => Promise<string>;
}

export const useMarketplaceStore = create<MarketplaceState>((set, get) => ({
  tab: "skill",
  query: "",
  results: [],
  trending: [],
  loading: false,
  trendingLoading: false,
  selectedItem: null,
  previewContent: null,
  previewLoading: false,
  auditInfo: null,
  auditLoading: false,
  installing: null,
  setTab(tab) {
    set({ tab, results: [], query: "", selectedItem: null, trending: [] });
    get().loadTrending();
  },
  setQuery(query) { set({ query }); },
  async search() {
    const { query, tab } = get();
    if (query.length < 2) { set({ results: [] }); return; }
    set({ loading: true });
    try {
      const results = await api.searchMarketplace(query, tab);
      set({ results, loading: false });
    } catch {
      set({ results: [], loading: false });
    }
  },
  async loadTrending() {
    const { tab } = get();
    set({ trendingLoading: true });
    try {
      const trending = await api.trendingMarketplace(tab, 10);
      set({ trending, trendingLoading: false });
    } catch {
      set({ trending: [], trendingLoading: false });
    }
  },
  selectItem(item) {
    set({
      selectedItem: item,
      previewContent: null, previewLoading: item.kind === "skill",
      auditInfo: null, auditLoading: item.kind === "skill",
    });
    if (item.kind === "skill") {
      const expectedId = item.id;
      const resolve = (source: string, skill_id: string) => {
        api.fetchSkillPreview(source, skill_id)
          .then((content) => {
            if (get().selectedItem?.id === expectedId) {
              set({ previewContent: content, previewLoading: false });
            }
          })
          .catch(() => {
            if (get().selectedItem?.id === expectedId) {
              set({ previewContent: null, previewLoading: false });
            }
          });
        api.fetchSkillAudit(source, skill_id)
          .then((auditInfo) => {
            if (get().selectedItem?.id === expectedId) {
              set({ auditInfo, auditLoading: false });
            }
          })
          .catch(() => {
            if (get().selectedItem?.id === expectedId) {
              set({ auditInfo: null, auditLoading: false });
            }
          });
      };

      if (item.source && item.skill_id && item.skill_id.length > 0) {
        resolve(item.source, item.skill_id);
      } else {
        api.searchMarketplace(item.name, "skill", 5).then((results) => {
          if (get().selectedItem?.id !== expectedId) return;
          const match = results.find((r) => r.source === item.source && r.name === item.name)
            ?? results.find((r) => r.source === item.source)
            ?? results.find((r) => r.name === item.name);
          if (!match) {
            set({ previewLoading: false, auditLoading: false });
            return;
          }
          resolve(match.source, match.skill_id);
        }).catch(() => {
          if (get().selectedItem?.id === expectedId) {
            set({ previewLoading: false, auditLoading: false });
          }
        });
      }
    }
  },
  closePreview() { set({ selectedItem: null, previewContent: null, auditInfo: null }); },
  async install(item, targetAgent) {
    set({ installing: item.id });
    try {
      let { source, skill_id } = item;
      // If skill_id is empty (trending items), resolve via skills.sh first
      if (!skill_id || skill_id.length === 0) {
        const results = await api.searchMarketplace(item.name, "skill", 5);
        const match = results.find((r) => r.source === item.source && r.name === item.name)
          ?? results.find((r) => r.source === item.source)
          ?? results.find((r) => r.name === item.name);
        if (match) {
          source = match.source;
          skill_id = match.skill_id;
        } else {
          throw new Error("Could not resolve skill details for this trending item. Try searching for it directly.");
        }
      }
      const name = await api.installFromMarketplace(source, skill_id, targetAgent);
      set({ installing: null });
      return name;
    } catch (e) {
      set({ installing: null });
      throw e;
    }
  },
}));
