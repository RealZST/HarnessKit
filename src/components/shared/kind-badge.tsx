import { clsx } from "clsx";
import type { ExtensionKind } from "@/lib/types";

const kindStyles: Record<ExtensionKind, string> = {
  skill: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20",
  mcp: "bg-violet-500/10 text-violet-600 dark:text-violet-400 border-violet-500/20",
  plugin: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border-emerald-500/20",
  hook: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
};

export function KindBadge({ kind }: { kind: ExtensionKind }) {
  return (
    <span className={clsx("rounded-md border px-2 py-0.5 text-xs font-medium", kindStyles[kind])}>
      {kind}
    </span>
  );
}
