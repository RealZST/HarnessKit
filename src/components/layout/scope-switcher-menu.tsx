import { Check, Folder, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useScope } from "@/hooks/use-scope";
import { useProjectStore } from "@/stores/project-store";
import type { ScopeValue } from "@/stores/scope-store";

interface MenuItem {
  key: string;
  scope: ScopeValue;
  label: string;
  icon: React.ElementType;
}

const ADD_PROJECT_KEY = "__add_project__";

type NavigableItem = MenuItem | { key: typeof ADD_PROJECT_KEY };

export function ScopeSwitcherMenu({ onClose }: { onClose: () => void }) {
  const { scope, setScope } = useScope();
  const projects = useProjectStore((s) => s.projects);
  const navigate = useNavigate();

  const items: MenuItem[] = [];
  if (projects.length > 0) {
    items.push({
      key: "all",
      scope: { type: "all" },
      label: "All scopes",
      icon: Folder,
    });
  }
  items.push({
    key: "global",
    scope: { type: "global" },
    label: "Global",
    icon: Folder,
  });
  for (const p of projects) {
    items.push({
      key: p.path,
      scope: { type: "project", name: p.name, path: p.path },
      label: p.name,
      icon: Folder,
    });
  }

  const isCurrent = (item: MenuItem): boolean => {
    if (scope.type === "all" && item.key === "all") return true;
    if (scope.type === "global" && item.key === "global") return true;
    if (scope.type === "project" && item.key === scope.path) return true;
    return false;
  };

  const handleSelect = (item: MenuItem) => {
    setScope(item.scope);
    onClose();
  };

  const handleAddProject = () => {
    navigate("/settings");
    onClose();
  };

  // Group items: All scopes | (sep) | Global + projects | (sep) | Add Project
  const allItem = items.find((i) => i.key === "all");
  const restItems = items.filter((i) => i.key !== "all");

  // Flat list of every selectable row in render order, used for ↑/↓ keyboard
  // navigation. The Add Project virtual row is appended at the end.
  const navigableItems = useMemo<NavigableItem[]>(() => {
    const list: NavigableItem[] = [];
    if (allItem) list.push(allItem);
    for (const it of restItems) list.push(it);
    list.push({ key: ADD_PROJECT_KEY });
    return list;
  }, [allItem, restItems]);

  const [activeIndex, setActiveIndex] = useState(() => {
    // Start with the currently selected scope highlighted, so opening the
    // menu doesn't visually jump to "All scopes" regardless of state.
    const idx = navigableItems.findIndex(
      (item) => item.key !== ADD_PROJECT_KEY && isCurrent(item as MenuItem),
    );
    return idx >= 0 ? idx : 0;
  });

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActiveIndex((i) => Math.min(i + 1, navigableItems.length - 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActiveIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === "Enter") {
        e.preventDefault();
        const item = navigableItems[activeIndex];
        if (!item) return;
        if (item.key === ADD_PROJECT_KEY) handleAddProject();
        else handleSelect(item as MenuItem);
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
    // handleSelect / handleAddProject are stable enough for this scope —
    // including activeIndex + navigableItems is sufficient to pick up
    // changes that affect dispatch.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeIndex, navigableItems]);

  const activeKey = navigableItems[activeIndex]?.key;

  // Render helper: JSX requires a CapitalCase identifier for components, so
  // we alias item.icon to a local PascalCase variable before using it as JSX.
  const renderOption = (item: MenuItem) => {
    const ItemIcon = item.icon;
    return (
      <button
        key={item.key}
        role="option"
        aria-selected={isCurrent(item)}
        data-active={activeKey === item.key ? "true" : undefined}
        onClick={() => handleSelect(item)}
        className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm hover:bg-accent data-[active=true]:bg-accent"
      >
        <ItemIcon size={14} className="text-muted-foreground" />
        <span className="flex-1 text-left truncate">{item.label}</span>
        {isCurrent(item) && <Check size={12} />}
      </button>
    );
  };

  return (
    <div
      role="listbox"
      className="absolute left-0 right-0 bottom-full mb-1 z-50 max-h-80 overflow-y-auto rounded-xl border border-sidebar-border/60 bg-popover p-1 shadow-md"
    >
      {allItem && (
        <>
          {renderOption(allItem)}
          <div className="my-1 border-t border-border/40" />
        </>
      )}
      {restItems.map((item) => renderOption(item))}
      <div className="my-1 border-t border-border/40" />
      <button
        onClick={handleAddProject}
        data-active={activeKey === ADD_PROJECT_KEY ? "true" : undefined}
        className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent data-[active=true]:bg-accent"
      >
        <Plus size={14} />
        <span>Add Project...</span>
      </button>
    </div>
  );
}
