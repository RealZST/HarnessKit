import { create } from "zustand";
import type { AgentDetail } from "@/lib/types";
import { api } from "@/lib/invoke";
import { toast } from "@/stores/toast-store";
import { useAgentStore } from "@/stores/agent-store";

interface AgentConfigState {
  agentDetails: AgentDetail[];
  selectedAgent: string | null;
  expandedFiles: Set<string>;
  previewCache: Map<string, string>;
  loading: boolean;

  fetch: () => Promise<void>;
  selectAgent: (name: string) => void;
  expandFile: (path: string) => void;
  toggleFile: (path: string) => void;
  fetchPreview: (path: string) => Promise<string>;
  openInEditor: (path: string) => Promise<void>;
  copyPath: (path: string) => Promise<void>;
  addCustomPath: (agent: string, path: string, label: string, category: string) => Promise<void>;
  updateCustomPath: (id: number, path: string, label: string, category: string) => Promise<void>;
  removeCustomPath: (id: number) => Promise<void>;
}

export const useAgentConfigStore = create<AgentConfigState>((set, get) => ({
  agentDetails: [],
  selectedAgent: null,
  expandedFiles: new Set(),
  previewCache: new Map(),
  loading: false,

  async fetch() {
    set({ loading: true });
    try {
      const agentDetails = await api.listAgentConfigs();
      // Sort by agent store order
      const order = useAgentStore.getState().agentOrder;
      const idx = new Map(order.map((n, i) => [n, i]));
      agentDetails.sort((a, b) => (idx.get(a.name) ?? 99) - (idx.get(b.name) ?? 99));

      const current = get().selectedAgent;
      const firstDetected = agentDetails.find((a) => a.detected)?.name ?? null;
      set({
        agentDetails,
        selectedAgent: current && agentDetails.some((a) => a.name === current) ? current : firstDetected,
        loading: false,
      });
    } catch {
      set({ loading: false });
    }
  },

  selectAgent(name: string) {
    set({ selectedAgent: name, expandedFiles: new Set() });
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
    if (get().previewCache.has(path)) {
      return get().previewCache.get(path)!;
    }
    try {
      const content = await api.readConfigFilePreview(path, 30);
      const cache = new Map(get().previewCache);
      cache.set(path, content);
      set({ previewCache: cache });
      return content;
    } catch {
      return "";
    }
  },

  async openInEditor(path: string) {
    try {
      await api.openInSystem(path);
    } catch {
      toast.error("Failed to open file");
    }
  },

  async copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      toast.success("Path copied");
    } catch {
      toast.error("Failed to copy path");
    }
  },

  async addCustomPath(agent, path, label, category) {
    // Check if path already exists in auto-scanned config files
    const detail = get().agentDetails.find((a) => a.name === agent);
    if (detail) {
      const existing = detail.config_files.find((f) => f.path === path && f.custom_id == null);
      if (existing) {
        toast.error("This path is already detected automatically");
        return;
      }
      const customDup = detail.config_files.find((f) => f.path === path && f.custom_id != null);
      if (customDup) {
        toast.error("This path has already been added");
        return;
      }
    }
    try {
      await api.addCustomConfigPath(agent, path, label, category);
      toast.success("Custom path added");
      get().fetch();
    } catch {
      toast.error("Failed to add custom path");
    }
  },

  async updateCustomPath(id, path, label, category) {
    try {
      await api.updateCustomConfigPath(id, path, label, category);
      toast.success("Custom path updated");
      get().fetch();
    } catch {
      toast.error("Failed to update custom path");
    }
  },

  async removeCustomPath(id) {
    try {
      await api.removeCustomConfigPath(id);
      toast.success("Custom path removed");
      get().fetch();
    } catch {
      toast.error("Failed to remove custom path");
    }
  },
}));
