import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Sidebar } from "./sidebar";

const INTERACTIVE = "a, button, input, select, textarea, [role='button']";

export function AppShell() {
  const mainRef = useRef<HTMLElement>(null);
  const location = useLocation();

  useEffect(() => {
    mainRef.current?.scrollTo(0, 0);
  }, [location.pathname]);

  // Window dragging — anywhere outside <main> and interactive elements
  useEffect(() => {
    const onMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return;
      const target = e.target as HTMLElement;
      if (target.closest(INTERACTIVE) || target.closest("main")) return;
      e.preventDefault();
      getCurrentWindow().startDragging();
    };

    const onDblClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest(INTERACTIVE) || target.closest("main")) return;
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
            className="flex-1 overflow-auto rounded-xl bg-background/65 border border-border/30 shadow-[inset_0_1px_0_0_var(--border)] p-6"
          >
            <div key={location.pathname} className="animate-fade-in">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
