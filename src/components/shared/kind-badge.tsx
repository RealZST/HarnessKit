import { clsx } from "clsx";
import type { ExtensionKind } from "@/lib/types";

const kindStyles: Record<ExtensionKind, string> = {
  skill: "bg-blue-900/50 text-blue-300 border-blue-800",
  mcp: "bg-purple-900/50 text-purple-300 border-purple-800",
  plugin: "bg-emerald-900/50 text-emerald-300 border-emerald-800",
  hook: "bg-amber-900/50 text-amber-300 border-amber-800",
};

export function KindBadge({ kind }: { kind: ExtensionKind }) {
  return (
    <span className={clsx("rounded-md border px-2 py-0.5 text-xs font-medium", kindStyles[kind])}>
      {kind}
    </span>
  );
}
