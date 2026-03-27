import { create } from "zustand";

export type ThemeName = "tiesen" | "astrovista" | "claude" | "lightgreen";
export type Mode = "dark" | "light";

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

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  themeName: storedThemeName ?? "tiesen",
  mode: storedMode ?? "dark",
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
