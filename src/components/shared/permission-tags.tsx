import { File, Globe, Terminal, Database, Key } from "lucide-react";
import type { Permission } from "@/lib/types";

const iconMap: Record<string, typeof File> = {
  filesystem: File,
  network: Globe,
  shell: Terminal,
  database: Database,
  env: Key,
};

export function PermissionTags({ permissions }: { permissions: Permission[] }) {
  return (
    <div className="flex gap-1">
      {permissions.map((p) => {
        const Icon = iconMap[p.type] ?? File;
        return (
          <span key={p.type} className="text-muted-foreground" title={p.type}>
            <Icon size={14} aria-hidden="true" />
          </span>
        );
      })}
    </div>
  );
}
