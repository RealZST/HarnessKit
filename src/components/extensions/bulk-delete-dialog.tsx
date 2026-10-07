import { clsx } from "clsx";
import { Check, Loader2, X } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { parseError } from "@/lib/error-types";
import type { GroupedExtension } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

export function BulkDeleteDialog({
  groups,
  onClose,
}: {
  groups: GroupedExtension[];
  onClose: () => void;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const [checked, setChecked] = useState<Set<string>>(
    () => new Set(groups.map((g) => g.groupKey)),
  );
  const [running, setRunning] = useState(false);

  const hasCli = groups.some(
    (g) => g.kind === "cli" && checked.has(g.groupKey),
  );

  function toggle(key: string) {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  async function confirm() {
    if (running) return;
    setRunning(true);
    const failures: { group: string; message: string }[] = [];
    let deleted = 0;
    const store = useExtensionStore.getState();
    for (const g of groups) {
      if (!checked.has(g.groupKey)) continue;
      try {
        // CLI uninstall is all-or-nothing server-side; the id list is
        // ignored for `cli` (extension-store.ts:611-634).
        await store.deleteInstances(
          g.groupKey,
          g.instances.map((i) => i.id),
        );
        deleted += 1;
      } catch (e) {
        failures.push({ group: g.name, message: parseError(e).message });
      }
    }
    setRunning(false);
    // The store deletes optimistically with a 5s pendingDelete timer and
    // restores rows + toasts on timer-time refusal — late refusals surface
    // through the store toasts, not here.
    if (failures.length > 0) {
      toast.error(
        t("bulkDelete.failed", {
          count: failures.length,
          message: failures[0].message,
        }),
      );
    } else {
      toast.success(t("bulkDelete.deleted", { count: deleted }));
    }
    store.clearSelection();
    onClose();
  }

  return (
    <Modal onClose={onClose} busy={running} ariaLabel={t("bulkDelete.title")}>
      <div className="flex items-center justify-between border-b border-border px-5 py-4">
        <h2 className="text-sm font-semibold">{t("bulkDelete.title")}</h2>
        {!running && (
          <button
            type="button"
            onClick={onClose}
            aria-label={tc("actions.close")}
            className="rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
          >
            <X size={14} />
          </button>
        )}
      </div>

      <div className="space-y-3 overflow-y-auto px-5 py-4">
        <div className="flex items-center justify-between">
          <span className="block text-sm font-medium">
            {t("delete.selectItems")}
          </span>
          {groups.length > 1 && (
            <button
              type="button"
              onClick={() =>
                setChecked(
                  checked.size === groups.length
                    ? new Set()
                    : new Set(groups.map((g) => g.groupKey)),
                )
              }
              className="text-xs text-muted-foreground hover:text-foreground"
            >
              {t("newSkills.selectAll", { count: groups.length })}
            </button>
          )}
        </div>
        <ul className="space-y-1">
          {groups.map((g) => {
            const sel = checked.has(g.groupKey);
            return (
              <li key={g.groupKey}>
                {/* biome-ignore lint/a11y/useSemanticElements: rows are visually list items but semantically toggle-able (multi-select); aria-pressed/role=checkbox keeps the multi-select semantics on a styled <button>. */}
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={sel}
                  onClick={() => toggle(g.groupKey)}
                  className={clsx(
                    "flex w-full items-center gap-2 rounded-lg border px-3 py-1.5 text-xs transition-[background-color,border-color] duration-150",
                    sel
                      ? "border-primary/40 bg-primary/20 text-foreground"
                      : "border-border bg-primary/10 text-muted-foreground hover:bg-primary/20 hover:border-ring",
                  )}
                >
                  {sel && <Check size={12} className="shrink-0 text-primary" />}
                  <span className="truncate font-medium">{g.name}</span>
                  <span className="shrink-0 text-muted-foreground">
                    {g.kind} · {g.instances.length}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
        {hasCli && (
          <p className="text-xs text-muted-foreground">
            {t("bulkDelete.warningCli")}
          </p>
        )}
      </div>

      <div className="flex items-center justify-end gap-2 border-t border-border px-5 py-3">
        <button
          type="button"
          onClick={onClose}
          disabled={running}
          className="rounded-lg px-3 py-1.5 text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
        >
          {tc("actions.cancel")}
        </button>
        <button
          type="button"
          onClick={confirm}
          disabled={running || checked.size === 0}
          className="flex items-center gap-1.5 rounded-lg bg-destructive px-3 py-1.5 text-xs text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
        >
          {running && <Loader2 size={12} className="animate-spin" />}
          {t("bulkDelete.confirm", { count: checked.size })}
        </button>
      </div>
    </Modal>
  );
}
