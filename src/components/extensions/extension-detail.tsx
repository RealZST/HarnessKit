import {
  ArrowDownCircle,
  Calendar,
  Download,
  FolderOpen,
  GitBranch,
  Globe,
  Link,
  Loader2,
  Shield,
  Trash2,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { DeleteDialog } from "@/components/extensions/delete-dialog";
import { CATEGORIES } from "@/components/extensions/extension-filters";
import { PermissionDetail } from "@/components/extensions/permission-detail";
import { SkillFileSection } from "@/components/extensions/skill-file-section";
import { KindBadge } from "@/components/shared/kind-badge";
import { TrustBadge } from "@/components/shared/trust-badge";
import { api } from "@/lib/invoke";
import type { ExtensionContent as ExtContent } from "@/lib/types";
import { agentDisplayName, sortAgents } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ExtensionDetail() {
  const navigate = useNavigate();
  const grouped = useExtensionStore((s) => s.grouped);
  const selectedId = useExtensionStore((s) => s.selectedId);
  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const toggle = useExtensionStore((s) => s.toggle);
  const updateStatuses = useExtensionStore((s) => s.updateStatuses);
  const updateExtension = useExtensionStore((s) => s.updateExtension);
  const updateCategory = useExtensionStore((s) => s.updateCategory);
  const deployToAgent = useExtensionStore((s) => s.deployToAgent);
  const deleteFromAgents = useExtensionStore((s) => s.deleteFromAgents);
  const extensions = useExtensionStore((s) => s.extensions);
  const group = grouped().find((g) => g.groupKey === selectedId);
  /** Per-instance content data keyed by instance id */
  const [instanceData, setInstanceData] = useState<Map<string, ExtContent>>(
    new Map(),
  );
  const [loadingContent, setLoadingContent] = useState(false);
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const [deploying, setDeploying] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [activeInstanceId, setActiveInstanceId] = useState<string | null>(null);
  const [showDelete, setShowDelete] = useState(false);
  const [deleteAgents, setDeleteAgents] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);
  // All physical paths where this skill exists, keyed by agent name
  const [skillLocations, setSkillLocations] = useState<[string, string][]>([]);

  // Reset state and load ALL instance data when group changes
  useEffect(() => {
    if (group && group.instances.length > 0) {
      setActiveInstanceId(group.instances[0].id);
      // Load content + path for every instance in parallel
      setLoadingContent(true);
      setInstanceData(new Map());
      Promise.all(
        group.instances.map((inst) =>
          api
            .getExtensionContent(inst.id)
            .then((res) => [inst.id, res] as const)
            .catch(() => [inst.id, null] as const),
        ),
      ).then((results) => {
        const map = new Map<string, ExtContent>();
        for (const [id, data] of results) {
          if (data) map.set(id, data);
        }
        setInstanceData(map);
        setLoadingContent(false);
      });
      // Load skill locations for skills
      if (group.kind === "skill") {
        api
          .getSkillLocations(group.name)
          .then(setSkillLocations)
          .catch(() => setSkillLocations([]));
      } else {
        setSkillLocations([]);
      }
    } else {
      setActiveInstanceId(null);
      setInstanceData(new Map());
      setSkillLocations([]);
    }
    setShowDelete(false);
    setDeleteAgents(new Set());
  }, [group?.kind, group?.instances[0]?.id, group]);

  // Reset deleteAgents when showDelete is toggled on
  useEffect(() => {
    if (showDelete && group) {
      setDeleteAgents(new Set());
    }
  }, [showDelete, group]);

  if (!group) return null;

  return (
    <div
      onWheel={(e) => e.stopPropagation()}
      className="relative flex h-full flex-col rounded-xl border border-border bg-card shadow-sm"
    >
      {/* Fixed header */}
      <div className="shrink-0 flex items-start justify-between border-b border-border px-5 py-4">
        <div>
          <h3 className="text-lg font-semibold">
            {group.kind === "hook"
              ? (() => {
                  const parts = group.name.split(":");
                  if (parts.length >= 3) {
                    const command = parts.slice(2).join(":");
                    return command.split(" ").map((t) => t.split("/").pop() || t).join(" ");
                  }
                  return group.name;
                })()
              : group.name}
          </h3>
          <div className="mt-1 flex items-center gap-2">
            <KindBadge kind={group.kind} />
            {group.trust_score != null && (
              <TrustBadge score={group.trust_score} size="sm" />
            )}
            {group.trust_score != null && (
              <button
                onClick={() => navigate(`/audit?ext=${group.instances[0].id}`)}
                className="flex items-center gap-1 rounded-md px-2 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
                title="View audit details"
              >
                <Shield size={12} />
                View Audit
              </button>
            )}
            {(() => {
              const hasUpdate = group.instances.some(
                (inst) =>
                  updateStatuses.get(inst.id)?.status === "update_available",
              );
              if (!hasUpdate) return null;
              const handleUpdate = async () => {
                setUpdating(true);
                try {
                  const inst = group.instances.find(
                    (i) =>
                      updateStatuses.get(i.id)?.status === "update_available",
                  );
                  if (inst) {
                    await updateExtension(inst.id);
                    toast.success(`${group.name} updated`);
                  }
                } catch (e) {
                  toast.error(`Update failed: ${e}`);
                } finally {
                  setUpdating(false);
                }
              };
              return (
                <button
                  onClick={handleUpdate}
                  disabled={updating}
                  className="flex items-center gap-1 rounded-md bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
                >
                  {updating ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <ArrowDownCircle size={12} />
                  )}
                  {updating ? "Updating..." : "Update Available"}
                </button>
              );
            })()}
          </div>
        </div>
        <button
          onClick={() => setSelectedId(null)}
          aria-label="Close extension details"
          className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground"
        >
          <X size={18} />
        </button>
      </div>

      {/* Scrollable body */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
        {group.description && (
          <p className="text-sm text-muted-foreground">{group.description}</p>
        )}

        {/* 1. Status + Category row */}
        <div className="mt-4 flex items-center gap-2">
          <button
            onClick={() => {
              toggle(group.groupKey, !group.enabled);
              toast.success(
                `Extension ${group.enabled ? "disabled" : "enabled"}`,
              );
            }}
            aria-pressed={group.enabled}
            className={`shrink-0 rounded-full px-3 py-1 text-xs font-medium ${
              group.enabled
                ? "bg-primary/10 text-primary"
                : "bg-destructive/10 text-destructive"
            }`}
          >
            {group.enabled ? "Enabled" : "Disabled"}
          </button>
          <select
            value={group.category ?? ""}
            onChange={(e) =>
              updateCategory(group.groupKey, e.target.value || null)
            }
            aria-label="Extension category"
            className="min-w-0 flex-1 rounded-full border border-border bg-card px-2.5 py-1 text-xs text-muted-foreground focus:border-ring focus:outline-none"
          >
            <option value="">No category</option>
            {CATEGORIES.map((cat) => (
              <option key={cat} value={cat}>
                {cat}
              </option>
            ))}
          </select>
        </div>

        {/* 2. Info */}
        <div className="mt-4 space-y-2 text-sm">
          <h4 className="mb-1 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Info
          </h4>
          {(() => {
            const meta = group.instances.find(
              (i) => i.install_meta,
            )?.install_meta;
            const sourceUrl =
              meta?.url_resolved ?? meta?.url ?? group.source.url;
            const repoPath = sourceUrl
              ? (() => {
                  const m = sourceUrl.match(/github\.com\/([^/]+\/[^/]+)/);
                  return m ? m[1].replace(/\.git$/, "") : null;
                })()
              : null;
            return (
              <>
                {repoPath && (
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <Globe size={14} />
                    <a
                      href={`https://github.com/${repoPath}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="truncate hover:text-foreground transition-colors"
                      title={`https://github.com/${repoPath}`}
                    >
                      {repoPath}
                    </a>
                  </div>
                )}
              </>
            );
          })()}
          <div className="flex items-center gap-2 text-muted-foreground">
            <Calendar size={14} />
            <span>
              Installed{" "}
              {group.kind === "skill" || group.kind === "plugin"
                ? formatDate(group.installed_at)
                : "\u2014"}
            </span>
          </div>
          {group.source.origin === "git" &&
            group.source.url &&
            !group.instances.find((i) => i.install_meta) && (
              <div className="flex items-center gap-2 text-muted-foreground">
                <GitBranch size={14} />
                <span className="truncate">{group.source.url}</span>
              </div>
            )}
        </div>

        {/* 3. Agents + Deploy */}
        <div className="mt-4">
          <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Agents
          </h4>
          <div className="flex flex-wrap gap-1">
            {group.agents.map((agent) => (
              <span
                key={agent}
                className="inline-flex rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary"
              >
                {agentDisplayName(agent)}
              </span>
            ))}
          </div>
        </div>

        {(group.kind === "skill" ||
          group.kind === "mcp" ||
          group.kind === "hook") &&
          (() => {
            const detectedAgents = sortAgents(
              agents.filter((a) => a.detected),
              agentOrder,
            );
            const AGENTS_WITHOUT_HOOKS = new Set(["antigravity"]);
            const otherAgents = detectedAgents.filter(
              (a) => !group.agents.includes(a.name),
            );
            if (otherAgents.length === 0) return null;
            return (
              <div className="mt-3">
                <h4
                  className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground"
                  title="Copy this extension's configuration to another agent on your machine"
                >
                  Deploy to Agent
                </h4>
                <div className="flex flex-wrap gap-1.5">
                  {otherAgents.map((agent) => {
                    const hookUnsupported = group.kind === "hook" && AGENTS_WITHOUT_HOOKS.has(agent.name);
                    return (
                    <button
                      key={agent.name}
                      disabled={deploying === agent.name || hookUnsupported}
                      title={hookUnsupported ? "Hooks not supported" : undefined}
                      onClick={async () => {
                        if (hookUnsupported) return;
                        setDeploying(agent.name);
                        try {
                          await deployToAgent(
                            group.instances[0].id,
                            agent.name,
                          );
                          const msg = group.kind === "hook"
                            ? `Deployed to ${agentDisplayName(agent.name)}. Takes effect in new sessions.`
                            : `Deployed to ${agentDisplayName(agent.name)}`;
                          toast.success(msg);
                        } catch {
                          toast.error(
                            `Failed to deploy to ${agentDisplayName(agent.name)}`,
                          );
                        } finally {
                          setDeploying(null);
                        }
                      }}
                      className={hookUnsupported
                        ? "flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-muted-foreground/50 cursor-not-allowed"
                        : "flex items-center gap-1.5 rounded-lg border border-border bg-primary/10 px-3 py-1.5 text-xs font-medium text-foreground hover:bg-primary/20 hover:border-ring disabled:opacity-50"
                      }
                    >
                      {deploying === agent.name ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <Download size={12} />
                      )}
                      {agentDisplayName(agent.name)}
                      {hookUnsupported && <span className="text-[10px] opacity-60 ml-0.5">(N/A)</span>}
                    </button>
                    );
                  })}
                </div>
              </div>
            );
          })()}

        {/* 5. Permissions */}
        {group.permissions.length > 0 && (
          <div className="mt-4">
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Permissions
            </h4>
            <div className="space-y-2">
              {group.permissions.map((p, i) => (
                <PermissionDetail key={i} perm={p} />
              ))}
            </div>
          </div>
        )}

        {/* 6. CLI Details */}
        {group.kind === "cli" &&
          group.instances[0]?.cli_meta &&
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
                      <span className="font-mono text-xs truncate">
                        {cli_meta.binary_path}
                      </span>
                    </>
                  )}
                  {cli_meta.credentials_path && (
                    <>
                      <span>Credentials:</span>
                      <span className="font-mono text-xs truncate">
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

        {/* 7. CLI Associated Skills */}
        {group.kind === "cli" &&
          (() => {
            const children = extensions.filter(
              (e) => e.cli_parent_id === group.instances[0]?.id,
            );
            return children.length > 0 ? (
              <div className="mt-4">
                <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">
                  Associated Skills ({children.length})
                </h4>
                <div className="space-y-1">
                  {children.map((child) => (
                    <div
                      key={child.id}
                      className="flex items-center justify-between text-sm py-1"
                    >
                      <span>{child.name}</span>
                      <span
                        className={
                          child.enabled
                            ? "text-trust-safe"
                            : "text-muted-foreground"
                        }
                      >
                        {child.enabled ? "Enabled" : "Disabled"}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            ) : null;
          })()}

        {/* 8. Paths (per-agent breakdown) */}
        {group.instances.length > 0 && (
          <div className="mt-4">
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Paths
            </h4>
            <div className="space-y-3">
              {(() => {
                // Group instances by agent to avoid duplicate path cards
                const byAgent = new Map<string, typeof group.instances>();
                for (const inst of group.instances) {
                  const agent = inst.agents[0] ?? "unknown";
                  const list = byAgent.get(agent) ?? [];
                  list.push(inst);
                  byAgent.set(agent, list);
                }
                return [...byAgent.entries()].map(([agentName, instances]) => {
                  const firstData = instanceData.get(instances[0].id);
                  const agentLocations = skillLocations.filter(([a]) => a === agentName);
                  // Collect unique event names for hooks
                  const hookEvents = group.kind === "hook"
                    ? [...new Set(instances.map((inst) => {
                        const parts = inst.name.split(":");
                        return parts.length >= 1 ? parts[0] : "";
                      }).filter(Boolean))]
                    : [];
                  return (
                    <div
                      key={agentName}
                      className="rounded-lg border border-border bg-card p-3"
                    >
                      <span className="text-sm font-medium">
                        {agentDisplayName(agentName)}
                      </span>
                      <div className="mt-1.5 space-y-1">
                        {agentLocations.length > 0 ? (
                          agentLocations.map(([, path]) => (
                            <div key={path} className="flex items-start gap-2 text-muted-foreground">
                              <FolderOpen size={12} className="mt-0.5 shrink-0" />
                              <span className="break-all text-xs">{path}</span>
                            </div>
                          ))
                        ) : firstData?.path ? (
                          <div className="flex items-start gap-2 text-muted-foreground">
                            <FolderOpen size={12} className="mt-0.5 shrink-0" />
                            <span className="break-all text-xs">{firstData.path}</span>
                          </div>
                        ) : null}
                        {firstData?.symlink_target && (
                          <div className="flex items-start gap-2 text-muted-foreground/70">
                            <Link size={12} className="mt-0.5 shrink-0" />
                            <span className="break-all text-xs italic">
                              {firstData.symlink_target}
                            </span>
                          </div>
                        )}
                        {hookEvents.length > 0 && (
                          <div className="flex items-center gap-2 text-muted-foreground mt-0.5">
                            <span className="text-xs">
                              {hookEvents.length === 1 ? "Event" : "Events"}: {hookEvents.join(", ")}
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
        )}

        {/* 9. Content / Documentation */}
        {group.kind !== "hook" && (
          <div className="mt-4">
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {group.kind === "skill"
                ? "Documentation"
                : group.kind === "mcp"
                  ? "Configuration"
                  : "Details"}
            </h4>
            {/* Agent tabs for switching instance content */}
            {group.instances.length > 1 && (
              <div className="mb-2 flex flex-wrap gap-1">
                {group.instances.map((instance) => (
                  <button
                    key={instance.id}
                    onClick={() => setActiveInstanceId(instance.id)}
                    className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
                      activeInstanceId === instance.id
                        ? "bg-primary/20 text-primary"
                        : "bg-muted text-muted-foreground hover:bg-muted/80"
                    }`}
                  >
                    {agentDisplayName(instance.agents[0] ?? "unknown")}
                  </button>
                ))}
              </div>
            )}

            {/* File tree + SKILL.md content */}
            {activeInstanceId && (
              <SkillFileSection
                instanceId={activeInstanceId}
                content={instanceData.get(activeInstanceId)?.content ?? null}
                dirPath={instanceData.get(activeInstanceId)?.path ?? null}
                loading={loadingContent}
                kind={group.kind}
              />
            )}
          </div>
        )}

        {/* 10. Delete trigger */}
        <div className="mt-4">
          <button
            onClick={() => setShowDelete(true)}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-destructive hover:bg-destructive/10"
          >
            <Trash2 size={12} />
            Delete...
          </button>
        </div>

        {/* Delete confirmation dialog */}
        {showDelete && (
          <DeleteDialog
            group={group}
            instanceData={instanceData}
            deleting={deleting}
            deleteAgents={deleteAgents}
            setDeleteAgents={setDeleteAgents}
            onDelete={async (agents) => {
              setDeleting(true);
              try {
                await deleteFromAgents(group.groupKey, agents);
                toast.success(
                  agents.length === group.agents.length
                    ? "Extension deleted"
                    : `Deleted from ${agents.map(agentDisplayName).join(", ")}`,
                );
                if (agents.length === group.agents.length) setSelectedId(null);
              } catch {
                toast.error("Failed to delete");
              } finally {
                setDeleting(false);
                setShowDelete(false);
              }
            }}
            onClose={() => setShowDelete(false)}
          />
        )}
      </div>
    </div>
  );
}
