import { create } from "zustand";
import { api } from "@/lib/invoke";
import type { Project } from "@/lib/types";
import { useExtensionStore } from "./extension-store";
import { useScopeStore } from "./scope-store";
import { toast } from "./toast-store";

interface ProjectState {
  projects: Project[];
  loading: boolean;
  loaded: boolean;

  loadProjects: () => Promise<void>;
  addProject: (path: string) => Promise<void>;
  removeProject: (id: string) => Promise<void>;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  loading: false,
  loaded: false,

  async loadProjects() {
    set({ loading: true });
    try {
      const projects = await api.listProjects();
      set({ projects, loading: false, loaded: true });
    } catch (e) {
      console.error("Failed to load projects:", e);
      set({ loading: false, loaded: true });
    }
  },

  async addProject(path: string) {
    const project = await api.addProject(path);
    set((s) => ({ projects: [...s.projects, project] }));
    // Discover the new project's extensions and refresh the in-memory
    // list. Without this, web-mode users see no extensions for the
    // newly-added project until they refresh the page (desktop relies on
    // the Tauri `extensions-changed` event, which has no web equivalent).
    try {
      await api.scanAndSync();
    } catch (e) {
      console.error("Failed to scan after adding project:", e);
    }
    await useExtensionStore.getState().fetch();
  },

  async removeProject(id: string) {
    const project = get().projects.find((p) => p.id === id);
    await api.removeProject(id);
    set((s) => ({ projects: s.projects.filter((p) => p.id !== id) }));
    if (project) {
      const scope = useScopeStore.getState().current;
      if (scope.type === "project" && scope.path === project.path) {
        useScopeStore.getState().setScope({ type: "global" });
        toast.warning(
          `Project '${project.name}' was removed, switched to Global`,
        );
      }
    }
    // Backend cascades the project's extension rows on delete, so refresh
    // the in-memory list to drop the now-stale entries (web mode has no
    // event channel for this; see addProject above).
    await useExtensionStore.getState().fetch();
  },
}));
