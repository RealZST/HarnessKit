import { create } from "zustand";
import i18n from "@/lib/i18n";
import { api } from "@/lib/invoke";
import { AGENT_ORDER, type AgentInfo, agentDisplayName } from "@/lib/types";
import { toast } from "@/stores/toast-store";

interface AgentState {
  agents: AgentInfo[];
  loading: boolean;
  /** Current agent order — derived from backend-returned agents array. */
  agentOrder: readonly string[];
  fetch: () => Promise<void>;
  updatePath: (name: string, path: string) => Promise<void>;
  setEnabled: (name: string, enabled: boolean) => Promise<void>;
  reorderAgents: (orderedNames: string[]) => Promise<void>;
}

export const useAgentStore = create<AgentState>((set, get) => ({
  agents: [],
  loading: false,
  agentOrder: AGENT_ORDER,
  async fetch() {
    set({ loading: true });
    try {
      const agents = await api.listAgents();
      // Backend returns agents already sorted by sort_order
      set({
        agents,
        agentOrder: agents.map((a) => a.name),
        loading: false,
      });
    } catch (e) {
      console.error("Failed to fetch agents:", e);
      set({ loading: false });
    }
  },
  async updatePath(name: string, path: string) {
    try {
      await api.updateAgentPath(name, path);
      set({
        agents: get().agents.map((a) => (a.name === name ? { ...a, path } : a)),
      });
      toast.success(
        i18n.t("agents:toast.pathUpdated", { agent: agentDisplayName(name) }),
      );
    } catch {
      toast.error(
        i18n.t("agents:toast.pathUpdateFailed", {
          agent: agentDisplayName(name),
        }),
      );
    }
  },
  async setEnabled(name: string, enabled: boolean) {
    try {
      await api.setAgentEnabled(name, enabled);
      set({
        agents: get().agents.map((a) =>
          a.name === name ? { ...a, enabled } : a,
        ),
      });
      toast.success(
        i18n.t(
          enabled ? "agents:toast.agentEnabled" : "agents:toast.agentDisabled",
          { agent: agentDisplayName(name) },
        ),
      );
    } catch {
      toast.error(
        i18n.t("agents:toast.updateFailed", { agent: agentDisplayName(name) }),
      );
    }
  },
  async reorderAgents(orderedNames: string[]) {
    // Optimistic update
    const agents = get().agents;
    const byName = new Map(agents.map((a) => [a.name, a]));
    const reordered = orderedNames
      .map((n) => byName.get(n))
      .filter(Boolean) as AgentInfo[];
    set({ agents: reordered, agentOrder: orderedNames });
    try {
      await api.updateAgentOrder(orderedNames);
    } catch {
      toast.error(i18n.t("agents:toast.failedSaveOrder"));
      // Revert on failure
      get().fetch();
    }
  },
}));
