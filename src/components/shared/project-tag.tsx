import { Folder } from "lucide-react";
import type { ConfigScope } from "@/lib/types";

/** Marks a list row that belongs to a project. Global rows carry no mark. */
export function ProjectTag({ scope }: { scope: ConfigScope }) {
  if (scope.type !== "project") return null;
  return (
    <span
      className="inline-flex min-w-3 max-w-36 shrink-[9999] items-center gap-1 overflow-hidden text-xs font-normal text-muted-foreground"
      title={scope.path}
    >
      <Folder size={12} className="shrink-0" />
      <span className="truncate">{scope.name}</span>
    </span>
  );
}
