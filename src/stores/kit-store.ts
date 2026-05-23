import { create } from "zustand";
import { api } from "@/lib/invoke";
import type {
  CreateKitRequest,
  KitAssetCandidates,
  KitConflictPreview,
  KitDetails,
  KitSummary,
  KitSyncResult,
  PreviewKitConflictsRequest,
  ProjectInstallRecords,
  SyncKitRequest,
  UnsyncKitRequest,
  UpdateKitRequest,
} from "@/types/kits";
import { getCachedGroups } from "./extension-helpers";
import { useProjectStore } from "./project-store";

// Dedupe concurrent fetchCandidates calls: the backend scan walks every
// extension × every agent's on-disk config, which can run 1-2s. The page
// prefetches it on mount in the background, but KitEditorDialog also calls
// fetchCandidates on its own mount — without dedupe both calls would fire
// in parallel and waste an IPC round-trip on the slow path. Callers pass
// `{ force: true }` (KitEditorDialog) when stale data is unacceptable;
// page prefetch leaves force off so a warm cache from a prior visit serves
// the next editor open instantly.
let candidatesInflight: Promise<void> | null = null;

interface KitState {
  kits: KitSummary[];
  details: KitDetails | null;
  candidates: KitAssetCandidates | null;
  installRecords: ProjectInstallRecords[];

  fetchKits(): Promise<void>;
  fetchDetails(id: string): Promise<void>;
  fetchCandidates(opts?: { force?: boolean }): Promise<void>;
  fetchInstallRecords(): Promise<void>;

  createKit(req: CreateKitRequest): Promise<KitSummary>;
  updateKit(req: UpdateKitRequest): Promise<KitSummary>;
  deleteKit(id: string): Promise<void>;
  exportKit(id: string, targetPath: string): Promise<void>;
  importKit(zipPath: string): Promise<KitSummary>;

  previewConflicts(
    req: PreviewKitConflictsRequest,
  ): Promise<KitConflictPreview>;
  syncKit(req: SyncKitRequest): Promise<KitSyncResult>;
  unsyncKit(req: UnsyncKitRequest): Promise<void>;
}

export const useKitStore = create<KitState>((set, get) => ({
  kits: [],
  details: null,
  candidates: null,
  installRecords: [],

  async fetchKits() {
    const kits = await api.listKits();
    set({ kits });
  },
  async fetchDetails(id) {
    const details = await api.getKitDetails(id);
    set({ details });
  },
  async fetchCandidates(opts) {
    if (!opts?.force && get().candidates) return;
    if (candidatesInflight) return candidatesInflight;
    candidatesInflight = (async () => {
      try {
        const candidates = await api.listKitAssetCandidates();
        // Pre-warm the group cache while we're already in the slow path.
        // The user is waiting for the backend scan anyway (idle prefetch
        // or KitEditorDialog mount), so spending ~50ms more grouping
        // here means EditorAssetTab can render with a cache hit — no
        // buildGroups cost on the dialog-open paint.
        getCachedGroups(candidates.extensions);
        set({ candidates });
      } finally {
        candidatesInflight = null;
      }
    })();
    return candidatesInflight;
  },
  async fetchInstallRecords() {
    const installRecords = await api.listProjectInstallRecords();
    set({ installRecords });
  },

  async createKit(req) {
    const summary = await api.createKit(req);
    await get().fetchKits();
    return summary;
  },
  async updateKit(req) {
    const summary = await api.updateKit(req);
    await get().fetchKits();
    if (get().details?.summary.id === req.id) await get().fetchDetails(req.id);
    return summary;
  },
  async deleteKit(id) {
    await api.deleteKit(id);
    await get().fetchKits();
    await get().fetchInstallRecords();
  },
  async exportKit(id, targetPath) {
    await api.exportKit(id, targetPath);
  },
  async importKit(zipPath) {
    const summary = await api.importKit(zipPath);
    await get().fetchKits();
    return summary;
  },

  async previewConflicts(req) {
    return await api.previewKitProjectConflicts(req);
  },
  async syncKit(req) {
    const result = await api.syncKitToProject(req);
    await get().fetchKits();
    if (get().details?.summary.id === req.kit_id)
      await get().fetchDetails(req.kit_id);
    await get().fetchInstallRecords();
    // Sync may have auto-registered a brand-new project (drag-to-empty-folder
    // path). Refresh useProjectStore so the new project appears on the page
    // without forcing a reload. Swallow failures — the install itself
    // succeeded; the projects list will catch up on next mount.
    try {
      await useProjectStore.getState().loadProjects();
    } catch (e) {
      console.error("loadProjects after syncKit failed:", e);
    }
    return result;
  },
  async unsyncKit(req) {
    await api.unsyncKitFromProject(req);
    await get().fetchKits();
    if (get().details?.summary.id === req.kit_id)
      await get().fetchDetails(req.kit_id);
    await get().fetchInstallRecords();
  },
}));
