import {
  Brain,
  ChevronDown,
  ChevronRight,
  EyeOff,
  FileText,
  FolderCog,
  Settings,
  Workflow,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { AgentConfigFile, ConfigCategory } from "@/lib/types";
import { CONFIG_CATEGORY_LABELS } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { ConfigFileEntry } from "./config-file-entry";

const CATEGORY_ICONS: Record<string, React.ElementType> = {
  rules: FileText,
  memory: Brain,
  settings: Settings,
  workflow: Workflow,
  ignore: EyeOff,
  custom: FolderCog,
};

const CATEGORY_LABELS: Record<string, string> = {
  ...CONFIG_CATEGORY_LABELS,
  custom: "Custom",
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
  const storageKey = agentName ? collapseStorageKey(agentName, category) : null;
  const pendingFocusFile = useAgentConfigStore((s) => s.pendingFocusFile);

  const [collapsed, setCollapsed] = useState<boolean>(() => {
    if (!storageKey) return false;
    return localStorage.getItem(storageKey) === "1";
  });

  // When the user switches agents the storageKey changes; rehydrate from disk.
  useEffect(() => {
    if (!storageKey) return;
    setCollapsed(localStorage.getItem(storageKey) === "1");
  }, [storageKey]);

  // If the user navigates here with a focus target (e.g. clicked a file in the
  // Overview's Agent Activity widget), and that file lives in this section,
  // force-open it. We also clear the persisted collapse state so the section
  // doesn't snap shut once the focus signal is consumed — the user can
  // re-collapse with the chevron if they want.
  const containsFocusFile =
    pendingFocusFile != null && files.some((f) => f.path === pendingFocusFile);
  useEffect(() => {
    if (!containsFocusFile || !collapsed) return;
    setCollapsed(false);
    if (storageKey) localStorage.removeItem(storageKey);
  }, [containsFocusFile, collapsed, storageKey]);

  const toggle = () => {
    setCollapsed((prev) => {
      const next = !prev;
      if (storageKey) {
        if (next) localStorage.setItem(storageKey, "1");
        else localStorage.removeItem(storageKey);
      }
      return next;
    });
  };

  if (files.length === 0) return null;
  const Icon = CATEGORY_ICONS[category] ?? Settings;
  const label = CATEGORY_LABELS[category] ?? category;
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
          {files.map((file) => (
            <ConfigFileEntry key={file.path} file={file} />
          ))}
        </div>
      )}
    </div>
  );
}
