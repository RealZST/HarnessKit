import { AlertTriangle, FolderOpen, Loader2, MinusCircle } from "lucide-react";
import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useEscape } from "@/hooks/use-escape";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import { useKitStore } from "@/stores/kit-store";
import type { KitSyncTarget } from "@/types/kits";

interface Props {
  kitId: string;
  syncTargets: KitSyncTarget[];
  onClose(): void;
}

function targetKey(t: KitSyncTarget): string {
  return `${t.project_path}:${t.agent_name}`;
}

export function RemoveFromProjectDialog({
  kitId,
  syncTargets,
  onClose,
}: Props) {
  const { t } = useTranslation("kits");
  const { t: tc } = useTranslation("common");
  const unsyncKit = useKitStore((s) => s.unsyncKit);

  const dlgRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dlgRef, true);

  const [selected, setSelected] = useState<Set<string>>(() => new Set());
  const [removing, setRemoving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEscape(onClose, !removing);

  const isSingle = syncTargets.length === 1;
  const allSelected =
    syncTargets.length > 0 &&
    syncTargets.every((tg) => selected.has(targetKey(tg)));

  function toggle(key: string) {
    const next = new Set(selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelected(next);
  }

  async function confirmRemove() {
    const targets = isSingle
      ? syncTargets
      : syncTargets.filter((tg) => selected.has(targetKey(tg)));
    if (targets.length === 0) return;
    setRemoving(true);
    setError(null);
    try {
      for (const tg of targets) {
        await unsyncKit({
          kit_id: kitId,
          project_path: tg.project_path,
          agent_name: tg.agent_name,
        });
      }
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRemoving(false);
    }
  }

  const selectedCount = isSingle ? syncTargets.length : selected.size;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => {
        if (e.target === e.currentTarget && !removing) onClose();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="remove-from-title"
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <MinusCircle size={16} />
          </span>
          <div>
            <h3
              id="remove-from-title"
              className="text-sm font-semibold text-foreground"
            >
              {t("detail.removeFromTitle")}
            </h3>
            <p className="text-xs text-muted-foreground">
              {t("detail.removeFromHint")}
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
            {/* Select-all toggle when there are multiple targets */}
            {!isSingle && (
              <label className="flex items-start gap-2 text-xs cursor-pointer pb-1.5 mb-1.5 border-b border-border/50">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={() => {
                    setSelected(
                      allSelected
                        ? new Set()
                        : new Set(syncTargets.map(targetKey)),
                    );
                  }}
                  className="mt-0.5 rounded border-border accent-destructive"
                />
                <span className="font-medium text-foreground">
                  {t("detail.allInstalls", { defaultValue: "All installs" })}
                </span>
              </label>
            )}

            {syncTargets.map((tg) => {
              const key = targetKey(tg);
              return (
                <label
                  key={key}
                  className={`flex items-start gap-2 text-xs ${isSingle ? "" : "cursor-pointer"}`}
                >
                  {!isSingle && (
                    <input
                      type="checkbox"
                      checked={selected.has(key)}
                      onChange={() => toggle(key)}
                      className="mt-0.5 rounded border-border accent-destructive"
                    />
                  )}
                  <div className="min-w-0 flex-1">
                    <span className="font-medium text-foreground">
                      {tg.agent_name}
                    </span>
                    {tg.shared_with.length > 0 && (
                      <span className="ml-1.5 text-[10px] font-medium text-warning">
                        {t("detail.sharedBadge", {
                          defaultValue: "shared install path",
                        })}
                      </span>
                    )}
                    <span className="ml-1.5 text-muted-foreground">
                      · {t("detail.fileCount", { count: tg.file_count })}
                    </span>
                    <p className="text-muted-foreground flex items-start gap-1 mt-0.5">
                      <FolderOpen size={10} className="mt-0.5 shrink-0" />
                      <span className="break-all">{tg.project_path}</span>
                    </p>
                  </div>
                </label>
              );
            })}
          </div>

          {/* Aggregate shared-dir warning: lists every selected target whose
              install path is shared with other agents, so the user sees the
              fallout before clicking Remove. The backend already filters
              shared_with by project presence — agents the user hasn't
              configured here are dropped before this surface. */}
          {(() => {
            const selectedTargets = isSingle
              ? syncTargets
              : syncTargets.filter((tg) => selected.has(targetKey(tg)));
            const uniqueOthers = Array.from(
              new Set(selectedTargets.flatMap((tg) => tg.shared_with)),
            );
            if (uniqueOthers.length === 0) return null;
            return (
              <div className="flex items-start gap-1.5 rounded-lg border border-warning/40 bg-warning/10 p-2.5 text-xs text-warning">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                <span>
                  {t("detail.sharedWarning", {
                    count: uniqueOthers.length,
                    agents: uniqueOthers.join(", "),
                    defaultValue: `${uniqueOthers.join(", ")} reads from the same folder — removing here will also remove these skills for it.`,
                  })}
                </span>
              </div>
            );
          })()}

          {error && <p className="text-xs text-destructive">{error}</p>}

          {/* Confirm button */}
          <button
            type="button"
            disabled={removing || selectedCount === 0}
            onClick={confirmRemove}
            className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
          >
            {removing ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <MinusCircle size={12} />
            )}
            {t("detail.removeCount", {
              count: selectedCount,
              defaultValue: `Remove ${selectedCount}`,
            })}
          </button>
        </div>

        <button
          type="button"
          onClick={onClose}
          disabled={removing}
          className="mt-4 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
        >
          {tc("actions.cancel")}
        </button>
      </div>
    </div>
  );
}
