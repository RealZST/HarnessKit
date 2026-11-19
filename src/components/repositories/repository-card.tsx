import { clsx } from "clsx";
import { AlertTriangle, ChevronDown, FolderX, RefreshCw } from "lucide-react";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Modal } from "@/components/ui/modal";
import { humanizeError } from "@/lib/errors";
import {
  formatRelativeTime,
  type RepositoryItem,
  type RepositoryWithItems,
} from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";
import { InstallItemDialog } from "./install-item-dialog";
import { RepositoryAddress } from "./repository-address";
import { RepositoryItemRow } from "./repository-item-row";
import { useInstalledCopies } from "./use-installed-copies";

/** One tracked repository: its sync state and the skills it holds. `items`
 *  is the part of its item list the page's search and filter let through,
 *  and `narrowed` says whether either is at work. */
export function RepositoryCard({
  repo,
  items,
  narrowed,
  defaultExpanded,
}: {
  repo: RepositoryWithItems;
  items: RepositoryItem[];
  narrowed: boolean;
  defaultExpanded: boolean;
}) {
  const { t, i18n } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const installedCopies = useInstalledCopies();
  const syncing = useRepositoryStore((s) => s.syncingIds.has(repo.id));
  const queued = useRepositoryStore((s) => s.queuedIds.has(repo.id));
  const busy = syncing || queued;
  const sync = useRepositoryStore((s) => s.sync);
  const remove = useRepositoryStore((s) => s.remove);
  const [toggled, setToggled] = useState<boolean | null>(null);
  const expanded = toggled ?? defaultExpanded;
  const [confirmingRemove, setConfirmingRemove] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [installingId, setInstallingId] = useState<string | null>(null);
  const installing = repo.items.find((item) => item.item_id === installingId);
  const listId = useId();
  const removeTitleId = useId();

  const installedCount = repo.items.filter(
    (item) => installedCopies(item).length > 0,
  ).length;
  // New marks a skill the user has not dealt with yet.
  const isNew = (item: RepositoryItem) =>
    item.is_new && !item.ignored && installedCopies(item).length === 0;
  const newCount = repo.items.filter(isNew).length;
  const rank = (item: RepositoryItem) =>
    item.ignored ? 2 : isNew(item) ? 0 : 1;
  const rows = [...items].sort((a, b) => rank(a) - rank(b));

  return (
    <section
      id={repo.id}
      className="rounded-xl border border-border bg-card shadow-sm"
    >
      <div className="flex flex-wrap items-start gap-3 p-4">
        <div className="min-w-0 flex-1 basis-64 space-y-1">
          <h3>
            <button
              onClick={() => setToggled(!expanded)}
              aria-expanded={expanded}
              aria-controls={listId}
              className="flex max-w-full items-center gap-1.5 text-left"
            >
              <ChevronDown
                size={14}
                className={clsx(
                  "shrink-0 text-muted-foreground transition-transform duration-200",
                  !expanded && "-rotate-90",
                )}
              />
              <span className="truncate text-sm font-semibold text-foreground">
                {repo.name}
              </span>
            </button>
          </h3>
          <div className="flex flex-wrap items-center gap-x-3 pl-5 text-xs text-muted-foreground">
            <RepositoryAddress repo={repo} />
            <span className="min-w-0">
              {!repo.last_synced_at ? (
                t("neverSynced")
              ) : narrowed ? (
                t("shownCount", {
                  count: items.length,
                  total: repo.items.length,
                })
              ) : (
                <>
                  {newCount > 0 && (
                    <span className="font-medium text-foreground">
                      <span className="mr-1 inline-block size-1.5 rounded-full bg-primary align-middle" />
                      {t("newCount", { count: newCount })}
                      {" · "}
                    </span>
                  )}
                  {t("skillCount", { count: repo.items.length })}
                  {installedCount > 0 &&
                    ` · ${t("installedCount", { count: installedCount })}`}
                  {" · "}
                  <span
                    title={new Date(repo.last_synced_at).toLocaleString(
                      i18n.resolvedLanguage,
                    )}
                  >
                    {t("synced", {
                      time: formatRelativeTime(
                        repo.last_synced_at,
                      ).toLocaleLowerCase(),
                    })}
                  </span>
                </>
              )}
            </span>
          </div>
          {repo.last_commit_hash && (
            <p
              title={repo.last_commit_message ?? undefined}
              className="truncate pl-5 text-xs text-muted-foreground"
            >
              <span className="font-mono">
                {repo.last_commit_hash.slice(0, 7)}
              </span>{" "}
              {repo.last_commit_message}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            // The reason for a failure is recorded on the repository and
            // shown below.
            onClick={() =>
              !busy &&
              sync(repo.id).then(
                () => toast.success(t("syncedNamed", { name: repo.name })),
                () => toast.error(t("syncFailedNamed", { name: repo.name })),
              )
            }
            aria-disabled={busy}
            aria-label={t("syncNamed", { name: repo.name })}
            className="inline-flex min-w-20 items-center justify-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent aria-disabled:opacity-50"
          >
            <RefreshCw size={12} className={clsx(syncing && "animate-spin")} />
            {syncing
              ? t("syncing")
              : queued
                ? t("queued")
                : tc("actions.refresh")}
          </button>
          <button
            onClick={() => setConfirmingRemove(true)}
            disabled={busy}
            aria-label={t("removeNamed", { name: repo.name })}
            className="rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent disabled:opacity-50"
          >
            {t("remove")}
          </button>
        </div>
      </div>

      {repo.last_sync_error && (
        <div className="mx-4 mb-4 flex items-start gap-2 rounded-lg bg-muted/50 px-3 py-2 text-xs">
          <AlertTriangle
            size={14}
            className="mt-0.5 shrink-0 text-destructive"
          />
          <div className="min-w-0 flex-1 space-y-0.5">
            <p className="font-medium text-foreground">{t("syncFailed")}</p>
            <p className="whitespace-pre-wrap text-muted-foreground [overflow-wrap:anywhere]">
              {repo.last_sync_error}
            </p>
          </div>
        </div>
      )}

      {expanded && (
        <div
          id={listId}
          className="divide-y divide-border/60 border-t border-border/60"
        >
          {items.length === 0 && (
            <p className="px-4 py-6 text-center text-xs text-muted-foreground">
              {!repo.last_synced_at
                ? t("neverSyncedHint")
                : repo.items.length === 0
                  ? t("noItems")
                  : t("noMatches")}
            </p>
          )}
          {rows.map((item) => (
            <RepositoryItemRow
              key={item.item_id}
              repoId={repo.id}
              item={item}
              copies={installedCopies(item)}
              isNew={isNew(item)}
              onInstall={() => setInstallingId(item.item_id)}
            />
          ))}
        </div>
      )}

      {confirmingRemove && (
        <Modal
          onClose={() => setConfirmingRemove(false)}
          busy={removing}
          ariaLabelledBy={removeTitleId}
          containerClassName="w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl"
        >
          <div className="mb-4 flex items-center gap-2">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
              <FolderX size={16} />
            </span>
            <div>
              <h3
                id={removeTitleId}
                className="text-sm font-semibold text-foreground"
              >
                {t("removeTitle", { name: repo.name })}
              </h3>
              <p className="text-xs text-muted-foreground">
                <span className="font-medium text-foreground">
                  {t("removeKeeps")}
                </span>{" "}
                {t("removeDescription")}
              </p>
            </div>
          </div>
          <button
            type="button"
            onClick={() => {
              // On success the card unmounts with the repository.
              setRemoving(true);
              remove(repo.id).catch((e) => {
                toast.error(humanizeError(e));
                setRemoving(false);
              });
            }}
            disabled={removing}
            className="flex w-full items-center justify-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {t("remove")}
          </button>
          <button
            type="button"
            onClick={() => setConfirmingRemove(false)}
            disabled={removing}
            className="mt-2 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
          >
            {tc("actions.cancel")}
          </button>
        </Modal>
      )}
      {installing && (
        <InstallItemDialog
          repoId={repo.id}
          item={installing}
          onClose={() => setInstallingId(null)}
        />
      )}
    </section>
  );
}
