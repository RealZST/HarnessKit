import { clsx } from "clsx";
import { ChevronsUpDown, Loader2, Plus, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { Hint } from "@/components/shared/hint";
import { SearchField } from "@/components/shared/search-field";
import { humanizeError } from "@/lib/errors";
import type { RepositoryItem, RepositoryWithItems } from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";
import { AddRepositoryDialog } from "./add-repository-dialog";
import { RepositoryCard } from "./repository-card";
import { useInstalledCopies } from "./use-installed-copies";

const FILTERS = ["all", "available", "installed", "ignored"] as const;
type Filter = (typeof FILTERS)[number];

/** Whether the search names the repository itself, and the items it lets
 *  through: all of them then, else the ones it names. */
function searchRepository(repo: RepositoryWithItems, query: string) {
  const q = query.trim().toLowerCase();
  const named =
    q !== "" && `${repo.name} ${repo.url}`.toLowerCase().includes(q);
  const items =
    !q || named
      ? repo.items
      : repo.items.filter((item) =>
          `${item.name} ${item.description}`.toLowerCase().includes(q),
        );
  return { repo, named, items };
}

const ACTION =
  "flex shrink-0 items-center gap-1 whitespace-nowrap rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md";

/** The Marketplace's Following view: every tracked repository as a
 *  catalog of skills to install. `?repo=` names one to open and scroll to. */
export function RepositoriesView({
  onSearchDiscover,
}: {
  /** Take a search that found nothing here to the public registry. */
  onSearchDiscover: (query: string) => void;
}) {
  const { t } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const { t: tm } = useTranslation("marketplace");
  const installedCopies = useInstalledCopies();
  const [searchParams, setSearchParams] = useSearchParams();
  const repositories = useRepositoryStore((s) => s.repositories);
  const loaded = useRepositoryStore((s) => s.loaded);
  const fetchAndSyncStale = useRepositoryStore((s) => s.fetchAndSyncStale);
  const syncAll = useRepositoryStore((s) => s.syncAll);
  const progress = useRepositoryStore((s) => s.syncAllProgress);
  const focusId = searchParams.get("repo");
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [adding, setAdding] = useState(false);
  // The toolbar's choice for every card; until it is made, cards open only
  // where there is one thing to look at.
  const [allExpanded, setAllExpanded] = useState<boolean | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const load = useCallback(() => {
    setLoadError(null);
    fetchAndSyncStale().catch((e) => {
      // An earlier visit's list stays up, so the error cannot take its place.
      if (useRepositoryStore.getState().loaded) toast.error(humanizeError(e));
      else setLoadError(humanizeError(e));
    });
  }, [fetchAndSyncStale]);
  useEffect(load, [load]);
  const focusListed = repositories.some((repo) => repo.id === focusId);
  useEffect(() => {
    if (focusListed && focusId) {
      document.getElementById(focusId)?.scrollIntoView({ block: "nearest" });
    }
  }, [focusListed, focusId]);

  // Each failure is also recorded on its own repository.
  async function reportSyncAll() {
    const outcome = await syncAll();
    if (!outcome) return;
    if (outcome.failed > 0) {
      toast.error(
        t("syncAllFailed", { count: outcome.failed, total: outcome.total }),
      );
    } else {
      toast.success(t("syncedCount", { count: outcome.total }));
    }
  }

  const inFilter: Record<Filter, (item: RepositoryItem) => boolean> = {
    all: (item) => !item.ignored,
    available: (item) => !item.ignored && installedCopies(item).length === 0,
    installed: (item) => installedCopies(item).length > 0,
    ignored: (item) => item.ignored,
  };
  const searched = repositories.map((repo) => searchRepository(repo, query));
  const narrowed = query.trim() !== "" || filter !== "all";
  const expandedByDefault =
    allExpanded ?? (narrowed || repositories.length === 1);
  // A repository the search names stays listed even with no skills to show,
  // so one that was never synced can still be found and synced.
  const shown = searched
    .map(({ repo, named, items }) => ({
      repo,
      named,
      items: items.filter(inFilter[filter]),
    }))
    .filter(
      ({ named, items }) =>
        !narrowed || items.length > 0 || (named && filter === "all"),
    );

  if (!loaded) {
    return loadError ? (
      <div className="flex flex-col items-center gap-3 py-12 text-sm text-muted-foreground">
        <p role="alert">{loadError}</p>
        <button
          onClick={load}
          className="rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm hover:bg-accent"
        >
          {tc("actions.retry")}
        </button>
      </div>
    ) : (
      <div role="status" className="flex justify-center py-12">
        <Loader2 size={20} className="animate-spin text-muted-foreground" />
        <span className="sr-only">{tc("status.loading")}</span>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-4">
      {repositories.length === 0 ? (
        <div className="flex flex-col items-center gap-4 py-16 text-center">
          <div className="max-w-sm space-y-1">
            <h3 className="text-sm font-semibold text-foreground">
              {t("emptyTitle")}
            </h3>
            <p className="text-sm text-muted-foreground">{t("empty")}</p>
          </div>
          <button
            onClick={() => setAdding(true)}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm hover:bg-primary/90"
          >
            <Plus size={12} />
            {t("add")}
          </button>
        </div>
      ) : (
        <>
          <div className="flex flex-wrap items-center justify-between gap-2">
            <SearchField
              value={query}
              onChange={setQuery}
              placeholder={t("searchPlaceholder")}
              ariaLabel={t("searchPlaceholder")}
              clearLabel={tm("search.clear")}
            />
            <div className="flex flex-wrap gap-2">
              <button
                onClick={() => setAllExpanded(!expandedByDefault)}
                className={ACTION}
              >
                <ChevronsUpDown size={12} />
                {expandedByDefault ? t("collapseAll") : t("expandAll")}
              </button>
              <button
                onClick={reportSyncAll}
                aria-disabled={progress !== null}
                className={clsx(
                  ACTION,
                  "tabular-nums aria-disabled:opacity-50",
                )}
              >
                <RefreshCw
                  size={12}
                  className={clsx(progress && "animate-spin")}
                />
                {progress ? t("syncAllProgress", progress) : t("syncAll")}
              </button>
              <button onClick={() => setAdding(true)} className={ACTION}>
                <Plus size={12} />
                {t("add")}
              </button>
            </div>
          </div>

          <Hint id="following-intro">{t("hint")}</Hint>

          <div className="flex flex-wrap items-center gap-2">
            {FILTERS.map((key) => (
              <button
                key={key}
                onClick={() => setFilter(key)}
                aria-pressed={filter === key}
                className={clsx(
                  "shrink-0 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                  filter === key
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground",
                )}
              >
                {t(`filters.${key}`, {
                  count: searched.reduce(
                    (n, { items }) => n + items.filter(inFilter[key]).length,
                    0,
                  ),
                })}
              </button>
            ))}
            <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
              {t("repositoryCount", { count: shown.length })}
            </span>
          </div>

          <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pb-4">
            {shown.length === 0 && (
              <div className="space-y-2 py-12 text-center text-sm text-muted-foreground">
                <p>{t("noMatches")}</p>
                {query.trim().length >= 2 && filter === "all" && (
                  <button
                    onClick={() => onSearchDiscover(query.trim())}
                    className="font-medium text-primary hover:underline"
                  >
                    {t("searchDiscover", { query: query.trim() })}
                  </button>
                )}
              </div>
            )}
            {shown.map(({ repo, items }) => (
              // Keyed by `narrowed` and the toolbar's choice too: either one
              // opens or closes every card again, whatever the user had
              // toggled.
              <RepositoryCard
                key={`${repo.id}:${narrowed}:${allExpanded}`}
                repo={repo}
                items={items}
                narrowed={narrowed}
                defaultExpanded={expandedByDefault}
                focused={repo.id === focusId}
              />
            ))}
          </div>
        </>
      )}

      {adding && (
        <AddRepositoryDialog
          onClose={() => setAdding(false)}
          onAdded={(id) =>
            setSearchParams(
              { view: "repositories", repo: id },
              { replace: true },
            )
          }
        />
      )}
    </div>
  );
}
