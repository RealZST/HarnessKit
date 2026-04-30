import { ChevronDown, Folder, Globe, LayoutGrid } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useScope } from "@/hooks/use-scope";
import type { ScopeValue } from "@/stores/scope-store";
import { ScopeSwitcherMenu } from "./scope-switcher-menu";

function scopeIcon(scope: ScopeValue) {
  if (scope.type === "all") return LayoutGrid;
  if (scope.type === "global") return Globe;
  return Folder;
}

function scopeLabel(scope: ScopeValue): string {
  if (scope.type === "all") return "All scopes";
  if (scope.type === "global") return "Global";
  return scope.name;
}

export function ScopeSwitcher() {
  const { scope } = useScope();
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    const onClick = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onClick);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onClick);
    };
  }, [open]);

  const Icon = scopeIcon(scope);
  const label = scopeLabel(scope);

  return (
    <div ref={containerRef} className="relative px-3">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Switch scope, currently ${label}`}
        className="flex w-full items-center gap-2 rounded-lg border border-border/40 bg-card/50 px-2 py-1.5 text-sm hover:bg-accent/60 transition-colors"
      >
        <Icon size={14} className="shrink-0 text-muted-foreground" />
        <span className="truncate flex-1 text-left">{label}</span>
        <ChevronDown size={12} className="shrink-0 text-muted-foreground" />
      </button>
      {open && <ScopeSwitcherMenu onClose={() => setOpen(false)} />}
    </div>
  );
}
