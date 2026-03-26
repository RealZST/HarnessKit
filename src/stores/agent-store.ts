import { create } from "zustand";
import type { AgentInfo } from "@/lib/types";
import { api } from "@/lib/invoke";

interface AgentState {
  agents: AgentInfo[];
  loading: boolean;
  fetch: () => Promise<void>;
}

export const useAgentStore = create<AgentState>((set) => ({
  agents: [],
  loading: false,
  async fetch() {
    set({ loading: true });
    const agents = await api.listAgents();
    set({ agents, loading: false });
  },
}));
