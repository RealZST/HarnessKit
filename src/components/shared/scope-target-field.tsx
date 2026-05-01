import { clsx } from "clsx";
import { Folder } from "lucide-react";
import { useScope } from "@/hooks/use-scope";
import type { ConfigScope } from "@/lib/types";
import { isWeb, webSelectStyle } from "@/lib/web-select";
import { useProjectStore } from "@/stores/project-store";

interface ScopeTargetFieldProps {
  /** The currently chosen install target. In single-scope mode this is
   *  always the active scope; in All-scopes mode it starts as `null`
   *  (or the smart-default scope) and the user must pick. */
  value: ConfigScope | null;
  onChange: (scope: ConfigScope | null) => void;
  /** Optional smart default to suggest in All-scopes mode. */
  smartDefault?: ConfigScope;
  /** When true, always render the picker — even in single-scope mode.
   *  Used by NewSkillsDialog where the dialog appears unexpectedly
   *  (post Check Updates discovery) and the active UI scope is not
   *  necessarily where the user wants the new skills installed. */
  alwaysPick?: boolean;
}

export function ScopeTargetField({
  value,
  onChange,
  smartDefault,
  alwaysPick = false,
}: ScopeTargetFieldProps) {
  const { scope } = useScope();
  const projects = useProjectStore((s) => s.projects);

  // Single-scope mode: render a static hint, no picker
  if (scope.type !== "all" && !alwaysPick) {
    return (
      <span className="inline-flex items-center gap-1 text-xs text-muted-foreground">
        <Folder size={11} />
        <span>{scope.type === "global" ? "Global" : scope.name}</span>
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
    <label className="flex w-full items-center gap-2">
      <span className="shrink-0 text-xs font-medium text-muted-foreground">
        Install to scope:
      </span>
      <select
        value={selectedKey}
        onChange={(e) => handleChange(e.target.value)}
        aria-label="Install to scope"
        style={webSelectStyle}
        className={clsx(
          "flex-1 min-w-0 border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none transition-colors",
          isWeb ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
        )}
      >
        <option value="">— Required —</option>
        <option value="global">Global</option>
        {projects.map((p) => (
          <option key={p.path} value={p.path}>
            {p.name}
          </option>
        ))}
      </select>
      {smartDefault && !value && (
        <button
          type="button"
          onClick={() => onChange(smartDefault)}
          className="shrink-0 text-xs text-primary hover:underline"
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
