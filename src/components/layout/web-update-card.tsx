import { Download } from "lucide-react";
import { useWebUpdateStore } from "@/stores/web-update-store";

export function WebUpdateCard() {
  const available = useWebUpdateStore((s) => s.available);
  const dismissed = useWebUpdateStore((s) => s.dismissed);
  const promptUpdate = useWebUpdateStore((s) => s.promptUpdate);

  if (!available || dismissed) return null;

  return (
    <div className="mb-2 rounded-xl border border-primary/20 bg-primary/5 p-3">
      <p className="text-xs font-medium text-foreground">
        v{available.version} available
      </p>
      <button
        onClick={promptUpdate}
        className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-md bg-primary px-2.5 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-colors hover:bg-primary/90"
      >
        <Download size={12} />
        How to Update
      </button>
    </div>
  );
}
