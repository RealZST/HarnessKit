import { Check, Folder, Globe, LayoutGrid, Plus } from "lucide-react";
import { useScope } from "@/hooks/use-scope";
import { openDirectoryPicker } from "@/lib/dialog";
import { useProjectStore } from "@/stores/project-store";
import type { ScopeValue } from "@/stores/scope-store";
import { toast } from "@/stores/toast-store";

interface MenuItem {
  key: string;
  scope: ScopeValue;
  label: string;
  icon: React.ElementType;
}

export function ScopeSwitcherMenu({ onClose }: { onClose: () => void }) {
  const { scope, setScope } = useScope();
  const projects = useProjectStore((s) => s.projects);
  const addProject = useProjectStore((s) => s.addProject);

  const items: MenuItem[] = [];
  if (projects.length > 0) {
    items.push({
      key: "all",
      scope: { type: "all" },
      label: "All scopes",
      icon: LayoutGrid,
    });
  }
  items.push({
    key: "global",
    scope: { type: "global" },
    label: "Global",
    icon: Globe,
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

  const handleAddProject = async () => {
    const path = await openDirectoryPicker();
    if (!path) return;
    try {
      await addProject(path);
      const fresh = useProjectStore
        .getState()
        .projects.find((p) => p.path === path);
      if (fresh) {
        setScope({ type: "project", name: fresh.name, path: fresh.path });
        toast.success(`Project '${fresh.name}' added and selected`);
      }
      onClose();
    } catch (e) {
      toast.error(`Failed to add project: ${String(e)}`);
    }
  };

  // Group items: All scopes | (sep) | Global + projects | (sep) | Add Project
  const allItem = items.find((i) => i.key === "all");
  const restItems = items.filter((i) => i.key !== "all");

  // Render helper: JSX requires a CapitalCase identifier for components, so
  // we alias item.icon to a local PascalCase variable before using it as JSX.
  const renderOption = (item: MenuItem) => {
    const ItemIcon = item.icon;
    return (
      <button
        key={item.key}
        role="option"
        aria-selected={isCurrent(item)}
        onClick={() => handleSelect(item)}
        className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm hover:bg-accent"
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
      className="absolute left-3 right-3 top-full mt-1 z-50 rounded-lg border border-border bg-popover p-1 shadow-lg"
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
        className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm text-muted-foreground hover:bg-accent"
      >
        <Plus size={14} />
        <span>Add Project...</span>
      </button>
    </div>
  );
}
