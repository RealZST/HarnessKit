import { ArrowDownCircle, Package, Plus, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { ExtensionDetail } from "@/components/extensions/extension-detail";
import { ExtensionFilters } from "@/components/extensions/extension-filters";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { NewSkillsDialog } from "@/components/extensions/new-skills-dialog";
import { useScope } from "@/hooks/use-scope";
import type { ConfigScope } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

export default function ExtensionsPage() {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const setAgentFilter = useExtensionStore((s) => s.setAgentFilter);

  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const setKindFilter = useExtensionStore((s) => s.setKindFilter);
  const setSearchQuery = useExtensionStore((s) => s.setSearchQuery);
  const setPackFilter = useExtensionStore((s) => s.setPackFilter);
  const allGrouped = useExtensionStore((s) => s.grouped);

  const extensions = useExtensionStore((s) => s.extensions);
  const pendingNameRef = useRef(searchParams.get("name"));
  const pendingGroupKeyRef = useRef(searchParams.get("groupKey"));

  // Apply query params synchronously on first render to avoid filter-change flash.
  const didApplyRef = useRef(false);
  if (!didApplyRef.current) {
    const agent = searchParams.get("agent");
    if (agent) setAgentFilter(agent);
    if (pendingNameRef.current || pendingGroupKeyRef.current) {
      setKindFilter(null);
      setAgentFilter(null);
      setPackFilter(null);
      setSearchQuery("");
    }
    didApplyRef.current = true;
  }

  // Match the extension once data is available and scroll to it
  const [scrollToId, setScrollToId] = useState<string | null>(null);
  useEffect(() => {
    if (extensions.length === 0) return;
    const groups = allGrouped();

    const gk = pendingGroupKeyRef.current;
    if (gk) {
      const match = groups.find((g) => g.groupKey === gk);
      if (match) {
        setSelectedId(match.groupKey);
        setScrollToId(match.groupKey);
        pendingGroupKeyRef.current = null;
        pendingNameRef.current = null;
      }
      return;
    }

    const name = pendingNameRef.current;
    if (!name) return;
    const match = groups.find(
      (g) => g.name.toLowerCase() === name.toLowerCase(),
    );
    if (match) {
      setSelectedId(match.groupKey);
      setScrollToId(match.groupKey);
      pendingNameRef.current = null;
    }
  }, [extensions, allGrouped, setSelectedId]);
  // Individual selectors — prevents unrelated state changes from causing re-renders
  const loading = useExtensionStore((s) => s.loading);
  const fetch = useExtensionStore((s) => s.fetch);
  const selectedId = useExtensionStore((s) => s.selectedId);
  const selectedIds = useExtensionStore((s) => s.selectedIds);
  const batchToggle = useExtensionStore((s) => s.batchToggle);
  const clearSelection = useExtensionStore((s) => s.clearSelection);
  const checkUpdates = useExtensionStore((s) => s.checkUpdates);
  const checkingUpdates = useExtensionStore((s) => s.checkingUpdates);
  const updateAll = useExtensionStore((s) => s.updateAll);
  const updatingAll = useExtensionStore((s) => s.updatingAll);
  const updateStatuses = useExtensionStore((s) => s.updateStatuses);
  const newRepoSkills = useExtensionStore((s) => s.newRepoSkills);
  const installNewRepoSkills = useExtensionStore((s) => s.installNewRepoSkills);
  const grouped = useExtensionStore((s) => s.grouped);
  const [showNewSkills, setShowNewSkills] = useState(false);
  const updatesAvailable = useMemo(() => {
    return grouped().filter((g) =>
      g.instances.some(
        (inst) => updateStatuses.get(inst.id)?.status === "update_available",
      ),
    ).length;
  }, [updateStatuses, grouped]);
  const data = useExtensionStore((s) => s.filtered());
  const batchMode = selectedIds.size > 0;
  const { scope } = useScope();
  // scope.type === "all" is impossible in single-scope mode; in All-scopes mode
  // Task 9 will supply a picker. For Task 8, narrow with a placeholder.
  const targetScope: ConfigScope =
    scope.type === "all" ? { type: "global" } : scope;

  const fetchAgents = useAgentStore((s) => s.fetch);
  const didFetchRef = useRef(false);
  useEffect(() => {
    if (didFetchRef.current) return;
    didFetchRef.current = true;
    fetch();
    fetchAgents();
  }, [fetch, fetchAgents]);

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold tracking-tight select-none">
              Extensions
            </h2>
            <button
              onClick={() => navigate("/marketplace")}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
            >
              <Plus size={12} />
              Install New
            </button>
            <button
              onClick={() => {
                checkUpdates().then(() => {
                  const state = useExtensionStore.getState();
                  const statuses = state.updateStatuses;
                  const count = state
                    .grouped()
                    .filter((g) =>
                      g.instances.some(
                        (inst) =>
                          statuses.get(inst.id)?.status === "update_available",
                      ),
                    ).length;
                  toast.success(
                    count > 0
                      ? `${count} update${count > 1 ? "s" : ""} available`
                      : "No updates available",
                  );
                });
              }}
              disabled={checkingUpdates}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
            >
              <RefreshCw
                size={12}
                className={checkingUpdates ? "animate-spin" : ""}
              />
              {checkingUpdates ? "Checking..." : "Check Updates"}
            </button>
            {updatesAvailable > 0 && (
              <button
                onClick={() => {
                  updateAll().then((n) => {
                    if (n > 0)
                      toast.success(
                        `${n} extension${n > 1 ? "s" : ""} updated`,
                      );
                  });
                }}
                disabled={updatingAll}
                className="flex items-center gap-1 rounded-lg border border-primary/30 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/20 hover:shadow-md disabled:opacity-50"
              >
                <ArrowDownCircle
                  size={12}
                  className={updatingAll ? "animate-bounce" : ""}
                />
                {updatingAll
                  ? "Updating..."
                  : `Update All (${updatesAvailable})`}
              </button>
            )}
            {newRepoSkills.length > 0 && (
              <button
                onClick={() => setShowNewSkills(true)}
                className="flex items-center gap-1 rounded-lg border border-primary/30 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/20 hover:shadow-md"
              >
                <Package size={12} />
                {newRepoSkills.length} More from Repos
              </button>
            )}
          </div>
          {batchMode && (
            <div className="animate-fade-in flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
              <span className="text-sm text-muted-foreground">
                {selectedIds.size} selected
              </span>
              <button
                onClick={() => {
                  batchToggle(true);
                  toast.success(
                    `${selectedIds.size} extension${selectedIds.size === 1 ? "" : "s"} enabled`,
                  );
                }}
                aria-label="Enable selected extensions"
                className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90"
              >
                Enable
              </button>
              <button
                onClick={() => {
                  batchToggle(false);
                  toast.success(
                    `${selectedIds.size} extension${selectedIds.size === 1 ? "" : "s"} disabled`,
                  );
                }}
                aria-label="Disable selected extensions"
                className="rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground"
              >
                Disable
              </button>
              <button
                onClick={clearSelection}
                className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground"
              >
                Cancel
              </button>
            </div>
          )}
        </div>
        <ExtensionFilters />
      </div>

      {/* Scrollable content */}
      <div className="relative flex-1 min-h-0">
        <div className="absolute inset-0 overflow-y-auto pb-4">
          {loading && extensions.length === 0 ? (
            <div
              className="rounded-xl border border-border overflow-hidden shadow-sm"
              aria-live="polite"
              role="status"
            >
              <div className="bg-muted/20 px-4 py-3">
                <div className="h-3 w-20 rounded animate-shimmer" />
              </div>
              {Array.from({ length: 5 }).map((_, i) => (
                <div
                  key={i}
                  className="flex items-center gap-4 border-t border-border px-4 py-3"
                >
                  <div className="h-4 w-4 rounded animate-shimmer" />
                  <div className="h-3 w-32 rounded animate-shimmer" />
                  <div className="h-3 w-16 rounded animate-shimmer" />
                  <div className="h-3 w-24 rounded animate-shimmer" />
                  <div className="ml-auto h-3 w-14 rounded animate-shimmer" />
                </div>
              ))}
            </div>
          ) : (
            <ExtensionTable data={data} scrollToId={scrollToId} />
          )}
        </div>
        {selectedId && (
          <div className="absolute right-0 top-0 bottom-0 w-96 z-10">
            <ExtensionDetail />
          </div>
        )}
      </div>
      {showNewSkills && newRepoSkills.length > 0 && (
        <NewSkillsDialog
          skills={newRepoSkills}
          onInstall={async (url, skillIds, targetAgents) => {
            await installNewRepoSkills(
              url,
              skillIds,
              targetAgents,
              targetScope,
            );
            toast.success(
              `${skillIds.length} skill${skillIds.length > 1 ? "s" : ""} installed`,
            );
          }}
          onDismiss={() => {
            useExtensionStore.setState({ newRepoSkills: [] });
            setShowNewSkills(false);
          }}
          onClose={() => setShowNewSkills(false)}
        />
      )}
    </div>
  );
}
