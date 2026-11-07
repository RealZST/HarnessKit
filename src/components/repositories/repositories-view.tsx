import {
  ChevronsUpDown,
  FolderGit2,
  Loader2,
  Plus,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AddRepositoryDialog } from "@/components/repositories/add-repository-dialog";
import { RepositoryCard } from "@/components/repositories/repository-card";
import { useRepositoryStore } from "@/stores/repository-store";

interface RepositoriesViewProps {
  showHeader?: boolean;
  highlightRepoId?: string | null;
  highlightItemId?: string | null;
  initialAddOpen?: boolean;
}

export function RepositoriesView({
  showHeader = false,
  highlightRepoId,
  highlightItemId,
  initialAddOpen = false,
}: RepositoriesViewProps) {
  const { t } = useTranslation("repositories");
  const { repositories, loading, isSyncingAll, fetch, syncAll } =
    useRepositoryStore();

  const [search, setSearch] = useState("");
  const [showAddDialog, setShowAddDialog] = useState(initialAddOpen);
  const [allCollapsed, setAllCollapsed] = useState(false);

  const normalizedItemId = highlightItemId?.trim().toLowerCase();
  const repoHoldingItem = normalizedItemId
    ? repositories.find((r) =>
        r.items.some(
          (i) =>
            i.item_id.toLowerCase() === normalizedItemId ||
            i.name.toLowerCase() === normalizedItemId,
        ),
      )
    : undefined;

  const effectiveHighlightRepoId =
    highlightRepoId ?? repoHoldingItem?.repository.id ?? null;

  useEffect(() => {
    fetch().catch((e) => console.error("Failed to fetch repositories:", e));
  }, [fetch]);

  useEffect(() => {
    if (repositories.length === 0) return;

    if (highlightItemId) {
      const timer = setTimeout(() => {
        const itemEl =
          document.getElementById(`repo-item-${highlightItemId}`) ||
          document.querySelector(`[data-repo-item="${highlightItemId}"]`);
        if (itemEl) {
          itemEl.scrollIntoView({ behavior: "smooth", block: "center" });
          return;
        }
        if (effectiveHighlightRepoId) {
          const repoEl = document.getElementById(
            `repo-${effectiveHighlightRepoId}`,
          );
          if (repoEl) {
            repoEl.scrollIntoView({ behavior: "smooth", block: "center" });
          }
        }
      }, 60);
      return () => clearTimeout(timer);
    } else if (effectiveHighlightRepoId) {
      const repoEl = document.getElementById(
        `repo-${effectiveHighlightRepoId}`,
      );
      if (repoEl) {
        repoEl.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }
  }, [highlightItemId, effectiveHighlightRepoId, repositories]);

  const q = search.trim().toLowerCase();
  const filteredRepos = repositories.filter((repoData) => {
    if (!q) return true;
    const nameMatch = repoData.repository.name.toLowerCase().includes(q);
    const urlMatch = repoData.repository.url.toLowerCase().includes(q);
    const itemsMatch = repoData.items.some(
      (item) =>
        item.name.toLowerCase().includes(q) ||
        item.item_id.toLowerCase().includes(q) ||
        item.description.toLowerCase().includes(q),
    );
    return nameMatch || urlMatch || itemsMatch;
  });

  return (
    <div className="flex flex-col space-y-5 pb-6">
      {/* Optional Standalone Page Header */}
      {showHeader && (
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              {t("title")}
            </h1>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t("subtitle")}
            </p>
          </div>

          <div className="flex items-center gap-2.5">
            <button
              onClick={() => syncAll()}
              disabled={isSyncingAll || repositories.length === 0}
              className="inline-flex items-center gap-1.5 rounded-xl border border-border bg-card px-3.5 py-2 text-xs font-semibold text-foreground hover:bg-accent disabled:opacity-50 transition-colors shadow-xs"
            >
              <RefreshCw
                size={13}
                className={isSyncingAll ? "animate-spin text-primary" : ""}
              />
              {isSyncingAll ? t("syncing") : t("syncAll")}
            </button>

            <button
              onClick={() => setShowAddDialog(true)}
              className="inline-flex items-center gap-1.5 rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition-colors shadow-xs"
            >
              <Plus size={14} />
              {t("addRepository")}
            </button>
          </div>
        </div>
      )}

      {/* Action & Search Bar */}
      <div className="flex items-center justify-between gap-3">
        <div className="relative flex-1 max-w-md">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
          />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t("searchPlaceholder")}
            className="w-full rounded-lg border border-border bg-card py-2 pl-9 pr-8 text-xs text-foreground placeholder:text-muted-foreground focus:border-ring focus:bg-background focus:shadow-xs focus:outline-none transition-[background-color,border-color,box-shadow]"
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch("")}
              aria-label="Clear search"
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground p-0.5 rounded transition-colors"
            >
              <X size={14} />
            </button>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setAllCollapsed(!allCollapsed)}
            disabled={repositories.length === 0}
            title={allCollapsed ? t("expandAll") : t("collapseAll")}
            className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-semibold text-foreground hover:bg-accent disabled:opacity-50 transition-colors shadow-xs"
          >
            <ChevronsUpDown size={12} className="text-muted-foreground" />
            {allCollapsed ? t("expandAll") : t("collapseAll")}
          </button>

          <button
            onClick={() => syncAll()}
            disabled={isSyncingAll || repositories.length === 0}
            className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-semibold text-foreground hover:bg-accent disabled:opacity-50 transition-colors shadow-xs"
          >
            <RefreshCw
              size={12}
              className={isSyncingAll ? "animate-spin text-primary" : ""}
            />
            {isSyncingAll ? t("syncing") : t("syncAll")}
          </button>

          <button
            onClick={() => setShowAddDialog(true)}
            className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition-colors shadow-xs"
          >
            <Plus size={13} />
            {t("addRepository")}
          </button>
        </div>
      </div>

      {/* Content */}
      {loading && repositories.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 text-muted-foreground space-y-2">
          <Loader2 size={24} className="animate-spin text-primary" />
          <p className="text-xs">{t("syncing")}</p>
        </div>
      ) : repositories.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 rounded-2xl border border-dashed border-border p-8 text-center space-y-3 bg-muted/10">
          <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-primary/10 text-primary">
            <FolderGit2 size={24} />
          </div>
          <div className="max-w-sm space-y-1">
            <h3 className="text-sm font-semibold text-foreground">
              {t("emptyTitle")}
            </h3>
            <p className="text-xs text-muted-foreground leading-relaxed">
              {t("emptyDescription")}
            </p>
          </div>
          <button
            onClick={() => setShowAddDialog(true)}
            className="mt-2 inline-flex items-center gap-1.5 rounded-xl bg-primary px-4 py-2 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition-colors shadow-xs"
          >
            <Plus size={14} />
            {t("addRepository")}
          </button>
        </div>
      ) : filteredRepos.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-16 rounded-2xl border border-dashed border-border p-8 text-center space-y-3 bg-muted/10">
          <Search size={22} className="text-muted-foreground" />
          <p className="text-xs text-muted-foreground">
            {t("searchNoResults")}
          </p>
          <button
            onClick={() => setSearch("")}
            className="rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground hover:bg-accent transition-colors shadow-xs"
          >
            {t("clearSearch")}
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          {filteredRepos.map((repoData) => {
            const isHighlightRepo =
              effectiveHighlightRepoId === repoData.repository.id;
            return (
              <div
                key={repoData.repository.id}
                id={`repo-${repoData.repository.id}`}
                className={
                  isHighlightRepo && !highlightItemId
                    ? "ring-2 ring-primary rounded-xl"
                    : ""
                }
              >
                <RepositoryCard
                  repoData={repoData}
                  searchFilter={search}
                  defaultCollapsed={allCollapsed && !isHighlightRepo}
                  highlightItemId={isHighlightRepo ? highlightItemId : null}
                />
              </div>
            );
          })}
        </div>
      )}

      {/* Add Repository Modal */}
      <AddRepositoryDialog
        open={showAddDialog}
        onClose={() => setShowAddDialog(false)}
      />
    </div>
  );
}
