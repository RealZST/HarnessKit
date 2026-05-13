import { create } from "zustand";
import {
  type Language,
  setCurrentLanguage as setI18nLanguage,
} from "@/lib/i18n";

export type ThemeName = "tiesen" | "claude";
export type Mode = "system" | "dark" | "light";
export type AppIcon = "icon-1" | "icon-2";
export type { Language };

/**
 * Safely retrieves and validates a localStorage value against allowed values.
 * Falls back to the default if localStorage is unavailable or the value is invalid.
 */
function getValidItem<T extends string>(
  key: string,
  allowed: readonly T[],
  fallback: T,
): T {
  if (typeof localStorage === "undefined") return fallback;
  const val = localStorage.getItem(key);
  return val && (allowed as readonly string[]).includes(val)
    ? (val as T)
    : fallback;
}

interface UIState {
  sidebarOpen: boolean;
  themeName: ThemeName;
  mode: Mode;
  appIcon: AppIcon;
  language: Language;
  toggleSidebar: () => void;
  setThemeName: (name: ThemeName) => void;
  setMode: (mode: Mode) => void;
  setAppIcon: (icon: AppIcon) => void;
  setLanguage: (lang: Language) => void;
}

const ALLOWED_MODES: readonly Mode[] = ["system", "dark", "light"];
const ALLOWED_THEME_NAMES: readonly ThemeName[] = ["tiesen", "claude"];
const ALLOWED_APP_ICONS: readonly AppIcon[] = ["icon-1", "icon-2"];
const ALLOWED_LANGUAGES: readonly Language[] = ["en", "zh"];

const storedMode = getValidItem("hk-theme", ALLOWED_MODES, "system");
const storedThemeName = getValidItem(
  "hk-theme-name",
  ALLOWED_THEME_NAMES,
  "tiesen",
);
const storedAppIcon = getValidItem("hk-app-icon", ALLOWED_APP_ICONS, "icon-1");
const storedLanguage = getValidItem("hk-language", ALLOWED_LANGUAGES, "en");

// Initialize i18n language on load
setI18nLanguage(storedLanguage);

/** Resolve "system" to actual light/dark based on OS preference */
export function resolveMode(mode: Mode): "dark" | "light" {
  if (mode !== "system") return mode;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  themeName: storedThemeName,
  mode: storedMode,
  appIcon: storedAppIcon,
  language: storedLanguage,
  toggleSidebar() {
    set((s) => ({ sidebarOpen: !s.sidebarOpen }));
  },
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
  setLanguage(language) {
    localStorage.setItem("hk-language", language);
    setI18nLanguage(language);
    set({ language });
  },
}));
