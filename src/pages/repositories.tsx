import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, RefreshCw, Search, FolderGit2, Loader2 } from "lucide-react";
import { useRepositoryStore } from "@/stores/repository-store";
import { RepositoryCard } from "@/components/repositories/repository-card";
import { AddRepositoryDialog } from "@/components/repositories/add-repository-dialog";

export default function RepositoriesPage() {
  const { t } = useTranslation("repositories");
  const { repositories, loading, isSyncingAll, fetch, syncAll } = useRepositoryStore();

  const [search, setSearch] = useState("");
  const [showAddDialog, setShowAddDialog] = useState(false);

  useEffect(() => {
    fetch().catch((e) => console.error("Failed to fetch repositories:", e));
  }, [fetch]);

  return (
    <div className="flex flex-col h-full overflow-y-auto p-6 md:p-8 space-y-6">
      {/* Page Header */}
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
            <RefreshCw size={13} className={isSyncingAll ? "animate-spin text-primary" : ""} />
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

      {/* Search Bar */}
      <div className="relative">
        <Search
          size={16}
          className="absolute left-3.5 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none"
        />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t("searchPlaceholder")}
          className="w-full rounded-xl border border-border bg-card pl-10 pr-4 py-2 text-xs text-foreground placeholder:text-muted-foreground focus:border-primary focus:outline-hidden shadow-2xs"
        />
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
      ) : (
        <div className="space-y-5">
          {repositories.map((repoData) => (
            <RepositoryCard
              key={repoData.repository.id}
              repoData={repoData}
              searchFilter={search}
            />
          ))}
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
