import { clsx } from "clsx";
import {
  AlertTriangle,
  ArrowUpCircle,
  CheckCircle2,
  ChevronDown,
  Download,
  ExternalLink,
  EyeOff,
  FolderGit2,
  GitBranch,
  RefreshCw,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { humanizeError } from "@/lib/errors";
import type {
  RepositoryItemWithInstallStatus,
  RepositoryWithItems,
} from "@/lib/types";
import { agentDisplayName, revisionsMatch } from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";
import { InstallRepoItemDialog } from "./install-repo-item-dialog";

interface RepositoryCardProps {
  repoData: RepositoryWithItems;
  searchFilter: string;
  defaultCollapsed?: boolean;
  highlightItemId?: string | null;
}

type TabKey = "all" | "available" | "installed" | "ignored";

export function RepositoryCard({
  repoData,
  searchFilter,
  defaultCollapsed,
  highlightItemId,
}: RepositoryCardProps) {
  const { t } = useTranslation("repositories");
  const { repository: repo, items } = repoData;

  const syncRepo = useRepositoryStore((s) => s.syncRepo);
  const deleteRepo = useRepositoryStore((s) => s.deleteRepo);
  const setItemStatus = useRepositoryStore((s) => s.setItemStatus);
  const syncingRepoIds = useRepositoryStore((s) => s.syncingRepoIds);

  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed ?? false);
  const [activeTab, setActiveTab] = useState<TabKey>("all");
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [installTargetItem, setInstallTargetItem] =
    useState<RepositoryItemWithInstallStatus | null>(null);

  useEffect(() => {
    if (!showDeleteConfirm) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setShowDeleteConfirm(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [showDeleteConfirm]);

  const normalizedHighlight = highlightItemId?.trim().toLowerCase();
  const hasHighlightedItem = Boolean(
    normalizedHighlight &&
      items.some(
        (i) =>
          i.item_id.toLowerCase() === normalizedHighlight ||
          i.name.toLowerCase() === normalizedHighlight,
      ),
  );

  useEffect(() => {
    if (defaultCollapsed !== undefined) {
      setIsCollapsed(defaultCollapsed);
    }
  }, [defaultCollapsed]);

  useEffect(() => {
    if (hasHighlightedItem) {
      setActiveTab("all");
    }
  }, [hasHighlightedItem]);

  const isSyncing = syncingRepoIds.has(repo.id);

  // Auto-expand when a search query is active or when containing highlighted item
  const isExpanded =
    !isCollapsed || searchFilter.trim().length > 0 || hasHighlightedItem;

  // Filter items by search string and active tab
  const q = searchFilter.trim().toLowerCase();
  const searchMatches = items.filter((item) => {
    if (!q) return true;
    return (
      item.name.toLowerCase().includes(q) ||
      item.item_id.toLowerCase().includes(q) ||
      item.description.toLowerCase().includes(q) ||
      repo.name.toLowerCase().includes(q)
    );
  });

  const tabCounts = {
    all: searchMatches.length,
    available: searchMatches.filter(
      (i) => i.status !== "ignored" && i.installed_agents.length === 0,
    ).length,
    installed: searchMatches.filter((i) => i.installed_agents.length > 0)
      .length,
    ignored: searchMatches.filter((i) => i.status === "ignored").length,
  };

  const filteredItems = searchMatches.filter((item) => {
    switch (activeTab) {
      case "available":
        return item.status !== "ignored" && item.installed_agents.length === 0;
      case "installed":
        return item.installed_agents.length > 0;
      case "ignored":
        return item.status === "ignored";
      default:
        return true;
    }
  });

  const handleSync = async () => {
    if (isSyncing) return;
    try {
      await syncRepo(repo.id);
    } catch (e) {
      toast.error(humanizeError(e));
    }
  };

  const handleDelete = async () => {
    try {
      await deleteRepo(repo.id);
      toast.success(t("removeSuccess"));
      setShowDeleteConfirm(false);
    } catch (e) {
      toast.error(humanizeError(e));
    }
  };

  const isWebUrl =
    repo.url.startsWith("http://") || repo.url.startsWith("https://");

  return (
    <div className="rounded-2xl border border-border bg-card shadow-xs overflow-hidden transition-all duration-200">
      {/* Repository Header */}
      <div
        onClick={() => setIsCollapsed(!isCollapsed)}
        className={clsx(
          "p-5 bg-muted/20 space-y-3 cursor-pointer select-none hover:bg-muted/30 transition-colors",
          isExpanded && "border-b border-border/60",
        )}
      >
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-start gap-3 min-w-0">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary mt-0.5">
              <FolderGit2 size={20} />
            </div>
            <div className="min-w-0 space-y-1">
              <div className="flex items-center gap-2 flex-wrap">
                <ChevronDown
                  size={16}
                  className={clsx(
                    "shrink-0 text-muted-foreground transition-transform duration-200",
                    !isExpanded && "-rotate-90",
                  )}
                />
                <h3 className="text-base font-semibold text-foreground truncate">
                  {repo.name}
                </h3>
                <span className="rounded-full bg-muted border border-border px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                  {t("itemsCount", { count: items.length })}
                </span>
                {repo.branch && (
                  <span className="inline-flex items-center gap-1 rounded-full border border-border bg-background px-2 py-0.5 text-[11px] font-medium text-muted-foreground">
                    <GitBranch size={11} />
                    {repo.branch}
                  </span>
                )}
                {repo.last_commit_hash && (
                  <span className="rounded-md bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground">
                    {repo.last_commit_hash.slice(0, 7)}
                  </span>
                )}
              </div>

              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <span className="truncate max-w-sm">{repo.url}</span>
                {isWebUrl && (
                  <a
                    href={repo.url.replace(/\.git$/, "")}
                    target="_blank"
                    rel="noopener noreferrer"
                    onClick={(e) => e.stopPropagation()}
                    className="text-muted-foreground hover:text-foreground transition-colors"
                  >
                    <ExternalLink size={12} />
                  </a>
                )}
                <span>•</span>
                <span>
                  {repo.last_synced_at
                    ? t("lastSynced", {
                        time: new Date(repo.last_synced_at).toLocaleString(),
                      })
                    : t("neverSynced")}
                </span>
              </div>
            </div>
          </div>

          {/* Quick Actions */}
          <div
            className="flex items-center gap-1.5 shrink-0"
            onClick={(e) => e.stopPropagation()}
          >
            <button
              onClick={handleSync}
              disabled={isSyncing}
              title={t("sync")}
              className="inline-flex items-center gap-1.5 rounded-xl border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground hover:bg-accent disabled:opacity-50 transition-colors"
            >
              <RefreshCw
                size={13}
                className={isSyncing ? "animate-spin text-primary" : ""}
              />
              {isSyncing ? t("syncing") : t("sync")}
            </button>

            <button
              onClick={() => setShowDeleteConfirm(true)}
              title={t("remove")}
              className="rounded-xl border border-border/60 p-2 text-muted-foreground hover:bg-destructive/10 hover:border-destructive/30 hover:text-destructive transition-colors"
            >
              <Trash2 size={14} />
            </button>
          </div>
        </div>

        {/* Commit subject message */}
        {repo.last_commit_message && (
          <p className="text-xs text-foreground/80 font-mono line-clamp-1 bg-background/50 border border-border/40 rounded-lg px-2.5 py-1">
            {repo.last_commit_message}
          </p>
        )}

        {/* Sync error alert */}
        {repo.last_sync_error && (
          <div className="flex items-start gap-2 rounded-xl border border-destructive/20 bg-destructive/10 px-3.5 py-2.5 text-xs text-destructive">
            <AlertTriangle size={15} className="shrink-0 mt-0.5" />
            <div className="space-y-0.5">
              <span className="font-semibold">
                {t("syncError", { error: "" })}
              </span>
              <p className="font-mono text-[11px] whitespace-pre-wrap">
                {repo.last_sync_error}
              </p>
            </div>
          </div>
        )}
      </div>

      {isExpanded && (
        <>
          {/* Tabs */}
          <div className="flex items-center justify-between border-b border-border/60 bg-muted/10 px-5 text-xs font-medium text-muted-foreground">
            <div className="flex">
              {(["all", "available", "installed", "ignored"] as TabKey[]).map(
                (tab) => (
                  <button
                    key={tab}
                    onClick={() => setActiveTab(tab)}
                    className={`border-b-2 px-3 py-2.5 transition-colors ${
                      activeTab === tab
                        ? "border-primary text-foreground font-semibold"
                        : "border-transparent hover:text-foreground"
                    }`}
                  >
                    {t(`tabs.${tab}`, { count: tabCounts[tab] })}
                  </button>
                ),
              )}
            </div>
          </div>

          {/* Items List */}
          <div className="divide-y divide-border/40">
            {filteredItems.length === 0 ? (
              <div className="py-8 text-center text-xs text-muted-foreground">
                {t("item.noItems")}
              </div>
            ) : (
              filteredItems.map((item) => {
                const isInstalled = item.installed_agents.length > 0;
                const hasUpdate =
                  isInstalled &&
                  Boolean(repo.last_commit_hash) &&
                  item.installed_agents.some(
                    (ia) =>
                      Boolean(ia.revision) &&
                      !revisionsMatch(ia.revision, repo.last_commit_hash),
                  );

                const isHighlighted = Boolean(
                  normalizedHighlight &&
                    (item.item_id.toLowerCase() === normalizedHighlight ||
                      item.name.toLowerCase() === normalizedHighlight),
                );

                return (
                  <div
                    key={item.item_id}
                    id={`repo-item-${item.item_id}`}
                    data-repo-item={item.item_id}
                    className={clsx(
                      "flex items-center justify-between gap-4 p-4 transition-all duration-300",
                      isHighlighted
                        ? "ring-2 ring-primary/80 bg-primary/10 rounded-xl my-1 shadow-xs"
                        : "hover:bg-muted/15",
                    )}
                  >
                    <div className="min-w-0 space-y-1">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-sm font-semibold text-foreground">
                          {item.name}
                        </span>
                        {item.subpath &&
                          item.subpath !== "." &&
                          item.subpath !== item.name && (
                            <span className="font-mono text-[10px] text-muted-foreground/70">
                              {item.subpath}
                            </span>
                          )}

                        {/* Installed agent badges */}
                        {item.installed_agents.map((ia) => (
                          <span
                            key={`${ia.agent}-${JSON.stringify(ia.scope)}`}
                            className="inline-flex items-center gap-1 rounded-full border border-primary/20 bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary"
                          >
                            <CheckCircle2 size={10} />
                            {agentDisplayName(ia.agent)} (
                            {ia.scope.type === "project"
                              ? ia.scope.name
                              : t("scope.global", { ns: "common" })}
                            )
                          </span>
                        ))}
                      </div>

                      {item.description && (
                        <p className="text-xs text-muted-foreground line-clamp-2">
                          {item.description}
                        </p>
                      )}
                    </div>

                    {/* Item Actions */}
                    <div className="flex items-center gap-2 shrink-0">
                      {hasUpdate ? (
                        <button
                          onClick={() => setInstallTargetItem(item)}
                          className="inline-flex items-center gap-1 rounded-xl bg-amber-500/15 border border-amber-500/30 px-2.5 py-1.5 text-xs font-semibold text-amber-600 dark:text-amber-400 hover:bg-amber-500/25 transition-colors"
                        >
                          <ArrowUpCircle size={13} />
                          {t("item.update")}
                        </button>
                      ) : (
                        <button
                          onClick={() => setInstallTargetItem(item)}
                          className="inline-flex items-center gap-1 rounded-xl border border-border bg-background px-2.5 py-1.5 text-xs font-medium text-foreground hover:bg-accent transition-colors"
                        >
                          <Download size={13} className="text-primary" />
                          {t("item.install")}
                        </button>
                      )}

                      {item.status === "ignored" ? (
                        <button
                          onClick={() =>
                            setItemStatus(
                              repo.id,
                              item.item_id,
                              "available",
                              item.kind,
                            ).catch((e) => toast.error(humanizeError(e)))
                          }
                          title={t("item.restore")}
                          className="inline-flex items-center gap-1 rounded-xl border border-border px-2 py-1.5 text-xs text-muted-foreground hover:bg-accent hover:text-foreground transition-colors"
                        >
                          <RotateCcw size={12} />
                          {t("item.restore")}
                        </button>
                      ) : (
                        <button
                          onClick={() =>
                            setItemStatus(
                              repo.id,
                              item.item_id,
                              "ignored",
                              item.kind,
                            ).catch((e) => toast.error(humanizeError(e)))
                          }
                          title={t("item.ignore")}
                          className="rounded-xl border border-transparent p-1.5 text-muted-foreground/60 hover:border-border hover:bg-accent hover:text-foreground transition-colors"
                        >
                          <EyeOff size={13} />
                        </button>
                      )}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </>
      )}

      {/* Delete Confirmation Modal */}
      {showDeleteConfirm && (
        <div
          role="dialog"
          aria-modal="true"
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              setShowDeleteConfirm(false);
            }
          }}
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4"
        >
          <div className="w-full max-w-sm rounded-2xl border border-border bg-card p-5 shadow-2xl space-y-4">
            <h4 className="text-sm font-semibold text-foreground">
              {t("removeDialogTitle")}
            </h4>
            <p className="text-xs text-muted-foreground leading-relaxed">
              {t("removeDialogDescription", { name: repo.name })}
            </p>
            <div className="flex items-center justify-end gap-2 pt-2">
              <button
                onClick={() => setShowDeleteConfirm(false)}
                className="rounded-xl border border-border px-3.5 py-1.5 text-xs font-medium text-muted-foreground hover:bg-accent hover:text-foreground"
              >
                {t("cancel")}
              </button>
              <button
                onClick={handleDelete}
                className="rounded-xl bg-destructive px-3.5 py-1.5 text-xs font-semibold text-destructive-foreground hover:bg-destructive/90"
              >
                {t("removeConfirm")}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Install Dialog */}
      {installTargetItem && (
        <InstallRepoItemDialog
          open={Boolean(installTargetItem)}
          onClose={() => setInstallTargetItem(null)}
          repoId={repo.id}
          item={installTargetItem}
        />
      )}
    </div>
  );
}
