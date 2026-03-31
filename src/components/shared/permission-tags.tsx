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
    <div className="flex gap-1" aria-label="Permissions">
      {permissions.map((p) => {
        const Icon = iconMap[p.type] ?? File;
        return (
          <span key={p.type} className="text-muted-foreground" title={p.type}>
            <Icon size={14} aria-hidden="true" />
            <span style={{ position: 'absolute', width: '1px', height: '1px', padding: 0, margin: '-1px', overflow: 'hidden', clip: 'rect(0,0,0,0)', whiteSpace: 'nowrap', borderWidth: 0 }}>{p.type}</span>
          </span>
        );
      })}
    </div>
  );
}
