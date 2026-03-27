import { create } from "zustand";
import type { AuditResult } from "@/lib/types";
import { api } from "@/lib/invoke";

interface AuditState {
  results: AuditResult[];
  loading: boolean;
  loadCached: () => Promise<void>;
  runAudit: () => Promise<void>;
}

export const useAuditStore = create<AuditState>((set) => ({
  results: [],
  loading: false,
  async loadCached() {
    try {
      const results = await api.listAuditResults();
      set({ results });
    } catch {
      // No cached results — that's fine
    }
  },
  async runAudit() {
    set({ loading: true });
    try {
      const results = await api.runAudit();
      set({ results, loading: false });
    } catch {
      set({ loading: false });
    }
  },
}));
