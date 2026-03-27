import { useEffect, useState } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { ExtensionFilters } from "@/components/extensions/extension-filters";
import { ExtensionDetail } from "@/components/extensions/extension-detail";
import { RefreshCw } from "lucide-react";

export default function ExtensionsPage() {
  const { loading, fetch, filtered, selectedId, selectedIds, batchToggle, batchDelete, clearSelection, checkUpdates } = useExtensionStore();
  const data = filtered();
  const batchMode = selectedIds.size > 0;
  const [checkingUpdates, setCheckingUpdates] = useState(false);

  useEffect(() => { fetch(); }, [fetch]);

  return (
    <div className="flex gap-4">
      <div className="flex-1 space-y-4 min-w-0">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-xl font-semibold">Extensions</h2>
            <button
              onClick={() => { setCheckingUpdates(true); checkUpdates().finally(() => setCheckingUpdates(false)); }}
              disabled={checkingUpdates}
              className="flex items-center gap-1 rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground disabled:opacity-50"
            >
              <RefreshCw size={12} className={checkingUpdates ? "animate-spin" : ""} />
              {checkingUpdates ? "Checking..." : "Check Updates"}
            </button>
          </div>
          {batchMode && (
            <div className="flex items-center gap-2">
              <span className="text-sm text-muted-foreground">{selectedIds.size} selected</span>
              <button onClick={() => batchToggle(true)} className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90">Enable</button>
              <button onClick={() => batchToggle(false)} className="rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground">Disable</button>
              <button onClick={() => batchDelete()} className="rounded-lg bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90">Delete</button>
              <button onClick={clearSelection} className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground">Cancel</button>
            </div>
          )}
        </div>
        <ExtensionFilters />
        {loading ? (
          <div className="text-muted-foreground">Scanning...</div>
        ) : (
          <ExtensionTable data={data} />
        )}
      </div>
      {selectedId && <ExtensionDetail />}
    </div>
  );
}
