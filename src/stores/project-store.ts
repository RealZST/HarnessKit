import { create } from "zustand";
import { api } from "@/lib/invoke";
import type { Project } from "@/lib/types";
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
  },
}));
