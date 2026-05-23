import { X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { api } from "@/lib/invoke";

const PREVIEW_MAX_LINES = 500;

interface Props {
  path: string;
  onClose(): void;
}

export function FilePreviewModal({ path, onClose }: Props) {
  const { t } = useTranslation("kits");
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .readConfigFilePreview(path, PREVIEW_MAX_LINES)
      .then((text) => {
        if (alive) setContent(text);
      })
      .catch((e) => {
        if (alive) setError(String(e));
      });
    return () => {
      alive = false;
    };
  }, [path]);

  return (
    <Modal
      onClose={onClose}
      ariaLabelledBy="file-preview-title"
      backdropClassName="fixed inset-0 z-[60] flex items-center justify-center bg-black/50"
      containerClassName="flex max-h-[85vh] w-[min(90vw,800px)] flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl"
    >
      <header className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
        <div
          id="file-preview-title"
          className="min-w-0 flex-1 truncate text-sm font-medium"
          title={path}
        >
          {path}
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label={t("common:close", "Close")}
          className="ml-3 shrink-0 rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          <X className="h-4 w-4" />
        </button>
      </header>
      <div className="flex-1 overflow-auto bg-muted/30 p-4">
        {error ? (
          <p className="text-sm text-destructive">{error}</p>
        ) : content === null ? (
          <p className="text-xs text-muted-foreground">
            {t("editor.previewLoading")}
          </p>
        ) : (
          <pre className="whitespace-pre-wrap break-words font-mono text-xs leading-relaxed text-foreground">
            {content}
          </pre>
        )}
      </div>
    </Modal>
  );
}
