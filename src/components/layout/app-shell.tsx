import { useEffect, useRef } from "react";
import { Outlet, useLocation, useNavigate } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Sidebar } from "./sidebar";

const INTERACTIVE = "a, button, input, select, textarea, [role='button']";

/** Route targets for Cmd+1 through Cmd+5 */
const NAV_SHORTCUTS: Record<string, string> = {
  "1": "/",
  "2": "/extensions",
  "3": "/marketplace",
  "4": "/audit",
  "5": "/agents",
};

export function AppShell() {
  const mainRef = useRef<HTMLElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const location = useLocation();
  const navigate = useNavigate();

  useEffect(() => {
    mainRef.current?.scrollTo(0, 0);
    const el = contentRef.current;
    if (el) {
      el.style.animation = "none";
      el.offsetHeight; // reflow
      el.style.animation = "";
    }
  }, [location.pathname]);

  // Global keyboard shortcuts
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // Don't intercept when the user is typing in an input or textarea
      const tag = (e.target as HTMLElement).tagName;
      const isTyping = tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";

      // Cmd+1..5 — page navigation (always, even when typing)
      if (e.metaKey && !e.shiftKey && !e.altKey && NAV_SHORTCUTS[e.key]) {
        e.preventDefault();
        navigate(NAV_SHORTCUTS[e.key]);
        return;
      }

      // Cmd+, — settings (always)
      if (e.metaKey && e.key === ",") {
        e.preventDefault();
        navigate("/settings");
        return;
      }

      // Cmd+K — focus search (always, even from an input)
      if (e.metaKey && e.key === "k") {
        e.preventDefault();
        focusSearch();
        return;
      }

      // "/" — focus search (only when NOT typing)
      if (e.key === "/" && !isTyping && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        focusSearch();
        return;
      }
    };

    const focusSearch = () => {
      // Try to find a search input on the current page
      const searchInput =
        document.querySelector<HTMLInputElement>('[aria-label="Search extensions"]') ??
        document.querySelector<HTMLInputElement>('[aria-label="Search marketplace"]');

      if (searchInput) {
        searchInput.focus();
        searchInput.select();
      } else {
        // Navigate to Extensions and focus search after transition
        navigate("/extensions");
        requestAnimationFrame(() => {
          setTimeout(() => {
            const input = document.querySelector<HTMLInputElement>('[aria-label="Search extensions"]');
            input?.focus();
            input?.select();
          }, 100);
        });
      }
    };

    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [navigate]);

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
            className="flex-1 flex flex-col overflow-clip rounded-xl bg-background/65 border border-border/30 shadow-[inset_0_1px_0_0_var(--border)] p-6"
          >
            <div ref={contentRef} className="animate-fade-in flex-1 flex flex-col min-h-0">
              <Outlet />
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
