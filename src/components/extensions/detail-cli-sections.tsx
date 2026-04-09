import { FolderOpen, Link } from "lucide-react";
import type { Extension, ExtensionContent as ExtContent, GroupedExtension } from "@/lib/types";
import { agentDisplayName } from "@/lib/types";

interface CliSectionsProps {
  group: GroupedExtension;
  extensions: Extension[];
  instanceData: Map<string, ExtContent>;
  skillLocations: [string, string, string | null][];
}

export function CliSections({ group, extensions, instanceData, skillLocations }: CliSectionsProps) {
  if (group.kind !== "cli") return null;

  return (
    <>
      {/* CLI Details */}
      {group.instances[0]?.cli_meta &&
        (() => {
          const cli_meta = group.instances[0].cli_meta!;
          return (
            <div className="mt-4 space-y-3 text-sm">
              <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                CLI Details
              </h4>
              <div className="grid grid-cols-2 gap-2 text-muted-foreground">
                <span>Binary:</span>
                <span className="font-mono">{cli_meta.binary_name}</span>
                {cli_meta.version && (
                  <>
                    <span>Version:</span>
                    <span>{cli_meta.version}</span>
                  </>
                )}
                {cli_meta.install_method && (
                  <>
                    <span>Installed via:</span>
                    <span>{cli_meta.install_method}</span>
                  </>
                )}
                {cli_meta.binary_path && (
                  <>
                    <span>Path:</span>
                    <span className="font-mono text-xs break-all">
                      {cli_meta.binary_path}
                    </span>
                  </>
                )}
                {cli_meta.credentials_path && (
                  <>
                    <span>Credentials:</span>
                    <span className="font-mono text-xs break-all">
                      {cli_meta.credentials_path}
                    </span>
                  </>
                )}
              </div>
              {cli_meta.api_domains.length > 0 && (
                <div>
                  <span className="text-muted-foreground">API Domains:</span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {cli_meta.api_domains.map((d) => (
                      <span
                        key={d}
                        className="text-xs px-2 py-0.5 bg-muted rounded-full"
                      >
                        {d}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </div>
          );
        })()}

      {/* CLI Associated Extensions (Skills + MCPs) */}
      {(() => {
        const children = extensions.filter(
          (e) => e.cli_parent_id === group.instances[0]?.id,
        );
        const mcps = children.filter((e) => e.kind === "mcp");
        // Group skill locations by agent for display
        const agentSkillPaths = new Map<string, { path: string; symlink: string | null }[]>();
        for (const [agent, path, symlink] of skillLocations) {
          const list = agentSkillPaths.get(agent) ?? [];
          list.push({ path, symlink });
          agentSkillPaths.set(agent, list);
        }
        return children.length > 0 ? (
          <div className="mt-4 space-y-3">
            {agentSkillPaths.size > 0 && (
              <div>
                <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Associated Skills
                </h4>
                <div className="space-y-3">
                  {[...agentSkillPaths.entries()].map(([agent, paths]) => (
                    <div
                      key={agent}
                      className="rounded-lg border border-border bg-card p-3"
                    >
                      <span className="text-sm font-medium">
                        {agentDisplayName(agent)}
                      </span>
                      <div className="mt-1.5 space-y-1">
                        {paths.map((p) => (
                          <div key={p.path}>
                            <div className="flex items-start gap-2 text-muted-foreground">
                              <FolderOpen size={12} className="mt-0.5 shrink-0" />
                              <span className="break-all text-xs">{p.path}</span>
                            </div>
                            {p.symlink && (
                              <div className="flex items-start gap-2 text-muted-foreground/70 ml-0">
                                <Link size={12} className="mt-0.5 shrink-0" />
                                <span className="break-all text-xs italic">{p.symlink}</span>
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {mcps.length > 0 && (
              <div>
                <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Associated MCP Servers
                </h4>
                <div className="space-y-3">
                  {mcps.map((child) => {
                    const mcpData = instanceData.get(child.id);
                    return (
                      <div
                        key={child.id}
                        className="rounded-lg border border-border bg-card p-3"
                      >
                        <span className="text-sm font-medium">
                          {agentDisplayName(child.agents[0] ?? "unknown")}
                        </span>
                        <div className="mt-1.5 space-y-1">
                          {mcpData?.path && (
                            <div className="flex items-start gap-2 text-muted-foreground">
                              <FolderOpen size={12} className="mt-0.5 shrink-0" />
                              <span className="break-all text-xs">{mcpData.path}</span>
                            </div>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>
        ) : null;
      })()}
    </>
  );
}
