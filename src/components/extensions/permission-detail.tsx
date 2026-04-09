import { Braces, Database, File, Globe, Terminal } from "lucide-react";
import type { Permission } from "@/lib/types";

export function PermissionDetail({ perm }: { perm: Permission }) {
  const icons: Record<string, typeof File> = {
    filesystem: File,
    network: Globe,
    shell: Terminal,
    database: Database,
    env: Braces,
  };
  const labels: Record<string, string> = {
    filesystem: "File System",
    network: "Network",
    shell: "Shell",
    database: "Database",
    env: "Environment",
  };
  const Icon = icons[perm.type] ?? File;
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
          {labels[perm.type] ?? perm.type}
        </span>
        {details.length > 0 && (
          <p className="text-xs text-muted-foreground">{details.join(", ")}</p>
        )}
      </div>
    </div>
  );
}
