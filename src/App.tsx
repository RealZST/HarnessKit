import { useEffect, useRef, useState } from "react";
import { HashRouter, Routes, Route } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppShell } from "./components/layout/app-shell";
import { useUIStore, resolveMode } from "./stores/ui-store";
import { useExtensionStore } from "./stores/extension-store";
import { useAuditStore } from "./stores/audit-store";
import { api } from "./lib/invoke";
import OverviewPage from "./pages/overview";
import AgentsPage from "./pages/agents";
import ExtensionsPage from "./pages/extensions";
import AuditPage from "./pages/audit";
import SettingsPage from "./pages/settings";
import MarketplacePage from "./pages/marketplace";
import { Onboarding, useOnboarding } from "./components/onboarding/onboarding";
import { Confetti } from "./components/onboarding/confetti";

/** Minimum interval (ms) between consecutive scan_and_sync calls */
const SCAN_DEBOUNCE_MS = 5_000;

export default function App() {
  const themeName = useUIStore((s) => s.themeName);
  const mode = useUIStore((s) => s.mode);
  const fetchExtensions = useExtensionStore((s) => s.fetch);
  const loadCachedAudit = useAuditStore((s) => s.loadCached);
  const { show: showOnboarding, complete: completeOnboarding } = useOnboarding();
  const [showConfetti, setShowConfetti] = useState(false);
  const lastScanRef = useRef(0);
  const appIcon = useUIStore((s) => s.appIcon);

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

  // Background scan + rescan on window focus
  useEffect(() => {
    const runScan = () => {
      const now = Date.now();
      if (now - lastScanRef.current < SCAN_DEBOUNCE_MS) return;
      lastScanRef.current = now;
      api.scanAndSync()
        .catch(() => {})
        .then(() => {
          fetchExtensions();
          loadCachedAudit();
        });
    };

    // Initial scan on startup
    runScan();

    // Re-scan when the window regains focus (catches external installs)
    const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) runScan();
    });
    return () => { unlisten.then((fn) => fn()); };
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

  // Restore app icon from saved preference
  useEffect(() => {
    api.setAppIcon(appIcon).catch(() => {});
  }, [appIcon]);

  if (showOnboarding) {
    return <Onboarding onComplete={() => { completeOnboarding(); setShowConfetti(true); setTimeout(() => setShowConfetti(false), 3000); }} />;
  }

  return (
    <>
      {showConfetti && <Confetti />}
      <HashRouter>
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<OverviewPage />} />
            <Route path="agents" element={<AgentsPage />} />
            <Route path="extensions" element={<ExtensionsPage />} />
            <Route path="marketplace" element={<MarketplacePage />} />
            <Route path="audit" element={<AuditPage />} />
            <Route path="settings" element={<SettingsPage />} />
          </Route>
        </Routes>
      </HashRouter>
    </>
  );
}
