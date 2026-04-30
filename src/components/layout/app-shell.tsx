import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";
import { Outlet, useLocation, useSearchParams } from "react-router-dom";
import { ToastContainer } from "@/components/shared/toast-container";
import { useProjectStore } from "@/stores/project-store";
import { useScopeStore } from "@/stores/scope-store";
import { Sidebar } from "./sidebar";

const INTERACTIVE = "a, button, input, select, textarea, [role='button']";

export function AppShell() {
  const mainRef = useRef<HTMLElement>(null);
  useLocation();
  useEffect(() => {
    mainRef.current?.scrollTo(0, 0);
  }, []);

  const [searchParams, setSearchParams] = useSearchParams();
  const projects = useProjectStore((s) => s.projects);
  const projectsLoaded = useProjectStore((s) => !s.loading);
  const scopeHydrated = useScopeStore((s) => s.hydrated);
  const scope = useScopeStore((s) => s.current);

  // Effect 1: load projects on first mount if not already loaded
  useEffect(() => {
    if (
      useProjectStore.getState().projects.length === 0 &&
      !useProjectStore.getState().loading
    ) {
      useProjectStore.getState().loadProjects();
    }
  }, []);

  // Effect 2: hydrate scope-store once after projects load
  useEffect(() => {
    if (!projectsLoaded || scopeHydrated) return;
    const urlScope = searchParams.get("scope");
    useScopeStore.getState().hydrate(urlScope, projects);
  }, [projectsLoaded, projects, searchParams, scopeHydrated]);

  // Effect 3: keep URL in sync with store (covers programmatic setScope from
  // stores that can't use the useScope hook, e.g. project-store.removeProject
  // in Task 10). Without this, the URL would drift stale after such calls.
  useEffect(() => {
    if (!scopeHydrated) return;
    const expected =
      scope.type === "global"
        ? null
        : scope.type === "all"
          ? "all"
          : scope.path;
    const current = searchParams.get("scope");
    if (current === expected) return;
    const params = new URLSearchParams(searchParams);
    if (expected == null) params.delete("scope");
    else params.set("scope", expected);
    setSearchParams(params, { replace: true });
  }, [scope, scopeHydrated, searchParams, setSearchParams]);

  // Window dragging — anywhere outside <main> and interactive elements
  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement;
      if (
        target.closest(INTERACTIVE) ||
        target.closest("main") ||
        target.closest("nav")
      )
        return;
      e.preventDefault();
      getCurrentWindow().startDragging();
    };

    const onDblClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (
        target.closest(INTERACTIVE) ||
        target.closest("main") ||
        target.closest("nav")
      )
        return;
      getCurrentWindow().toggleMaximize();
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("dblclick", onDblClick);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("dblclick", onDblClick);
    };
  }, []);

  return (
    <div className="h-screen overflow-hidden text-foreground">
      {/* Frosted glass surface */}
      <div className="flex h-full bg-sidebar/25 backdrop-blur-xl backdrop-saturate-150 backdrop-brightness-105">
        <Sidebar />

        {/* py+pr padding exposes frosted surface on top / right / bottom */}
        <div className="flex-1 flex flex-col min-w-0 py-2.5 pr-2.5">
          <main
            ref={mainRef}
            className="flex-1 flex flex-col min-h-0 overflow-y-auto overflow-x-hidden rounded-xl bg-background border border-border/50 shadow-[inset_0_1px_3px_-1px_var(--border)] p-6"
          >
            <div className="flex-1 flex flex-col min-h-0">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
      <ToastContainer />
    </div>
  );
}
