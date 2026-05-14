import { Braces, Database, File, Globe, Terminal } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Permission } from "@/lib/types";

type PermissionLabelKey =
  | "filesystem"
  | "network"
  | "shell"
  | "database"
  | "environment";

const PERMISSION_LABEL_KEYS: Record<string, PermissionLabelKey> = {
  filesystem: "filesystem",
  network: "network",
  shell: "shell",
  database: "database",
  env: "environment",
};

export function PermissionDetail({ perm }: { perm: Permission }) {
  const { t } = useTranslation("extensions");
  const icons: Record<string, typeof File> = {
    filesystem: File,
    network: Globe,
    shell: Terminal,
    database: Database,
    env: Braces,
  };
  const Icon = icons[perm.type] ?? File;
  const labelKey = PERMISSION_LABEL_KEYS[perm.type];
  const details =
    "paths" in perm
      ? perm.paths
      : "domains" in perm
        ? perm.domains
        : "commands" in perm
          ? perm.commands
          : "engines" in perm
            ? perm.engines
            : "keys" in perm
              ? perm.keys
              : [];

  return (
    <div className="flex items-start gap-2 text-sm">
      <Icon size={14} className="mt-0.5 shrink-0 text-muted-foreground" />
      <div>
        <span className="font-medium text-foreground">
          {labelKey ? t(`permissions.${labelKey}`) : perm.type}
        </span>
        {details.length > 0 && (
          <p className="text-xs text-muted-foreground">{details.join(", ")}</p>
        )}
      </div>
    </div>
  );
}
