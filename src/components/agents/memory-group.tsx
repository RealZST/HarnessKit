import { ChevronDown, ChevronRight, Folder } from "lucide-react";
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
        className="w-full flex items-center gap-2 px-3 py-2 bg-muted/40 border-b border-border hover:bg-muted/60 transition-colors text-left"
      >
        <Chevron size={13} className="shrink-0 text-muted-foreground" />
        <Folder size={13} className="shrink-0 text-muted-foreground" />
        {isProject ? (
          <>
            <span
              className="text-[12px] font-semibold shrink-0"
              title={group.storePath}
            >
              {group.projectName}
            </span>
            <span
              className="text-[11px] text-muted-foreground truncate min-w-0"
              title={group.storePath}
            >
              {group.storePath}
            </span>
          </>
        ) : (
          <span
            className="text-[12px] font-semibold truncate min-w-0"
            title={group.storePath}
          >
            {group.storePath}
          </span>
        )}
        <span className="ml-auto text-[10px] text-muted-foreground shrink-0">
          {t("memory.fileCount", { count: group.files.length })} ·{" "}
          {formatBytes(group.totalBytes)}
        </span>
      </button>
      {!collapsed &&
        group.files.map((file) => (
          <ConfigFileEntry key={file.path} file={file} hideScopePath inset />
        ))}
    </div>
  );
}
