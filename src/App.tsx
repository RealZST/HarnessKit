import { useEffect } from "react";
import { HashRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "./components/layout/app-shell";
import { useUIStore } from "./stores/ui-store";
import { api } from "./lib/invoke";
import OverviewPage from "./pages/overview";
import ExtensionsPage from "./pages/extensions";
import AuditPage from "./pages/audit";
import AgentsPage from "./pages/agents";
import SettingsPage from "./pages/settings";
import MarketplacePage from "./pages/marketplace";

export default function App() {
  const themeName = useUIStore((s) => s.themeName);
  const mode = useUIStore((s) => s.mode);

  // Scan extensions on app startup
  useEffect(() => {
    api.scanAndSync();
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.setAttribute("data-theme", themeName);
    if (mode === "dark") {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
  }, [themeName, mode]);

  return (
    <HashRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="extensions" element={<ExtensionsPage />} />
          <Route path="marketplace" element={<MarketplacePage />} />
          <Route path="audit" element={<AuditPage />} />
          <Route path="agents" element={<AgentsPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </HashRouter>
  );
}
