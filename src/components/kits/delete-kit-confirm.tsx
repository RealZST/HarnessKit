import { FolderInput, Trash2 } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useEscape } from "@/hooks/use-escape";
import { useFocusTrap } from "@/hooks/use-focus-trap";

interface Props {
  name: string;
  syncTargets: {
    project_path: string;
    agent_name: string;
    file_count: number;
  }[];
  onConfirm(alsoUninstall: boolean): void;
  onCancel(): void;
}

/** Drawer-overlay confirm for deleting a Kit. Renders absolute-inset over the
 *  detail drawer (not portalled) so it visually sits inside the drawer. */
export function DeleteKitConfirm({
  name,
  syncTargets,
  onConfirm,
  onCancel,
}: Props) {
  const { t } = useTranslation("kits");
  const { t: tc } = useTranslation("common");
  const dlgRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dlgRef, true);
  useEscape(onCancel);
  const totalFiles = syncTargets.reduce((sum, t) => sum + t.file_count, 0);
  const [alsoUninstall, setAlsoUninstall] = useState(false);
  const hasTargets = syncTargets.length > 0;
  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />
      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-kit-title"
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
      >
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <Trash2 size={16} />
          </span>
          <div>
            <h3
              id="delete-kit-title"
              className="text-sm font-semibold text-foreground"
            >
              {t("detail.deleteTitle", { name })}
            </h3>
            <p className="text-xs text-muted-foreground">
              {t("detail.deleteIrreversible", {
                defaultValue: "This action cannot be undone.",
              })}
            </p>
          </div>
        </div>

        {hasTargets && (
          <div className="space-y-3">
            <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
              <p className="text-xs font-medium text-foreground pb-1.5 mb-1.5 border-b border-border/50">
                {t("detail.deleteInstalledHeader", {
                  count: syncTargets.length,
                })}
              </p>
              {syncTargets.map((tgt) => (
                <div
                  key={`${tgt.project_path}:${tgt.agent_name}`}
                  className="flex items-start gap-2 text-xs"
                >
                  <div className="min-w-0 flex-1">
                    <span className="font-medium text-foreground">
                      {tgt.agent_name}
                    </span>
                    <span className="ml-1.5 text-muted-foreground">
                      · {t("detail.fileCount", { count: tgt.file_count })}
                    </span>
                    <p className="text-muted-foreground flex items-start gap-1 mt-0.5">
                      <FolderInput size={10} className="mt-0.5 shrink-0" />
                      <span className="break-all">{tgt.project_path}</span>
                    </p>
                  </div>
                </div>
              ))}
            </div>

            <label className="flex items-start gap-2 text-xs cursor-pointer">
              <input
                type="checkbox"
                checked={alsoUninstall}
                onChange={(e) => setAlsoUninstall(e.target.checked)}
                className="mt-0.5 rounded border-border accent-destructive"
              />
              <span className="text-foreground">
                {t("detail.alsoUninstallAll", {
                  count: syncTargets.length,
                  files: totalFiles,
                })}
              </span>
            </label>
          </div>
        )}

        <button
          type="button"
          onClick={() => onConfirm(alsoUninstall && hasTargets)}
          className="mt-4 w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90"
        >
          <Trash2 size={12} />
          {t("actions.delete")}
        </button>
        <button
          type="button"
          onClick={onCancel}
          className="mt-2 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted"
        >
          {tc("actions.cancel")}
        </button>
      </div>
    </div>
  );
}
