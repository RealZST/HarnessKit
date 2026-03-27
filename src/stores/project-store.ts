import { create } from "zustand";
import type { Project, Extension } from "@/lib/types";
import { api } from "@/lib/invoke";

interface ProjectState {
  projects: Project[];
  selectedProject: Project | null;
  projectExtensions: Extension[];
  loading: boolean;
  extensionsLoading: boolean;

  loadProjects: () => Promise<void>;
  addProject: (path: string) => Promise<void>;
  removeProject: (id: string) => Promise<void>;
  selectProject: (project: Project | null) => void;
  loadExtensions: (projectPath: string) => Promise<void>;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  projects: [],
  selectedProject: null,
  projectExtensions: [],
  loading: false,
  extensionsLoading: false,

  async loadProjects() {
    set({ loading: true });
    try {
      const projects = await api.listProjects();
      set({ projects, loading: false });
    } catch {
      set({ loading: false });
    }
  },

  async addProject(path: string) {
    const project = await api.addProject(path);
    set((s) => ({ projects: [...s.projects, project] }));
  },

  async removeProject(id: string) {
    await api.removeProject(id);
    const { selectedProject } = get();
    set((s) => ({
      projects: s.projects.filter((p) => p.id !== id),
      selectedProject: selectedProject?.id === id ? null : selectedProject,
      projectExtensions: selectedProject?.id === id ? [] : s.projectExtensions,
    }));
  },

  selectProject(project: Project | null) {
    set({ selectedProject: project, projectExtensions: [] });
    if (project) {
      get().loadExtensions(project.path);
    }
  },

  async loadExtensions(projectPath: string) {
    set({ extensionsLoading: true });
    try {
      const projectExtensions = await api.getProjectExtensions(projectPath);
      set({ projectExtensions, extensionsLoading: false });
    } catch {
      set({ projectExtensions: [], extensionsLoading: false });
    }
  },
}));
