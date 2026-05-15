import { ArrowDownCircle, Package, Plus, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useSearchParams } from "react-router-dom";
import { ExtensionDetail } from "@/components/extensions/extension-detail";
import { ExtensionFilters } from "@/components/extensions/extension-filters";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { NewSkillsDialog } from "@/components/extensions/new-skills-dialog";
import { useScope } from "@/hooks/use-scope";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { useProjectStore } from "@/stores/project-store";
import {
  resolveDeepLinkScope,
  scopesEqual,
  useScopeStore,
} from "@/stores/scope-store";
import { toast } from "@/stores/toast-store";

// Temporarily hide the "More from repos" banner + dialog. The discovery
// pipeline in check_updates still runs; only the UI surface is gated.
// Flip to true to re-enable.
const ENABLE_NEW_REPO_SKILLS: boolean = false;

export default function ExtensionsPage() {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const hydrated = useScopeStore((s) => s.hydrated);
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const setAgentFilter = useExtensionStore((s) => s.setAgentFilter);

  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const setKindFilter = useExtensionStore((s) => s.setKindFilter);
  const setSearchQuery = useExtensionStore((s) => s.setSearchQuery);
  const setPackFilter = useExtensionStore((s) => s.setPackFilter);
  const allGrouped = useExtensionStore((s) => s.grouped);

  const extensions = useExtensionStore((s) => s.extensions);
  const groupKeyParam = searchParams.get("groupKey");
  const nameParam = searchParams.get("name");
  const isDeepLink = !!(groupKeyParam || nameParam);
  const { scope, setScope } = useScope();
  const projects = useProjectStore((s) => s.projects);

  // Apply filter overrides synchronously on first render to avoid an initial
  // filter-change flash. Scope + selection are handled by the deep-link
  // effect below — calling setScope() in render warns about updating a
  // different component (ScopeSwitcher) while rendering this one.
  const didApplyFiltersRef = useRef(false);
  if (!didApplyFiltersRef.current) {
    const agent = searchParams.get("agent");
    if (agent) setAgentFilter(agent);
    if (isDeepLink) {
      setKindFilter(null);
      setAgentFilter(null);
      setPackFilter(null);
      setSearchQuery("");
    }
    didApplyFiltersRef.current = true;
  }

  // Cleanup: when the user manually changes scope (e.g. via Sidebar
  // ScopeSwitcher), close the detail panel — the selected ext may not exist
  // in the new scope. Declared BEFORE the deep-link effect so the deep-link
  // can pre-sync prevScopeRef.current without this cleanup undoing it on
  // the same render's effect phase.
  const prevScopeRef = useRef(scope);
  useEffect(() => {
    if (prevScopeRef.current !== scope) {
      setSelectedId(null);
      prevScopeRef.current = scope;
    }
  }, [scope, setSelectedId]);

  // Deep-link handler: applies ?scope= from URL, selects the target group,
  // then clears the URL params. Clearing is critical — without it, every
  // subsequent scope change (e.g. user clicking Sidebar ScopeSwitcher)
  // would re-fire this effect (scope dep), see the still-present groupKey
  // and "restore" the deep-link's scope/selection, fighting the user.
  // Mirrors the pattern in agents.tsx.
  const [scrollToId, setScrollToId] = useState<string | null>(null);
  useEffect(() => {
    if (!isDeepLink) return;
    if (extensions.length === 0) return;
    const targetScope = resolveDeepLinkScope(
      searchParams.get("scope"),
      projects,
    );
    if (!scopesEqual(targetScope, scope)) {
      setScope(targetScope);
      prevScopeRef.current = targetScope;
    }
    const groups = allGrouped();
    const match = groupKeyParam
      ? groups.find((g) => g.groupKey === groupKeyParam)
      : nameParam
        ? groups.find((g) => g.name.toLowerCase() === nameParam.toLowerCase())
        : undefined;
    if (match) {
      setSelectedId(match.groupKey);
      setScrollToId(match.groupKey);
    }
    setSearchParams({}, { replace: true });
  }, [
    isDeepLink,
    extensions,
    allGrouped,
    scope,
    setScope,
    projects,
    searchParams,
    setSearchParams,
    groupKeyParam,
    nameParam,
    setSelectedId,
  ]);
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

  // Close the detail panel when leaving the page so revisiting starts clean.
  // selectedId lives in zustand (persists across remounts) — without this,
  // navigating to Agents and back would keep an old row open.
  useEffect(() => {
    return () => {
      useExtensionStore.setState({ selectedId: null });
    };
  }, []);

  const fetchAgents = useAgentStore((s) => s.fetch);
  const didFetchRef = useRef(false);
  useEffect(() => {
    if (!hydrated || didFetchRef.current) return;
    didFetchRef.current = true;
    fetch();
    fetchAgents();
  }, [fetch, fetchAgents, hydrated]);

  if (!hydrated) {
    return (
      <div className="p-4 text-sm text-muted-foreground">
        {tc("status.loading")}
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold tracking-tight select-none">
              {t("page.title")}
            </h2>
            <button
              onClick={() => navigate("/marketplace")}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
            >
              <Plus size={12} />
              {t("page.installNew")}
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
                      ? t("page.updatesAvailable", { count })
                      : t("page.noUpdates"),
                  );
                });
              }}
              disabled={checkingUpdates}
              className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
            >
              <RefreshCw
                size={12}
                className={checkingUpdates ? "origin-center animate-spin" : ""}
              />
              {checkingUpdates ? t("page.checking") : t("page.checkUpdates")}
            </button>
            {updatesAvailable > 0 && (
              <button
                onClick={() => {
                  updateAll().then((n) => {
                    if (n > 0)
                      toast.success(t("page.updatedCount", { count: n }));
                  });
                }}
                disabled={updatingAll}
                title={t("page.updatesAvailable", { count: updatesAvailable })}
                className="flex items-center gap-1 rounded-lg border border-primary/30 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/20 hover:shadow-md disabled:opacity-50"
              >
                <ArrowDownCircle
                  size={12}
                  className={updatingAll ? "animate-bounce" : ""}
                />
                {updatingAll
                  ? t("page.updating")
                  : t("page.updateAll", { count: updatesAvailable })}
              </button>
            )}
            {ENABLE_NEW_REPO_SKILLS && newRepoSkills.length > 0 && (
              <button
                onClick={() => setShowNewSkills(true)}
                className="flex items-center gap-1 rounded-lg border border-primary/30 bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-primary/20 hover:shadow-md"
              >
                <Package size={12} />
                {t("page.moreFromRepos", { count: newRepoSkills.length })}
              </button>
            )}
          </div>
          {batchMode && (
            <div className="animate-fade-in flex items-center gap-2 rounded-lg bg-muted/50 px-3 py-2">
              <span className="text-sm text-muted-foreground">
                {t("page.selectedCount", { count: selectedIds.size })}
              </span>
              <button
                onClick={() => {
                  batchToggle(true);
                  toast.success(
                    t("page.enabledCount", { count: selectedIds.size }),
                  );
                }}
                aria-label={t("page.enableSelectedAria")}
                className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90"
              >
                {t("page.enable")}
              </button>
              <button
                onClick={() => {
                  batchToggle(false);
                  toast.success(
                    t("page.disabledCount", { count: selectedIds.size }),
                  );
                }}
                aria-label={t("page.disableSelectedAria")}
                className="rounded-lg bg-muted px-3 py-1 text-xs text-muted-foreground hover:bg-primary/10 hover:text-foreground"
              >
                {t("page.disable")}
              </button>
              <button
                onClick={clearSelection}
                className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground"
              >
                {tc("actions.cancel")}
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
      {ENABLE_NEW_REPO_SKILLS && showNewSkills && newRepoSkills.length > 0 && (
        <NewSkillsDialog
          skills={newRepoSkills}
          onInstall={async (url, skillIds, targetAgents, targetScope) => {
            await installNewRepoSkills(
              url,
              skillIds,
              targetAgents,
              targetScope,
            );
            toast.success(t("page.installedCount", { count: skillIds.length }));
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
