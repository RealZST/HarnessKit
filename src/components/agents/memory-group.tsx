import { ChevronDown, ChevronRight } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useCollapsibleState } from "@/hooks/use-collapsible-state";
import { formatBytes } from "@/lib/format";
import { ConfigFileEntry } from "./config-file-entry";
import type { MemoryGroup as MemoryGroupData } from "./memory-grouping";

/** localStorage key for one memory group's collapse state. */
function groupCollapseKey(agent: string, storePath: string): string {
  return `agent-detail-collapse:memory-group:${agent}:${storePath}`;
}

export function MemoryGroup({
  group,
  agentName,
}: {
  group: MemoryGroupData;
  agentName?: string;
}) {
  const { t } = useTranslation("agents");
  const { t: tc } = useTranslation("common");
  const storageKey = agentName
    ? groupCollapseKey(agentName, group.storePath)
    : null;
  const { collapsed, toggle } = useCollapsibleState(storageKey);

  const Chevron = collapsed ? ChevronRight : ChevronDown;
  const isProject = group.projectName != null;

  return (
    <div>
      <button
        type="button"
        onClick={toggle}
        aria-expanded={!collapsed}
        className="w-full flex items-center gap-2 px-3 py-2 border-b border-border/50 last:border-b-0 hover:bg-accent/20 transition-colors text-left"
      >
        <Chevron size={13} className="shrink-0 text-muted-foreground" />
        <span
          className="text-[12px] font-medium truncate"
          title={group.storePath}
        >
          {isProject ? group.projectName : group.storePath}
        </span>
        <span
          className={
            isProject
              ? "text-[10px] px-1.5 py-0.5 rounded-full bg-tag-project/10 text-tag-project shrink-0"
              : "text-[10px] px-1.5 py-0.5 rounded-full bg-tag-global/10 text-tag-global shrink-0"
          }
        >
          {isProject ? tc("scope.project") : tc("scope.global")}
        </span>
        <span className="ml-auto text-[10px] text-muted-foreground shrink-0">
          {t("memory.fileCount", { count: group.files.length })} ·{" "}
          {formatBytes(group.totalBytes)}
        </span>
      </button>
      {!collapsed &&
        group.files.map((file) => (
          <ConfigFileEntry key={file.path} file={file} hideScopeMeta />
        ))}
    </div>
  );
}
