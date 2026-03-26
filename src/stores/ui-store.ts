import { create } from "zustand";

interface UIState {
  sidebarOpen: boolean;
  theme: "dark" | "light";
  toggleSidebar: () => void;
  setTheme: (theme: "dark" | "light") => void;
}

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  theme: "dark",
  toggleSidebar() { set((s) => ({ sidebarOpen: !s.sidebarOpen })); },
  setTheme(theme) { set({ theme }); },
}));
