import { FolderOpen, Link } from "lucide-react";
import type {
  ExtensionContent as ExtContent,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName, sortAgentNames } from "@/lib/types";

interface DetailPathsProps {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  skillLocations: [string, string, string | null][];
  agentOrder: readonly string[];
}

export function DetailPaths({
  group,
  instanceData,
  skillLocations,
  agentOrder,
}: DetailPathsProps) {
  if (group.kind === "cli" || group.instances.length === 0) return null;

  // skillLocations is scope-agnostic on purpose (the get_skill_locations
  // API surfaces every place a skill named X exists, used by other UIs).
  // For the detail panel we only care about paths that actually belong to
  // *this* group's instances — otherwise a project-level skill row would
  // mistakenly show its same-named global cousin's path. Build a set of
  // directories referenced by this group's instances' source_path and
  // filter skillLocations against it.
  const instanceDirs = new Set(
    group.instances
      .map((inst) => inst.source_path)
      .filter((p): p is string => !!p)
      .map((p) => p.replace(/\/SKILL\.md(\.disabled)?$/, "")),
  );
  const filteredLocations =
    instanceDirs.size > 0
      ? skillLocations.filter(([, dir]) => instanceDirs.has(dir))
      : skillLocations;

  return (
    <div className="mt-4">
      <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        Paths
      </h4>
      <div className="space-y-3">
        {(() => {
          // Group instances by agent, sorted by agent order
          const byAgent = new Map<string, typeof group.instances>();
          for (const inst of group.instances) {
            const agent = inst.agents[0] ?? "unknown";
            const list = byAgent.get(agent) ?? [];
            list.push(inst);
            byAgent.set(agent, list);
          }
          const sortedAgentNames = sortAgentNames(
            [...byAgent.keys()],
            agentOrder,
          );
          return sortedAgentNames.map((agentName) => {
            const instances = byAgent.get(agentName)!;
            const firstData = instanceData.get(instances[0].id);
            const agentLocations = filteredLocations.filter(
              ([a]) => a === agentName,
            );
            // Collect unique event names for hooks
            const hookEvents =
              group.kind === "hook"
                ? [
                    ...new Set(
                      instances
                        .map((inst) => {
                          const parts = inst.name.split(":");
                          return parts.length >= 1 ? parts[0] : "";
                        })
                        .filter(Boolean),
                    ),
                  ]
                : [];
            return (
              <div
                key={agentName}
                className="rounded-lg border border-border bg-card p-3"
              >
                <span className="text-sm font-medium">
                  {agentDisplayName(agentName)}
                </span>
                <div
                  className={`mt-1.5 space-y-1 ${!group.enabled ? "opacity-50" : ""}`}
                >
                  {agentLocations.length > 0 ? (
                    agentLocations.map(([, path, symlink]) => (
                      <div key={path}>
                        <div className="flex items-start gap-2 text-muted-foreground">
                          <FolderOpen size={12} className="mt-0.5 shrink-0" />
                          <span className="break-all text-xs">{path}</span>
                        </div>
                        {(symlink ?? firstData?.symlink_target) && (
                          <div className="flex items-start gap-2 text-muted-foreground/70">
                            <Link size={12} className="mt-0.5 shrink-0" />
                            <span className="break-all text-xs italic">
                              {symlink ?? firstData?.symlink_target}
                            </span>
                          </div>
                        )}
                      </div>
                    ))
                  ) : firstData?.path ? (
                    <>
                      <div className="flex items-start gap-2 text-muted-foreground">
                        <FolderOpen size={12} className="mt-0.5 shrink-0" />
                        <span className="break-all text-xs">
                          {firstData.path}
                        </span>
                      </div>
                      {firstData?.symlink_target && (
                        <div className="flex items-start gap-2 text-muted-foreground/70">
                          <Link size={12} className="mt-0.5 shrink-0" />
                          <span className="break-all text-xs italic">
                            {firstData.symlink_target}
                          </span>
                        </div>
                      )}
                    </>
                  ) : null}
                  {hookEvents.length > 0 && (
                    <div className="flex items-center gap-2 text-muted-foreground mt-0.5">
                      <span className="text-xs">
                        {hookEvents.length === 1 ? "Event" : "Events"}:{" "}
                        {hookEvents.join(", ")}
                      </span>
                    </div>
                  )}
                </div>
              </div>
            );
          });
        })()}
      </div>
    </div>
  );
}
