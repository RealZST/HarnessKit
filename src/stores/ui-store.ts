import { create } from "zustand";

export type ThemeName = "tiesen" | "claude";
export type Mode = "system" | "dark" | "light";
export type AppIcon = "icon-1" | "icon-2";

interface UIState {
  sidebarOpen: boolean;
  themeName: ThemeName;
  mode: Mode;
  appIcon: AppIcon;
  toggleSidebar: () => void;
  setThemeName: (name: ThemeName) => void;
  setMode: (mode: Mode) => void;
  setAppIcon: (icon: AppIcon) => void;
}

const storedMode = (typeof localStorage !== "undefined" && localStorage.getItem("hk-theme")) as Mode | null;
const storedThemeName = (typeof localStorage !== "undefined" && localStorage.getItem("hk-theme-name")) as ThemeName | null;
const storedAppIcon = (typeof localStorage !== "undefined" && localStorage.getItem("hk-app-icon")) as AppIcon | null;

/** Resolve "system" to actual light/dark based on OS preference */
export function resolveMode(mode: Mode): "dark" | "light" {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  themeName: storedThemeName ?? "tiesen",
  mode: storedMode ?? "system",
  appIcon: storedAppIcon ?? "icon-1",
  toggleSidebar() { set((s) => ({ sidebarOpen: !s.sidebarOpen })); },
  setThemeName(themeName) {
    localStorage.setItem("hk-theme-name", themeName);
    set({ themeName });
  },
  setMode(mode) {
    localStorage.setItem("hk-theme", mode);
    set({ mode });
  },
  setAppIcon(appIcon) {
    localStorage.setItem("hk-app-icon", appIcon);
    set({ appIcon });
  },
}));
