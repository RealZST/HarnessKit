import { X } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { parseError } from "@/lib/error-types";
import { humanizeError } from "@/lib/errors";
import { useRepositoryStore } from "@/stores/repository-store";

/** Ask for a Git URL and track it. Stays open through the first sync, so a
 *  URL that cannot be cloned is reported here and corrected in place. */
export function AddRepositoryDialog({
  onClose,
  onAdded,
}: {
  onClose: () => void;
  onAdded: (id: string) => void;
}) {
  const { t } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const titleId = useId();
  const errorId = useId();
  const add = useRepositoryStore((s) => s.add);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  // `cloneFailed` tells a repository git could not fetch from one that was
  // refused before any cloning (already tracked, not a Git URL).
  const [error, setError] = useState<{
    message: string;
    cloneFailed: boolean;
  } | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onAdded(await add(url));
      onClose();
    } catch (err) {
      setError({
        message: humanizeError(err),
        cloneFailed: parseError(err).kind === "CommandFailed",
      });
      setBusy(false);
    }
  }

  return (
    <Modal onClose={onClose} busy={busy} ariaLabelledBy={titleId}>
      <form onSubmit={submit} className="flex min-h-0 flex-col">
        <header className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 id={titleId} className="text-base font-semibold">
            {t("addTitle")}
          </h2>
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            aria-label={tc("actions.cancel")}
            className="rounded-md p-1 hover:bg-muted disabled:opacity-50"
          >
            <X className="h-4 w-4" />
          </button>
        </header>
        <div className="flex-1 space-y-3 overflow-auto px-4 py-3">
          <p className="text-xs text-muted-foreground">{t("addDescription")}</p>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://github.com/owner/repo"
            aria-label={t("addTitle")}
            aria-invalid={error != null}
            aria-describedby={error ? errorId : undefined}
            readOnly={busy}
            className="w-full rounded-lg border border-border bg-card px-3 py-1.5 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
          {busy && (
            <p role="status" className="text-xs text-muted-foreground">
              {t("addingHint")}
            </p>
          )}
          {error && (
            <div
              id={errorId}
              role="alert"
              className="space-y-1 text-xs text-muted-foreground [overflow-wrap:anywhere]"
            >
              {error.cloneFailed && (
                <p className="font-medium text-foreground">{t("addFailed")}</p>
              )}
              <p>{error.message}</p>
            </div>
          )}
        </div>
        <footer className="flex justify-end gap-2 border-t border-border px-4 py-3">
          <button
            type="button"
            onClick={onClose}
            disabled={busy}
            className="rounded-md border border-border px-3 py-1.5 text-sm disabled:opacity-50"
          >
            {tc("actions.cancel")}
          </button>
          <button
            type="submit"
            disabled={busy || !url.trim()}
            className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground disabled:opacity-50"
          >
            {busy ? t("adding") : t("add")}
          </button>
        </footer>
      </form>
    </Modal>
  );
}
