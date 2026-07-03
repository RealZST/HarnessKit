import type { AgentConfigFile } from "@/lib/types";

export interface MemoryGroup {
  /** Storage directory (…/projects/<encoded>/memory), raw and never decoded.
   *  Doubles as the stable identity (React key / collapse-state key) and the
   *  header title. */
  storePath: string;
  /** Project name when project-scoped, else null (Global). */
  projectName: string | null;
  files: AgentConfigFile[];
  totalBytes: number;
}

/** The directory that physically holds a memory file (path minus file name). */
function storeDir(file: AgentConfigFile): string {
  // Separator-agnostic: backend paths are `to_string_lossy()` output, which
  // uses backslashes on Windows. Mirror the separator-safe logic used elsewhere.
  const idx = Math.max(file.path.lastIndexOf("/"), file.path.lastIndexOf("\\"));
  return idx >= 0 ? file.path.slice(0, idx) : file.path;
}

/**
 * Group memory files by storage directory. Project groups sort first (by name),
 * then Global groups (by store path); files within a group sort by name. All
 * files in one store share one owning project, so the first file's scope sets
 * the group's `projectName`.
 */
export function groupMemoryFiles(files: AgentConfigFile[]): MemoryGroup[] {
  const byDir = new Map<string, MemoryGroup>();
  for (const file of files) {
    const key = storeDir(file);
    let group = byDir.get(key);
    if (!group) {
      group = {
        storePath: key,
        projectName: file.scope.type === "project" ? file.scope.name : null,
        files: [],
        totalBytes: 0,
      };
      byDir.set(key, group);
    }
    group.files.push(file);
    group.totalBytes += file.size_bytes;
  }
  const groups = [...byDir.values()];
  for (const g of groups) {
    g.files.sort((a, b) => a.file_name.localeCompare(b.file_name));
  }
  return groups.sort((a, b) => {
    const aProj = a.projectName != null;
    const bProj = b.projectName != null;
    if (aProj !== bProj) return aProj ? -1 : 1;
    const byLabel = (a.projectName ?? a.storePath).localeCompare(
      b.projectName ?? b.storePath,
    );
    return byLabel !== 0 ? byLabel : a.storePath.localeCompare(b.storePath);
  });
}
