import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";
import { Sidebar } from "./sidebar";

export function AppShell() {
  const mainRef = useRef<HTMLElement>(null);
  const location = useLocation();

  // Reset scroll to top on route change
  useEffect(() => {
    mainRef.current?.scrollTo(0, 0);
  }, [location.pathname]);

  return (
    <div className="flex h-screen bg-background text-foreground">
      <Sidebar />
      <main ref={mainRef} className="flex-1 overflow-auto p-6">
        <Outlet />
      </main>
    </div>
  );
}
