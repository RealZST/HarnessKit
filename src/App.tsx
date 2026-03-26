import { useEffect } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { AppShell } from "./components/layout/app-shell";
import { useUIStore } from "./stores/ui-store";
import { api } from "./lib/invoke";
import OverviewPage from "./pages/overview";
import ExtensionsPage from "./pages/extensions";
import AuditPage from "./pages/audit";
import AgentsPage from "./pages/agents";
import SettingsPage from "./pages/settings";

export default function App() {
  const theme = useUIStore((s) => s.theme);
  // Scan extensions on app startup
  useEffect(() => {
    api.scanAndSync();
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "dark") {
      root.classList.add("dark");
      document.body.className = "bg-zinc-950 text-zinc-100";
    } else {
      root.classList.remove("dark");
      document.body.className = "bg-white text-zinc-900";
    }
  }, [theme]);

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<OverviewPage />} />
          <Route path="extensions" element={<ExtensionsPage />} />
          <Route path="audit" element={<AuditPage />} />
          <Route path="agents" element={<AgentsPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
