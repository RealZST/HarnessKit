import { GitBranch, Globe, KeyRound, Loader2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { humanizeError } from "@/lib/errors";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";

interface AddRepositoryDialogProps {
  open: boolean;
  onClose: () => void;
}

export function AddRepositoryDialog({
  open,
  onClose,
}: AddRepositoryDialogProps) {
  const { t } = useTranslation("repositories");
  const addRepo = useRepositoryStore((s) => s.addRepo);

  const [url, setUrl] = useState("");
  const [branch, setBranch] = useState("");
  const [token, setToken] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const dialogRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dialogRef, open);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !loading) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open, loading, onClose]);

  useEffect(() => {
    if (open) {
      setUrl("");
      setBranch("");
      setToken("");
      setError(null);
      setLoading(false);
    }
  }, [open]);

  if (!open) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!url.trim()) return;

    setLoading(true);
    setError(null);

    try {
      await addRepo(
        url.trim(),
        undefined,
        branch.trim() || undefined,
        token.trim() || undefined,
      );
      toast.success(t("addDialog.success"));
      onClose();
    } catch (err: unknown) {
      setError(humanizeError(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="add-repo-title"
        className="w-full max-w-lg rounded-2xl border border-border bg-card p-6 shadow-2xl space-y-5"
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2.5">
            <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-primary/10 text-primary">
              <Globe size={18} />
            </div>
            <div>
              <h2
                id="add-repo-title"
                className="text-base font-semibold text-card-foreground"
              >
                {t("addDialog.title")}
              </h2>
              <p className="text-xs text-muted-foreground">
                {t("addDialog.description")}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="rounded-lg p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {error && (
          <div className="rounded-xl border border-destructive/20 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="mb-1.5 block text-xs font-medium text-foreground">
              {t("addDialog.urlLabel")}{" "}
              <span className="text-destructive">*</span>
            </label>
            <input
              type="text"
              required
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder={t("addDialog.urlPlaceholder")}
              className="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden"
              // biome-ignore lint/a11y/noAutofocus: dialog is user-opened; focusing the primary input is expected.
              autoFocus
            />
          </div>

          <div>
            <label className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-foreground">
              <GitBranch size={13} className="text-muted-foreground" />
              {t("addDialog.branchLabel")}
            </label>
            <input
              type="text"
              value={branch}
              onChange={(e) => setBranch(e.target.value)}
              placeholder={t("addDialog.branchPlaceholder")}
              className="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden"
            />
          </div>

          <div>
            <label className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-foreground">
              <KeyRound size={13} className="text-muted-foreground" />
              {t("addDialog.tokenLabel")}
            </label>
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder={t("addDialog.tokenPlaceholder")}
              className="w-full rounded-xl border border-border bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden font-mono"
            />
          </div>

          <div className="flex items-center justify-end gap-2.5 pt-2">
            <button
              type="button"
              onClick={onClose}
              disabled={loading}
              className="rounded-xl border border-border px-4 py-2 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
            >
              {t("cancel")}
            </button>
            <button
              type="submit"
              disabled={loading || !url.trim()}
              className="flex items-center gap-2 rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90 disabled:opacity-50 transition-colors"
            >
              {loading ? (
                <>
                  <Loader2 size={14} className="animate-spin" />
                  {t("addDialog.submitting")}
                </>
              ) : (
                t("addDialog.submit")
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
