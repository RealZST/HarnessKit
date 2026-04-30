import { create } from "zustand";
import { api } from "@/lib/invoke";
import type { Project } from "@/lib/types";

interface ProjectState {
  projects: Project[];
  loading: boolean;
  loaded: boolean;

  loadProjects: () => Promise<void>;
  addProject: (path: string) => Promise<void>;
  removeProject: (id: string) => Promise<void>;
}

export const useProjectStore = create<ProjectState>((set) => ({
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
    await api.removeProject(id);
    set((s) => ({ projects: s.projects.filter((p) => p.id !== id) }));
  },
}));
