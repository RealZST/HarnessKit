import {
  Brain,
  EyeOff,
  FileText,
  FolderCog,
  Settings,
  Workflow,
} from "lucide-react";
import type { AgentConfigFile, ConfigCategory } from "@/lib/types";
import { CONFIG_CATEGORY_LABELS } from "@/lib/types";
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

export function ConfigSection({
  category,
  files,
}: {
  category: ConfigCategory | "custom";
  files: AgentConfigFile[];
}) {
  if (files.length === 0) return null;
  const Icon = CATEGORY_ICONS[category] ?? Settings;
  const label = CATEGORY_LABELS[category] ?? category;

  return (
    <div className="mb-5">
      <div className="flex items-center gap-2 mb-2 px-1">
        <Icon size={14} className="text-muted-foreground" />
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {label}
        </span>
        <span className="text-[10px] bg-muted px-1.5 py-0.5 rounded-full text-muted-foreground">
          {files.length}
        </span>
      </div>
      <div className="rounded-lg border border-border overflow-hidden">
        {files.map((file) => (
          <ConfigFileEntry key={file.path} file={file} />
        ))}
      </div>
    </div>
  );
}
