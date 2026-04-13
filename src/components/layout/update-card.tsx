import { Download, Loader2 } from "lucide-react";
import { useUpdateStore } from "@/stores/update-store";

export function UpdateCard() {
  const available = useUpdateStore((s) => s.available);
  const installing = useUpdateStore((s) => s.installing);
  const promptUpdate = useUpdateStore((s) => s.promptUpdate);

  if (!available) return null;

  return (
    <div className="mb-2 rounded-xl border border-primary/20 bg-primary/5 p-3">
      <p className="text-xs font-medium text-foreground">
        v{available.version} available
      </p>
      <button
        onClick={promptUpdate}
        disabled={installing}
        className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-md bg-primary px-2.5 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-colors hover:bg-primary/90 disabled:opacity-50"
      >
        {installing ? (
          <Loader2 size={12} className="animate-spin" />
        ) : (
          <Download size={12} />
        )}
        {installing ? "Updating..." : "Update"}
      </button>
    </div>
  );
}
