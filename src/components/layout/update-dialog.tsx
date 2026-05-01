import { Download, Loader2, X } from "lucide-react";
import { useUpdateStore } from "@/stores/update-store";
import { ChangelogMarkdown } from "./changelog-markdown";

export function UpdateDialog() {
  const available = useUpdateStore((s) => s.available);
  const showChangelog = useUpdateStore((s) => s.showChangelog);
  const installing = useUpdateStore((s) => s.installing);
  const dismissDialog = useUpdateStore((s) => s.dismissDialog);
  const dismissUpdate = useUpdateStore((s) => s.dismissUpdate);
  const confirmUpdate = useUpdateStore((s) => s.confirmUpdate);

  if (!showChangelog || !available) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/40 backdrop-blur-sm"
        onClick={dismissDialog}
      />

      {/* Dialog */}
      <div className="relative w-[420px] max-h-[70vh] flex flex-col rounded-xl border border-border bg-background shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h3 className="text-base font-semibold">
            Update to v{available.version}
          </h3>
          <button
            onClick={dismissDialog}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        {/* Changelog */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <ChangelogMarkdown body={available.body} />
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-3 border-t border-border px-5 py-4">
          <button
            onClick={dismissUpdate}
            className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          >
            Later
          </button>
          <button
            onClick={confirmUpdate}
            disabled={installing}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-sm transition-colors hover:bg-primary/90 disabled:opacity-50"
          >
            {installing ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Download size={12} />
            )}
            {installing ? "Updating..." : "Update Now"}
          </button>
        </div>
      </div>
    </div>
  );
}
