import { clsx } from "clsx";
import { Download, Eye, EyeOff } from "lucide-react";
import { useTranslation } from "react-i18next";
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { humanizeError } from "@/lib/errors";
import {
  agentDisplayName,
  type InstalledCopy,
  type RepositoryItem,
  sortAgentNames,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";

export const INSTALL_BUTTON =
  "inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-accent";

/** One skill of a tracked repository: `copies`, where it is installed in the
 *  scope being viewed, and the ways to read, install, ignore or restore it. */
export function RepositoryItemRow({
  repoId,
  item,
  copies,
  isNew,
  previewed,
  onPreview,
  onInstall,
}: {
  repoId: string;
  item: RepositoryItem;
  copies: InstalledCopy[];
  isNew: boolean;
  previewed: boolean;
  onPreview: () => void;
  onInstall: () => void;
}) {
  const { t } = useTranslation("repositories");
  const { t: tc } = useTranslation("common");
  const { t: tm } = useTranslation("marketplace");
  const setIgnored = useRepositoryStore((s) => s.setIgnored);
  const agentOrder = useAgentStore((s) => s.agentOrder);

  return (
    <div
      className={clsx(
        "flex flex-wrap items-center justify-between gap-x-4 gap-y-2 px-4 py-3 transition-colors",
        previewed ? "bg-accent" : "hover:bg-accent/50",
      )}
    >
      <button
        type="button"
        onClick={onPreview}
        aria-pressed={previewed}
        aria-label={tm("item.viewDetails", { name: item.name })}
        className="min-w-0 flex-1 basis-56 space-y-1 text-left"
      >
        <span className="flex flex-wrap items-center gap-2">
          <span
            className={clsx(
              "text-sm font-medium",
              item.ignored ? "text-muted-foreground" : "text-foreground",
            )}
          >
            {item.name}
          </span>
          {item.ignored && (
            <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
              {t("ignoredTag")}
            </span>
          )}
          {isNew && (
            <span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-foreground">
              <span className="size-1.5 rounded-full bg-primary" />
              {t("newTag")}
            </span>
          )}
          <span className="flex flex-wrap items-end gap-1">
            {sortAgentNames(
              [...new Set(copies.map((copy) => copy.agent))],
              agentOrder,
            ).map((name) => (
              <span
                key={name}
                title={agentDisplayName(name)}
                className="flex items-end justify-center"
                style={{ width: 20, height: 20 }}
              >
                <AgentMascot name={name} size={18} />
              </span>
            ))}
          </span>
        </span>
        {item.description && (
          <span className="line-clamp-2 text-xs text-muted-foreground">
            {item.description}
          </span>
        )}
      </button>
      <div className="flex shrink-0 items-center gap-1">
        <button
          onClick={onInstall}
          aria-label={t("installNamed", { name: item.name })}
          className={INSTALL_BUTTON}
        >
          <Download size={12} className="text-primary" />
          {tc("actions.install")}
        </button>
        {/* Ignoring is for skills the user does not want, so one that is
            installed here only offers the way back. The slot stays, keeping
            the Install buttons in one column. */}
        <span className="flex w-8 justify-center">
          {(item.ignored || copies.length === 0) && (
            <button
              onClick={() =>
                setIgnored(repoId, item.item_id, !item.ignored).catch((e) =>
                  toast.error(humanizeError(e)),
                )
              }
              aria-label={t(item.ignored ? "restoreNamed" : "ignoreNamed", {
                name: item.name,
              })}
              title={item.ignored ? t("restore") : t("ignore")}
              className="rounded-lg p-2 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
            >
              {item.ignored ? <Eye size={13} /> : <EyeOff size={13} />}
            </button>
          )}
        </span>
      </div>
    </div>
  );
}
