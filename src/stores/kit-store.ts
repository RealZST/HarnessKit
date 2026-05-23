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
import { useProjectStore } from "./project-store";

interface KitState {
  kits: KitSummary[];
  details: KitDetails | null;
  candidates: KitAssetCandidates | null;
  installRecords: ProjectInstallRecords[];

  fetchKits(): Promise<void>;
  fetchDetails(id: string): Promise<void>;
  fetchCandidates(): Promise<void>;
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
  async fetchCandidates() {
    const candidates = await api.listKitAssetCandidates();
    set({ candidates });
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
