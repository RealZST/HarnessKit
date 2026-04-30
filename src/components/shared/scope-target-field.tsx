import { Folder, Globe } from "lucide-react";
import type { ConfigScope } from "@/lib/types";
import { useScope } from "@/hooks/use-scope";
import { useProjectStore } from "@/stores/project-store";

interface ScopeTargetFieldProps {
  /** The currently chosen install target. In single-scope mode this is
   *  always the active scope; in All-scopes mode it starts as `null`
   *  (or the smart-default scope) and the user must pick. */
  value: ConfigScope | null;
  onChange: (scope: ConfigScope | null) => void;
  /** Optional smart default to suggest in All-scopes mode. */
  smartDefault?: ConfigScope;
}

export function ScopeTargetField({
  value,
  onChange,
  smartDefault,
}: ScopeTargetFieldProps) {
  const { scope } = useScope();
  const projects = useProjectStore((s) => s.projects);

  // Single-scope mode: render a static hint, no picker
  if (scope.type !== "all") {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
        <span>·</span>
        {scope.type === "global" ? (
          <Globe size={11} />
        ) : (
          <Folder size={11} />
        )}
        <span>
          {scope.type === "global" ? "Global" : scope.name}
        </span>
      </span>
    );
  }

  // All-scopes mode: required dropdown
  const selectedKey = value
    ? value.type === "global"
      ? "global"
      : value.path
    : "";

  const handleChange = (key: string) => {
    if (!key) {
      onChange(null);
      return;
    }
    if (key === "global") {
      onChange({ type: "global" });
      return;
    }
    const proj = projects.find((p) => p.path === key);
    if (proj) onChange({ type: "project", name: proj.name, path: proj.path });
  };

  return (
    <label className="flex items-center gap-2 text-xs">
      <span className="font-medium">Install to scope:</span>
      <select
        value={selectedKey}
        onChange={(e) => handleChange(e.target.value)}
        className="rounded border border-border bg-card px-2 py-1 text-xs"
        aria-label="Install to scope"
      >
        <option value="">— Required —</option>
        <option value="global">🌐 Global</option>
        {projects.map((p) => (
          <option key={p.path} value={p.path}>
            📁 {p.name}
          </option>
        ))}
      </select>
      {smartDefault && !value && (
        <button
          type="button"
          onClick={() => onChange(smartDefault)}
          className="text-xs text-primary hover:underline"
        >
          Use{" "}
          {smartDefault.type === "global"
            ? "Global"
            : smartDefault.type === "project"
              ? smartDefault.name
              : ""}
        </button>
      )}
    </label>
  );
}
