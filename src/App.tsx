import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "./components/layout/app-shell";
import { UpdateDialog } from "./components/layout/update-dialog";
import { WebUpdateDialog } from "./components/layout/web-update-dialog";
import { Confetti } from "./components/onboarding/confetti";
import { Onboarding, useOnboarding } from "./components/onboarding/onboarding";
import { ErrorBoundary } from "./components/shared/error-boundary";
import { api } from "./lib/invoke";
import { isDesktop } from "./lib/transport";
import AgentsPage from "./pages/agents";
import AuditPage from "./pages/audit";
import ExtensionsPage from "./pages/extensions";
import MarketplacePage from "./pages/marketplace";
import OverviewPage from "./pages/overview";
import SettingsPage from "./pages/settings";
import { useAuditStore } from "./stores/audit-store";
import { useExtensionStore } from "./stores/extension-store";
import { resolveMode, useUIStore } from "./stores/ui-store";
import { useUpdateStore } from "./stores/update-store";
import { useWebUpdateStore } from "./stores/web-update-store";

/** Minimum interval (ms) between consecutive scan_and_sync calls */
const SCAN_DEBOUNCE_MS = 5_000;

export default function App() {
  const themeName = useUIStore((s) => s.themeName);
  const mode = useUIStore((s) => s.mode);
  const fetchExtensions = useExtensionStore((s) => s.fetch);
  const loadCachedAudit = useAuditStore((s) => s.loadCached);
  const { show: showOnboarding, complete: completeOnboarding } =
    useOnboarding();
  const [showConfetti, setShowConfetti] = useState(false);
  const lastScanRef = useRef(0);
  const appIcon = useUIStore((s) => s.appIcon);

  // Track resolved dark/light (reacts to OS changes when mode === "system")
  const [resolved, setResolved] = useState<"dark" | "light">(() =>
    resolveMode(mode),
  );

  useEffect(() => {
    setResolved(resolveMode(mode));

    if (mode !== "system") return;

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => setResolved(mq.matches ? "dark" : "light");
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [mode]);

  // Check for updates on startup (non-blocking, silent failure).
  // Desktop uses Tauri's native updater; web mode polls GitHub releases.
  useEffect(() => {
    if (isDesktop()) {
      useUpdateStore.getState().checkForUpdate();
    } else {
      useWebUpdateStore.getState().checkForUpdate();
    }
  }, []);

  // Background scan + rescan on window focus
  useEffect(() => {
    const runScan = () => {
      const now = Date.now();
      if (now - lastScanRef.current < SCAN_DEBOUNCE_MS) return;
      lastScanRef.current = now;
      api
        .scanAndSync()
        .then(() => {
          fetchExtensions();
          loadCachedAudit();
        })
        .catch((e) => console.error("Failed to scan and sync:", e));
    };

    // Initial scan on startup
    runScan();

    // Re-scan when the window regains focus (catches external installs) — desktop only
    const unlistenFocus = isDesktop()
      ? getCurrentWindow().onFocusChanged(({ payload: focused }) => {
          if (focused) runScan();
        })
      : null;

    // Refresh when background marketplace matching completes — desktop only
    const unlistenChanged = isDesktop()
      ? listen("extensions-changed", () => {
          fetchExtensions();
        })
      : null;

    return () => {
      unlistenFocus?.then((fn) => fn());
      unlistenChanged?.then((fn) => fn());
    };
  }, [fetchExtensions, loadCachedAudit]);

  // Apply theme + dark class to <html>, and sync window appearance for vibrancy
  useEffect(() => {
    const root = document.documentElement;
    // Force Tiesen light during onboarding
    const activeTheme = showOnboarding ? "tiesen" : themeName;
    const activeDark = showOnboarding ? false : resolved === "dark";
    root.setAttribute("data-theme", activeTheme);
    if (activeDark) {
      root.classList.add("dark");
    } else {
      root.classList.remove("dark");
    }
    if (!isDesktop()) {
      root.setAttribute("data-web", "true");
    }
    // Force macOS vibrancy to match — "light" | "dark" | null (system)
    if (isDesktop()) {
      getCurrentWindow()
        .setTheme(
          showOnboarding ? "light" : mode === "system" ? null : resolved,
        )
        .catch((e) => console.error("Failed to set window theme:", e));
    }
  }, [themeName, mode, resolved, showOnboarding]);

  // Restore app icon from saved preference — desktop only
  useEffect(() => {
    if (isDesktop()) {
      api
        .setAppIcon(appIcon)
        .catch((e) => console.error("Failed to set app icon:", e));
    }
  }, [appIcon]);

  if (showOnboarding) {
    return (
      <Onboarding
        onComplete={() => {
          completeOnboarding();
          setShowConfetti(true);
          setTimeout(() => setShowConfetti(false), 3000);
        }}
      />
    );
  }

  return (
    <>
      {showConfetti && <Confetti />}
      {isDesktop() ? <UpdateDialog /> : <WebUpdateDialog />}
      <HashRouter>
        <ErrorBoundary>
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<OverviewPage />} />
              <Route path="agents" element={<AgentsPage />} />
              <Route path="extensions" element={<ExtensionsPage />} />
              <Route path="marketplace" element={<MarketplacePage />} />
              <Route path="audit" element={<AuditPage />} />
              <Route path="settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </ErrorBoundary>
      </HashRouter>
    </>
  );
}
