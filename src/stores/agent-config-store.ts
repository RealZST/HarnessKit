import { create } from "zustand";
import { humanizeError } from "@/lib/errors";
import i18n from "@/lib/i18n";
import { api } from "@/lib/invoke";
import type { AgentDetail, ConfigScope } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";

interface AgentConfigState {
  agentDetails: AgentDetail[];
  selectedAgent: string | null;
  expandedFiles: Set<string>;
  previewCache: Map<string, string>;
  previewLoading: Set<string>;
  previewErrors: Map<string, string>;
  loading: boolean;
  /** Path of a config file the user navigated to from elsewhere (e.g. the
   * Overview "Agent Activity" widget). The detail page consumes this to
   * force-open the containing section + scroll/highlight the row, then
   * clears it. `null` means no pending focus. */
  pendingFocusFile: string | null;

  fetch: () => Promise<void>;
  selectAgent: (name: string) => void;
  expandFile: (path: string) => void;
  toggleFile: (path: string) => void;
  setPendingFocusFile: (path: string | null) => void;
  fetchPreview: (path: string) => Promise<string>;
  openInEditor: (path: string) => Promise<void>;
  revealInFinder: (path: string) => Promise<void>;
  copyPath: (path: string) => Promise<void>;
  addCustomPath: (
    agent: string,
    path: string,
    label: string,
    category: string,
    targetScope: ConfigScope,
  ) => Promise<void>;
  updateCustomPath: (
    id: number,
    path: string,
    label: string,
    category: string,
  ) => Promise<void>;
  removeCustomPath: (id: number) => Promise<void>;
}

export const useAgentConfigStore = create<AgentConfigState>((set, get) => ({
  agentDetails: [],
  selectedAgent: null,
  expandedFiles: new Set(),
  previewCache: new Map(),
  previewLoading: new Set(),
  previewErrors: new Map(),
  loading: false,
  pendingFocusFile: null,

  async fetch() {
    set({ loading: true });
    try {
      const agentDetails = await api.listAgentConfigs();
      // Sort by agent store order
      const order = useAgentStore.getState().agentOrder;
      const idx = new Map(order.map((n, i) => [n, i]));
      agentDetails.sort(
        (a, b) => (idx.get(a.name) ?? 99) - (idx.get(b.name) ?? 99),
      );

      const current = get().selectedAgent;
      const firstDetected = agentDetails.find((a) => a.detected)?.name ?? null;
      set({
        agentDetails,
        selectedAgent:
          current && agentDetails.some((a) => a.name === current)
            ? current
            : firstDetected,
        loading: false,
      });
    } catch (e) {
      console.error("Failed to fetch agent configs:", e);
      set({ loading: false });
    }
  },

  selectAgent(name: string) {
    set({ selectedAgent: name, expandedFiles: new Set() });
  },

  setPendingFocusFile(path: string | null) {
    set({ pendingFocusFile: path });
  },

  expandFile(path: string) {
    const expanded = new Set(get().expandedFiles);
    if (!expanded.has(path)) {
      expanded.add(path);
      if (!get().previewCache.has(path)) {
        get().fetchPreview(path);
      }
      set({ expandedFiles: expanded });
    }
  },

  toggleFile(path: string) {
    const expanded = new Set(get().expandedFiles);
    if (expanded.has(path)) {
      expanded.delete(path);
    } else {
      expanded.add(path);
      if (!get().previewCache.has(path)) {
        get().fetchPreview(path);
      }
    }
    set({ expandedFiles: expanded });
  },

  async fetchPreview(path: string) {
    const cached = get().previewCache.get(path);
    if (cached !== undefined) {
      return cached;
    }
    if (get().previewLoading.has(path)) {
      return "";
    }

    const loading = new Set(get().previewLoading);
    loading.add(path);
    const errors = new Map(get().previewErrors);
    errors.delete(path);
    set({ previewLoading: loading, previewErrors: errors });

    try {
      const content = await api.readConfigFilePreview(path, 30);
      const cache = new Map(get().previewCache);
      cache.set(path, content);
      set({ previewCache: cache });
      return content;
    } catch (error) {
      const nextErrors = new Map(get().previewErrors);
      nextErrors.set(path, humanizeError(error));
      set({ previewErrors: nextErrors });
      return "";
    } finally {
      const nextLoading = new Set(get().previewLoading);
      nextLoading.delete(path);
      set({ previewLoading: nextLoading });
    }
  },

  async openInEditor(path: string) {
    try {
      await api.openInSystem(path);
    } catch {
      toast.error(i18n.t("agents:toast.failedOpenFile"));
    }
  },

  async revealInFinder(path: string) {
    try {
      await api.revealInFileManager(path);
    } catch {
      toast.error(i18n.t("agents:toast.failedRevealInFinder"));
    }
  },

  async copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      toast.success(i18n.t("agents:toast.pathCopied"));
    } catch {
      toast.error(i18n.t("agents:toast.failedCopyPath"));
    }
  },

  async addCustomPath(agent, path, label, category, targetScope) {
    // Check if path already exists in auto-scanned config files
    const detail = get().agentDetails.find((a) => a.name === agent);
    if (detail) {
      const existing = detail.config_files.find(
        (f) => f.path === path && f.custom_id == null,
      );
      if (existing) {
        toast.error(i18n.t("agents:toast.pathAlreadyDetected"));
        return;
      }
      const customDup = detail.config_files.find(
        (f) => f.path === path && f.custom_id != null,
      );
      if (customDup) {
        toast.error(i18n.t("agents:toast.pathAlreadyAdded"));
        return;
      }
    }
    try {
      await api.addCustomConfigPath(agent, path, label, category, targetScope);
      toast.success(i18n.t("agents:toast.customPathAdded"));
      get().fetch();
    } catch (error) {
      toast.error(humanizeError(error));
    }
  },

  async updateCustomPath(id, path, label, category) {
    try {
      await api.updateCustomConfigPath(id, path, label, category);
      toast.success(i18n.t("agents:toast.customPathUpdated"));
      get().fetch();
    } catch (error) {
      toast.error(humanizeError(error));
    }
  },

  async removeCustomPath(id) {
    try {
      await api.removeCustomConfigPath(id);
      toast.success(i18n.t("agents:toast.customPathRemoved"));
      get().fetch();
    } catch (error) {
      toast.error(humanizeError(error));
    }
  },
}));
