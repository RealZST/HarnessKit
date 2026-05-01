import { create } from "zustand";
import type { Project } from "@/lib/types";
import { toast } from "./toast-store";

const LS_KEY = "HK_SCOPE_LAST_USED";

export type ScopeValue =
  | { type: "all" }
  | { type: "global" }
  | { type: "project"; name: string; path: string };

interface ScopeState {
  current: ScopeValue;
  hydrated: boolean;
  setScope: (scope: ScopeValue) => void;
  hydrate: (urlScope: string | null, projects: Project[]) => void;
}

function parseUrlScope(
  urlScope: string | null,
  projects: Project[],
): ScopeValue | null {
  if (!urlScope) return null;
  if (urlScope === "all") {
    return projects.length > 0 ? { type: "all" } : null;
  }
  if (urlScope === "global") return { type: "global" };
  const proj = projects.find((p) => p.path === urlScope);
  if (proj) return { type: "project", name: proj.name, path: proj.path };
  return null;
}

function readLocalStorage(projects: Project[]): ScopeValue | null {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as ScopeValue;
    if (parsed.type === "global") return parsed;
    if (parsed.type === "all") {
      return projects.length > 0 ? parsed : null;
    }
    if (parsed.type === "project") {
      const proj = projects.find((p) => p.path === parsed.path);
      return proj
        ? { type: "project", name: proj.name, path: proj.path }
        : null;
    }
    return null;
  } catch {
    localStorage.removeItem(LS_KEY);
    return null;
  }
}

function writeLocalStorage(scope: ScopeValue) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(scope));
  } catch {
    // ignore (private mode / quota)
  }
}

export const useScopeStore = create<ScopeState>((set) => ({
  current: { type: "global" },
  hydrated: false,

  setScope(scope) {
    writeLocalStorage(scope);
    set({ current: scope });
  },

  hydrate(urlScope, projects) {
    const fromUrl = parseUrlScope(urlScope, projects);
    if (urlScope && !fromUrl) {
      toast.warning(`Scope '${urlScope}' not found, using fallback`);
    }
    // AppShell's URL-sync effect (Task 1) detects the mismatch between
    // the resolved scope and the URL and strips the invalid ?scope= via
    // setSearchParams({ replace: true }) — no extra code here.
    const fromLs = readLocalStorage(projects);
    const resolved: ScopeValue = fromUrl ?? fromLs ?? { type: "global" };
    writeLocalStorage(resolved);
    set({ current: resolved, hydrated: true });
  },
}));
