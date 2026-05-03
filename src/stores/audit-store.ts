import { create } from "zustand";
import { api } from "@/lib/invoke";
import type { AuditResult, TrustTier } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

const MIN_AUDIT_LOADING_VISIBLE_MS = 600;

interface AuditState {
  results: AuditResult[];
  loading: boolean;
  searchQuery: string;
  tierFilter: TrustTier | null;
  setSearchQuery: (q: string) => void;
  setTierFilter: (t: TrustTier | null) => void;
  loadCached: () => Promise<void>;
  runAudit: () => Promise<void>;
}

export const useAuditStore = create<AuditState>((set) => ({
  results: [],
  loading: false,
  searchQuery: "",
  tierFilter: null,
  setSearchQuery: (q) => set({ searchQuery: q }),
  setTierFilter: (t) => set({ tierFilter: t }),
  async loadCached() {
    try {
      const results = await api.listAuditResults();
      set({ results });
    } catch (e) {
      console.error("Failed to load cached audit results:", e);
    }
  },
  async runAudit() {
    const startedAt = Date.now();
    set({ loading: true });
    // Yield to let the browser paint loading state before Tauri IPC call
    await new Promise((r) => setTimeout(r, 50));
    try {
      const results = await api.runAudit();
      set({ results });
      // Refresh extensions so trust_score updates in the Extensions page
      useExtensionStore.getState().fetch();
      toast.success("Audit complete");
    } catch {
      toast.error("Audit failed");
    } finally {
      const remaining = MIN_AUDIT_LOADING_VISIBLE_MS - (Date.now() - startedAt);
      if (remaining > 0) {
        await new Promise((resolve) => setTimeout(resolve, remaining));
      }
      set({ loading: false });
    }
  },
}));
