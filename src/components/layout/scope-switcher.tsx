import { ChevronDown, Folder } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useScope } from "@/hooks/use-scope";
import type { ScopeValue } from "@/stores/scope-store";
import { ScopeSwitcherMenu } from "./scope-switcher-menu";

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

  const label = scopeLabel(scope);

  return (
    <div ref={containerRef} className="relative">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Switch scope, currently ${label}`}
        className="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-[14px] font-medium text-sidebar-foreground/60 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground transition-colors duration-150 ease-out"
      >
        <Folder size={20} strokeWidth={1.75} className="shrink-0" />
        <span className="truncate flex-1 text-left">{label}</span>
        <ChevronDown
          size={14}
          strokeWidth={1.75}
          className="shrink-0 opacity-60"
        />
      </button>
      {open && <ScopeSwitcherMenu onClose={() => setOpen(false)} />}
    </div>
  );
}
