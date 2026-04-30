import { Folder, Globe } from "lucide-react";
import type { ConfigScope } from "@/lib/types";
import { scopeLabel } from "@/lib/types";

export function ScopeBadge({ scope }: { scope: ConfigScope }) {
  const Icon = scope.type === "global" ? Globe : Folder;
  const label = scopeLabel(scope);
  return (
    <span
      aria-label={`Scope: ${label}`}
      className="inline-flex items-center gap-1 rounded bg-muted/60 px-1.5 py-0.5 text-[10px] text-muted-foreground"
    >
      <Icon size={9} />
      {label}
    </span>
  );
}
