import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { ExtensionFilters } from "@/components/extensions/extension-filters";
import { ExtensionDetail } from "@/components/extensions/extension-detail";
import { RefreshCw } from "lucide-react";
import { Hint } from "@/components/shared/hint";
import { Toast } from "@/components/shared/toast";

export default function ExtensionsPage() {
  const { loading, fetch, filtered, selectedId, selectedIds, batchToggle, batchDelete, undoDelete, confirmDelete, pendingDelete, clearSelection, checkUpdates } = useExtensionStore();
  const data = useMemo(() => filtered(), [filtered]);
  const batchMode = selectedIds.size > 0;
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const confirmDeleteTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [toastDeleteCount, setToastDeleteCount] = useState<number | null>(null);

  const handleBatchDelete = useCallback(() => {
    const count = selectedIds.size;
    batchDelete();
    setConfirmingDelete(false);
    setToastDeleteCount(count);
  }, [selectedIds.size, batchDelete]);

  const handleToastDismiss = useCallback(() => {
    setToastDeleteCount(null);
    confirmDelete();
  }, [confirmDelete]);

  const handleToastUndo = useCallback(() => {
    setToastDeleteCount(null);
    undoDelete();
  }, [undoDelete]);

  // Reset confirmation state when batch mode is exited
  useEffect(() => {
    if (!batchMode) setConfirmingDelete(false);
  }, [batchMode]);

  // Auto-cancel delete confirmation after 5 seconds
  useEffect(() => {
    if (confirmingDelete) {
      confirmDeleteTimerRef.current = setTimeout(() => setConfirmingDelete(false), 5000);
      return () => { if (confirmDeleteTimerRef.current) clearTimeout(confirmDeleteTimerRef.current); };
    }
  }, [confirmingDelete]);

  useEffect(() => { fetch(); }, [fetch]);

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold tracking-tight">Extensions</h2>
            <button
              onClick={() => { setCheckingUpdates(true); checkUpdates().finally(() => setCheckingUpdates(false)); }}
              disabled={checkingUpdates}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
            >
              <RefreshCw size={12} className={checkingUpdates ? "animate-spin" : ""} />
              {checkingUpdates ? "Checking..." : "Check Updates"}
            </button>
          </div>
          {batchMode && (
            <div className="animate-fade-in flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
              {confirmingDelete ? (
                <div className="animate-fade-in flex items-center gap-2">
                  <span className="text-sm text-muted-foreground">Delete {selectedIds.size} extension{selectedIds.size === 1 ? "" : "s"}?</span>
                  <button onClick={handleBatchDelete} className="rounded-lg bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90">Confirm</button>
                  <button onClick={() => setConfirmingDelete(false)} className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground">Cancel</button>
                </div>
              ) : (
                <>
                  <span className="text-sm text-muted-foreground">{selectedIds.size} selected</span>
                  <button onClick={() => batchToggle(true)} aria-label="Enable selected extensions" className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90">Enable</button>
                  <button onClick={() => batchToggle(false)} aria-label="Disable selected extensions" className="rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground">Disable</button>
                  <button onClick={() => setConfirmingDelete(true)} aria-label="Delete selected extensions" className="rounded-lg bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90">Delete</button>
                  <button onClick={clearSelection} className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground">Cancel</button>
                </>
              )}
            </div>
          )}
        </div>
        <ExtensionFilters />
        <Hint id="extensions-keyboard">
          Use arrow keys to navigate the table, Enter to select, and ⌘K to
          search. Click any row to see full details.
        </Hint>
      </div>

      {/* Scrollable content */}
      <div className="flex flex-1 min-h-0 flex-col md:flex-row gap-4">
      <div className="flex-1 min-w-0 overflow-y-auto">
        {loading ? (
          <div className="rounded-xl border border-border overflow-hidden shadow-sm" aria-live="polite" role="status">
            <div className="bg-muted/20 px-4 py-3">
              <div className="h-3 w-20 rounded animate-shimmer" />
            </div>
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="flex items-center gap-4 border-t border-border px-4 py-3">
                <div className="h-4 w-4 rounded animate-shimmer" />
                <div className="h-3 w-32 rounded animate-shimmer" />
                <div className="h-3 w-16 rounded animate-shimmer" />
                <div className="h-3 w-24 rounded animate-shimmer" />
                <div className="ml-auto h-3 w-14 rounded animate-shimmer" />
              </div>
            ))}
          </div>
        ) : (
          <ExtensionTable data={data} />
        )}
      </div>
      {selectedId && <ExtensionDetail />}
      </div>
      {toastDeleteCount !== null && pendingDelete && (
        <Toast
          message={`${toastDeleteCount} extension${toastDeleteCount === 1 ? "" : "s"} deleted`}
          onUndo={handleToastUndo}
          onDismiss={handleToastDismiss}
        />
      )}
    </div>
  );
}
