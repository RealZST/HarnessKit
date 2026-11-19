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
import { RepositoryAddress } from "./repository-address";
import { RepositoryItemRow } from "./repository-item-row";
import { useInstalledCopies, useIsNew } from "./use-installed-copies";

const TAG =
  "shrink-0 rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground tabular-nums";

/** One tracked repository: its sync state and the skills it holds. `items`
 *  is the part of its item list the page's search and filter let through,
 *  and `narrowed` says whether either is at work. */
export function RepositoryCard({
  repo,
  items,
  narrowed,
  defaultExpanded,
  focused,
  previewedId,
  onPreview,
  onInstall,
}: {
  repo: RepositoryWithItems;
  items: RepositoryItem[];
  narrowed: boolean;
  defaultExpanded: boolean;
  /** The repository a link from elsewhere pointed at. */
  focused: boolean;
  /** The skill of this repository the page is previewing, if any. */
  previewedId: string | undefined;
  onPreview: (item: RepositoryItem) => void;
  onInstall: (item: RepositoryItem) => void;
}) {
  const { t, i18n } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const installedCopies = useInstalledCopies();
  const isNew = useIsNew();
  const syncing = useRepositoryStore((s) => s.syncingIds.has(repo.id));
  const queued = useRepositoryStore((s) => s.queuedIds.has(repo.id));
  const busy = syncing || queued;
  const sync = useRepositoryStore((s) => s.sync);
  const remove = useRepositoryStore((s) => s.remove);
  const [toggled, setToggled] = useState<boolean | null>(null);
  const expanded = toggled ?? (defaultExpanded || focused);
  const [confirmingRemove, setConfirmingRemove] = useState(false);
  const [removing, setRemoving] = useState(false);
  const listId = useId();
  const removeTitleId = useId();

  const installedCount = repo.items.filter(
    (item) => installedCopies(item).length > 0,
  ).length;
  const newCount = repo.items.filter(isNew).length;
  const rank = (item: RepositoryItem) =>
    item.ignored ? 2 : isNew(item) ? 0 : 1;
  const rows = [...items].sort((a, b) => rank(a) - rank(b));

  return (
    <section
      id={repo.id}
      className={clsx(
        "rounded-xl border bg-card shadow-sm",
        focused ? "border-ring" : "border-border",
      )}
    >
      <div className="flex flex-wrap items-start gap-3 p-4">
        <div className="min-w-0 flex-1 basis-64 space-y-1">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <h3 className="min-w-0">
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
            {repo.last_synced_at &&
              (narrowed ? (
                <span className={TAG}>
                  {t("shownCount", {
                    count: items.length,
                    total: repo.items.length,
                  })}
                </span>
              ) : (
                <>
                  {newCount > 0 && (
                    <span
                      className={clsx(
                        TAG,
                        "inline-flex items-center gap-1 text-foreground",
                      )}
                    >
                      <span className="size-1.5 rounded-full bg-primary" />
                      {t("newCount", { count: newCount })}
                    </span>
                  )}
                  <span className={TAG}>
                    {t("skillCount", { count: repo.items.length })}
                  </span>
                  {installedCount > 0 && (
                    <span className={TAG}>
                      {t("installedCount", { count: installedCount })}
                    </span>
                  )}
                </>
              ))}
          </div>
          <div className="flex flex-wrap items-center gap-x-1.5 pl-5 text-xs text-muted-foreground">
            <RepositoryAddress repo={repo} />
            <span aria-hidden="true">·</span>
            {repo.last_synced_at ? (
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
            ) : (
              t("neverSynced")
            )}
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
              previewed={item.item_id === previewedId}
              onPreview={() => onPreview(item)}
              onInstall={() => onInstall(item)}
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
    </section>
  );
}
