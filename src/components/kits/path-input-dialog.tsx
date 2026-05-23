import { FolderSearch, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import {
  openDirectoryPicker,
  openFilePicker,
  saveFilePicker,
} from "@/lib/dialog";
import { isDesktop } from "@/lib/transport";

interface FileFilter {
  name: string;
  extensions: string[];
}

interface Props {
  title: string;
  description?: string;
  /** Initial value for the input. */
  defaultPath?: string;
  /** Label on the primary submit button. */
  submitLabel: string;
  /** "open" opens an existing file; "save" prompts for a target file;
   *  "directory" picks an existing folder (extension filters are ignored). */
  pickerMode: "open" | "save" | "directory";
  /** Extension filters for the Browse button (file modes only). */
  pickerFilters?: FileFilter[];
  /** Placeholder for the text input. */
  inputPlaceholder?: string;
  /** Hint shown beneath the input (e.g. supported extensions). */
  inputHint?: string;
  onSubmit(path: string): Promise<void> | void;
  onClose(): void;
}

/** Generic typed-path-with-Browse dialog used by import/export of Kits. */
export function PathInputDialog({
  title,
  description,
  defaultPath,
  submitLabel,
  pickerMode,
  pickerFilters,
  inputPlaceholder,
  inputHint,
  onSubmit,
  onClose,
}: Props) {
  const { t: tc } = useTranslation("common");
  const [path, setPath] = useState<string>(defaultPath ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pick() {
    let selected: string | null;
    if (pickerMode === "open") {
      selected = await openFilePicker({ filters: pickerFilters });
    } else if (pickerMode === "directory") {
      selected = await openDirectoryPicker();
    } else {
      selected = await saveFilePicker({
        defaultPath: path || undefined,
        filters: pickerFilters,
      });
    }
    if (selected) setPath(selected);
  }

  async function submit() {
    const trimmed = path.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onSubmit(trimmed);
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal onClose={onClose} ariaLabel={title} busy={busy}>
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <h2 className="text-base font-semibold">{title}</h2>
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
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void submit();
            }}
            disabled={busy}
            placeholder={
              inputPlaceholder ??
              (pickerMode === "directory" ? "/path/to/folder" : "/path/to/file")
            }
            className="flex-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none disabled:opacity-50"
          />
          {isDesktop() && (
            <button
              type="button"
              onClick={pick}
              disabled={busy}
              title={tc("actions.browse", { defaultValue: "Browse" })}
              className="shrink-0 rounded-lg border border-border bg-card p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground disabled:opacity-50"
            >
              <FolderSearch size={14} />
            </button>
          )}
        </div>
        {inputHint && (
          <p className="text-xs text-muted-foreground">{inputHint}</p>
        )}
        {error && <p className="text-xs text-destructive">{error}</p>}
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
          type="button"
          onClick={() => void submit()}
          disabled={busy || !path.trim()}
          className="rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground disabled:opacity-50"
        >
          {submitLabel}
        </button>
      </footer>
    </Modal>
  );
}
