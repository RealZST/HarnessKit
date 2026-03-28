import { create } from "zustand";

export type ThemeName = "tiesen" | "claude";
export type Mode = "system" | "dark" | "light";

interface UIState {
  sidebarOpen: boolean;
  themeName: ThemeName;
  mode: Mode;
  toggleSidebar: () => void;
  setThemeName: (name: ThemeName) => void;
  setMode: (mode: Mode) => void;
}

const storedMode = (typeof localStorage !== "undefined" && localStorage.getItem("hk-theme")) as Mode | null;
const storedThemeName = (typeof localStorage !== "undefined" && localStorage.getItem("hk-theme-name")) as ThemeName | null;

/** Resolve "system" to actual light/dark based on OS preference */
export function resolveMode(mode: Mode): "dark" | "light" {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  themeName: storedThemeName ?? "tiesen",
  mode: storedMode ?? "system",
  toggleSidebar() { set((s) => ({ sidebarOpen: !s.sidebarOpen })); },
  setThemeName(themeName) {
    localStorage.setItem("hk-theme-name", themeName);
    set({ themeName });
  },
  setMode(mode) {
    localStorage.setItem("hk-theme", mode);
    set({ mode });
  },
}));
