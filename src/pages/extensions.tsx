import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import { useExtensionStore } from "@/stores/extension-store";
import { useAgentStore } from "@/stores/agent-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { ExtensionFilters } from "@/components/extensions/extension-filters";
import { ExtensionDetail } from "@/components/extensions/extension-detail";
import { RefreshCw, Plus, ArrowDownCircle } from "lucide-react";
import { Toast } from "@/components/shared/toast";
import { toast } from "@/stores/toast-store";

export default function ExtensionsPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const setAgentFilter = useExtensionStore((s) => s.setAgentFilter);

  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const setKindFilter = useExtensionStore((s) => s.setKindFilter);
  const setSearchQuery = useExtensionStore((s) => s.setSearchQuery);
  const setCategoryFilter = useExtensionStore((s) => s.setCategoryFilter);
  const allGrouped = useExtensionStore((s) => s.grouped);

  const extensions = useExtensionStore(s => s.extensions);
  const pendingNameRef = useRef(searchParams.get("name"));

  // Apply ?agent= query param on mount only
  const didApplyRef = useRef(false);
  useEffect(() => {
    if (!didApplyRef.current) {
      const agent = searchParams.get("agent");
      if (agent) setAgentFilter(agent);
      // Clear filters once if navigating to a specific extension
      if (pendingNameRef.current) {
        setKindFilter(null);
        setAgentFilter(null);
        setCategoryFilter(null);
        setSearchQuery("");
      }
      didApplyRef.current = true;
    }
  }, [searchParams, setAgentFilter, setKindFilter, setCategoryFilter, setSearchQuery]);

  // Match the extension once data is available
  useEffect(() => {
    const name = pendingNameRef.current;
    if (!name || extensions.length === 0) return;
    const groups = allGrouped();
    const match = groups.find((g) => g.name.toLowerCase() === name.toLowerCase());
    if (match) {
      setSelectedId(match.groupKey);
      pendingNameRef.current = null;
    }
  }, [extensions, allGrouped, setSelectedId]);
  const { loading, fetch, selectedId, selectedIds, batchToggle, batchDelete, undoDelete, confirmDelete, pendingDelete, clearSelection, checkUpdates, checkingUpdates, updateAll, updatingAll } = useExtensionStore();
  const updateStatuses = useExtensionStore(s => s.updateStatuses);
  const grouped = useExtensionStore(s => s.grouped);
  const updatesAvailable = useMemo(() => {
    return grouped().filter((g) =>
      g.instances.some((inst) => updateStatuses.get(inst.id)?.status === "update_available"),
    ).length;
  }, [updateStatuses, grouped]);
  const searchQuery = useExtensionStore(s => s.searchQuery);
  const categoryFilter = useExtensionStore(s => s.categoryFilter);
  const filtered = useExtensionStore(s => s.filtered);
  const agentFilter = useExtensionStore(s => s.agentFilter);
  const kindFilter = useExtensionStore(s => s.kindFilter);
  const data = useMemo(() => filtered(), [extensions, searchQuery, categoryFilter, agentFilter, kindFilter]);
  const batchMode = selectedIds.size > 0;
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

  const fetchAgents = useAgentStore((s) => s.fetch);
  useEffect(() => { fetch(); fetchAgents(); }, [fetch, fetchAgents]);

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold tracking-tight select-none">Extensions</h2>
            <button
              onClick={() => navigate("/marketplace")}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
            >
              <Plus size={12} />
              Install New
            </button>
            <button
              onClick={() => {
                checkUpdates().then(() => toast.success("Updates checked"));
              }}
              disabled={checkingUpdates}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
            >
              <RefreshCw size={12} className={checkingUpdates ? "animate-spin" : ""} />
              {checkingUpdates ? "Checking..." : "Check Updates"}
            </button>
            {updatesAvailable > 0 && (
              <button
                onClick={() => {
                  updateAll().then((n) => {
                    if (n > 0) toast.success(`${n} extension${n > 1 ? "s" : ""} updated`);
                  });
                }}
                disabled={updatingAll}
                className="flex items-center gap-1 rounded-lg border border-primary/30 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/20 hover:shadow-md disabled:opacity-50"
              >
                <ArrowDownCircle size={12} className={updatingAll ? "animate-bounce" : ""} />
                {updatingAll ? "Updating..." : `Update All (${updatesAvailable})`}
              </button>
            )}
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
                  <button onClick={() => { batchToggle(true); toast.success(`${selectedIds.size} extension${selectedIds.size === 1 ? "" : "s"} enabled`); }} aria-label="Enable selected extensions" className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90">Enable</button>
                  <button onClick={() => { batchToggle(false); toast.success(`${selectedIds.size} extension${selectedIds.size === 1 ? "" : "s"} disabled`); }} aria-label="Disable selected extensions" className="rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground">Disable</button>
                  <button onClick={() => setConfirmingDelete(true)} aria-label="Delete selected extensions" className="rounded-lg bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90">Delete</button>
                  <button onClick={clearSelection} className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground">Cancel</button>
                </>
              )}
            </div>
          )}
        </div>
        <ExtensionFilters />
      </div>

      {/* Scrollable content */}
      <div className="relative flex-1 min-h-0">
        <div className="absolute inset-0 overflow-y-auto pb-4">
          {loading && extensions.length === 0 ? (
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
        {selectedId && (
          <div className="absolute right-0 top-0 bottom-0 w-96 z-10">
            <ExtensionDetail />
          </div>
        )}
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
