import { create } from "zustand";
import type { AuditResult } from "@/lib/types";
import { api } from "@/lib/invoke";
import { toast } from "@/stores/toast-store";

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
      const issues = results.reduce((n, r) => n + r.findings.length, 0);
      toast.success(`Audit complete — ${issues} issue${issues === 1 ? "" : "s"} found`);
    } catch {
      set({ loading: false });
      toast.error("Audit failed");
    }
  },
}));
