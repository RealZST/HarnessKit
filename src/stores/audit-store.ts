import { create } from "zustand";
import type { AuditResult } from "@/lib/types";
import { api } from "@/lib/invoke";

interface AuditState {
  results: AuditResult[];
  loading: boolean;
  runAudit: () => Promise<void>;
}

export const useAuditStore = create<AuditState>((set) => ({
  results: [],
  loading: false,
  async runAudit() {
    set({ loading: true });
    const results = await api.runAudit();
    set({ results, loading: false });
  },
}));
