import { create } from "zustand";
import type { AgentInfo } from "@/lib/types";
import { api } from "@/lib/invoke";
import { toast } from "@/stores/toast-store";

interface AgentState {
  agents: AgentInfo[];
  loading: boolean;
  fetch: () => Promise<void>;
  updatePath: (name: string, path: string) => Promise<void>;
  setEnabled: (name: string, enabled: boolean) => Promise<void>;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  agents: [],
  loading: false,
  async fetch() {
    set({ loading: true });
    try {
      const agents = await api.listAgents();
      set({ agents, loading: false });
    } catch {
      set({ loading: false });
    }
  },
  async updatePath(name: string, path: string) {
    try {
      await api.updateAgentPath(name, path);
      set({
        agents: get().agents.map((a) =>
          a.name === name ? { ...a, path } : a
        ),
      });
      toast.success(`${name} path updated`);
    } catch {
      toast.error(`Failed to update ${name} path`);
    }
  },
  async setEnabled(name: string, enabled: boolean) {
    try {
      await api.setAgentEnabled(name, enabled);
      set({
        agents: get().agents.map((a) =>
          a.name === name ? { ...a, enabled } : a
        ),
      });
      toast.success(`${name} ${enabled ? "enabled" : "disabled"}`);
    } catch {
      toast.error(`Failed to update ${name}`);
    }
  },
}));
