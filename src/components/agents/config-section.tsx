import {
  Bot,
  Brain,
  ChevronDown,
  ChevronRight,
  EyeOff,
  FileText,
  FolderCog,
  Settings,
  Workflow,
} from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useCollapsibleState } from "@/hooks/use-collapsible-state";
import type { AgentConfigFile, ConfigCategory } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { ConfigFileEntry } from "./config-file-entry";
import { MemoryGroup } from "./memory-group";
import { groupMemoryFiles } from "./memory-grouping";

const CATEGORY_ICONS: Record<string, React.ElementType> = {
  rules: FileText,
  memory: Brain,
  subagents: Bot,
  settings: Settings,
  workflow: Workflow,
  ignore: EyeOff,
  custom: FolderCog,
};

/** localStorage key for the collapse state of one (agent, category) pair. */
function collapseStorageKey(agent: string, category: string): string {
  return `agent-detail-collapse:${agent}:${category}`;
}

export function ConfigSection({
  category,
  files,
  agentName,
}: {
  category: ConfigCategory | "custom";
  files: AgentConfigFile[];
  /** Used to scope collapse state in localStorage so each agent remembers
   *  its own preferences. When omitted, collapse state is session-only. */
  agentName?: string;
}) {
  const { t } = useTranslation("common");
  const storageKey = agentName ? collapseStorageKey(agentName, category) : null;
  const pendingFocusFile = useAgentConfigStore((s) => s.pendingFocusFile);

  const { collapsed, setCollapsed, toggle } = useCollapsibleState(storageKey);

  // If the user navigates here with a focus target (e.g. clicked a file in the
  // Overview's Agent Activity widget), and that file lives in this section,
  // force-open it. Setting collapsed=false also clears the persisted state so
  // the section doesn't snap shut once the focus signal is consumed — the user
  // can re-collapse with the chevron if they want.
  const containsFocusFile =
    pendingFocusFile != null && files.some((f) => f.path === pendingFocusFile);
  useEffect(() => {
    if (containsFocusFile && collapsed) setCollapsed(false);
  }, [containsFocusFile, collapsed, setCollapsed]);

  if (files.length === 0) return null;
  const Icon = CATEGORY_ICONS[category] ?? Settings;
  const label = t(`configCategories.${category}` as const);
  const Chevron = collapsed ? ChevronRight : ChevronDown;

  return (
    <div className="mb-5" id={`section-${category}`}>
      <button
        type="button"
        onClick={toggle}
        aria-expanded={!collapsed}
        className="w-full flex items-center gap-1.5 mb-2 px-1 hover:opacity-80 transition-opacity text-left"
      >
        <Chevron size={12} className="text-muted-foreground" />
        <Icon size={14} className="text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
        <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground">
          {files.length}
        </span>
      </button>
      {!collapsed && (
        <div className="rounded-lg border border-border overflow-hidden">
          {category === "memory"
            ? groupMemoryFiles(files).map((group) => (
                <MemoryGroup
                  key={group.storePath}
                  group={group}
                  agentName={agentName}
                />
              ))
            : files.map((file) => (
                <ConfigFileEntry key={file.path} file={file} />
              ))}
        </div>
      )}
    </div>
  );
}
