import { ExternalLink, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { localizeChangelog } from "@/lib/i18n/changelog";
import { useWebUpdateStore } from "@/stores/web-update-store";
import { ChangelogMarkdown } from "./changelog-markdown";

const INSTRUCTIONS_URL = "https://github.com/RealZST/HarnessKit#updating";

export function WebUpdateDialog() {
  const { t, i18n } = useTranslation("update");
  const available = useWebUpdateStore((s) => s.available);
  const showDialog = useWebUpdateStore((s) => s.showDialog);
  const dismissDialog = useWebUpdateStore((s) => s.dismissDialog);
  const dismissUpdate = useWebUpdateStore((s) => s.dismissUpdate);

  if (!showDialog || !available) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      {/* Backdrop — left as plain chrome so pressing the dimmed area outside the
          dialog drags the window (handled by the app-shell drag listener, since
          it's outside <main>). Dismiss via the header ✕ or "Close". */}
      <div className="absolute inset-0 bg-black/40 backdrop-blur-sm" />

      <div
        role="dialog"
        aria-modal="true"
        className="relative w-[420px] max-h-[70vh] flex flex-col rounded-xl border border-border bg-background shadow-xl"
      >
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h3 className="text-base font-semibold">
            {t("title", { version: available.version })}
          </h3>
          <button
            onClick={dismissDialog}
            className="text-muted-foreground hover:text-foreground transition-colors"
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4">
          <ChangelogMarkdown
            body={localizeChangelog(available.body, i18n.language)}
          />
        </div>

        <div className="flex items-center justify-end gap-3 border-t border-border px-5 py-4">
          <button
            onClick={dismissUpdate}
            className="rounded-lg border border-border px-4 py-2 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          >
            {t("close")}
          </button>
          <a
            href={INSTRUCTIONS_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground shadow-sm transition-colors hover:bg-primary/90"
          >
            <ExternalLink size={12} />
            {t("viewInstructions")}
          </a>
        </div>
      </div>
    </div>
  );
}
