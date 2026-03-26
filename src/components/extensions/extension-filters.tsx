import type { ExtensionKind } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { clsx } from "clsx";

const kinds: (ExtensionKind | null)[] = [null, "skill", "mcp", "plugin", "hook"];

export function ExtensionFilters() {
  const { kindFilter, setKindFilter } = useExtensionStore();

  return (
    <div className="flex gap-2">
      {kinds.map((kind) => (
        <button
          key={kind ?? "all"}
          onClick={() => setKindFilter(kind)}
          className={clsx(
            "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
            kindFilter === kind
              ? "bg-zinc-700 text-zinc-100"
              : "bg-zinc-900 text-zinc-400 hover:bg-zinc-800"
          )}
        >
          {kind ?? "All"}
        </button>
      ))}
    </div>
  );
}
