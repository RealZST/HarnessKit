import { useEffect, useState } from "react";
import { HashRouter, Routes, Route } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppShell } from "./components/layout/app-shell";
import { useUIStore, resolveMode } from "./stores/ui-store";
import { useExtensionStore } from "./stores/extension-store";
import { useAuditStore } from "./stores/audit-store";
import { api } from "./lib/invoke";
import OverviewPage from "./pages/overview";
import ExtensionsPage from "./pages/extensions";
import AuditPage from "./pages/audit";
import SettingsPage from "./pages/settings";
import MarketplacePage from "./pages/marketplace";

export default function App() {
  const themeName = useUIStore((s) => s.themeName);
  const mode = useUIStore((s) => s.mode);
  const fetchExtensions = useExtensionStore((s) => s.fetch);
  const loadCachedAudit = useAuditStore((s) => s.loadCached);

  // Track resolved dark/light (reacts to OS changes when mode === "system")
  const [resolved, setResolved] = useState<"dark" | "light">(() => resolveMode(mode));

  useEffect(() => {
    setResolved(resolveMode(mode));

    if (mode !== "system") return;

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setResolved(mq.matches ? "dark" : "light");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  // Background scan
  useEffect(() => {
    api.scanAndSync()
      .catch(() => {})
      .then(() => {
        fetchExtensions();
        loadCachedAudit();
      });
  }, [fetchExtensions, loadCachedAudit]);

  // Apply theme + dark class to <html>, and sync window appearance for vibrancy
  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", themeName);
    if (resolved === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
    // Force macOS vibrancy to match — "light" | "dark" | null (system)
    getCurrentWindow().setTheme(mode === "system" ? null : resolved).catch(() => {});
  }, [themeName, mode, resolved]);

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="extensions" element={<ExtensionsPage />} />
          <Route path="marketplace" element={<MarketplacePage />} />
          <Route path="audit" element={<AuditPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
