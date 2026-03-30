import { useEffect, useRef, useState } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { KindBadge } from "@/components/shared/kind-badge";
import { TrustBadge } from "@/components/shared/trust-badge";
import { api } from "@/lib/invoke";
import { X, File, Globe, Terminal, Database, Key, Calendar, Clock, GitBranch, ArrowDownCircle, CheckCircle, FolderOpen, Download, Loader2, Trash2, Link, AlertTriangle } from "lucide-react";
import type { ExtensionContent as ExtContent } from "@/lib/types";
import { formatRelativeTime, sortAgents, agentDisplayName } from "@/lib/types";
import type { Permission } from "@/lib/types";
import { CATEGORIES } from "@/components/extensions/extension-filters";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

function PermissionDetail({ perm }: { perm: Permission }) {
  const icons: Record<string, typeof File> = { filesystem: File, network: Globe, shell: Terminal, database: Database, env: Key };
  const labels: Record<string, string> = { filesystem: "File System", network: "Network", shell: "Shell", database: "Database", env: "Environment" };
  const Icon = icons[perm.type] ?? File;
  const details = "paths" in perm ? perm.paths : "domains" in perm ? perm.domains : "commands" in perm ? perm.commands : "engines" in perm ? perm.engines : "keys" in perm ? perm.keys : [];

  return (
    <div className="flex items-start gap-2 text-sm">
      <Icon size={14} className="mt-0.5 shrink-0 text-muted-foreground" />
      <div>
        <span className="font-medium text-foreground">{labels[perm.type] ?? perm.type}</span>
        {details.length > 0 && (
          <p className="text-xs text-muted-foreground">{details.join(", ")}</p>
        )}
      </div>
    </div>
  );
}

export function ExtensionDetail() {
  const grouped = useExtensionStore(s => s.grouped);
  const selectedId = useExtensionStore(s => s.selectedId);
  const setSelectedId = useExtensionStore(s => s.setSelectedId);
  const toggle = useExtensionStore(s => s.toggle);
  const updateStatuses = useExtensionStore(s => s.updateStatuses);
  const updateCategory = useExtensionStore(s => s.updateCategory);
  const deployToAgent = useExtensionStore(s => s.deployToAgent);
  const deleteFromAgents = useExtensionStore(s => s.deleteFromAgents);
  const group = grouped().find((g) => g.groupKey === selectedId);
  /** Per-instance content data keyed by instance id */
  const [instanceData, setInstanceData] = useState<Map<string, ExtContent>>(new Map());
  const [loadingContent, setLoadingContent] = useState(false);
  const agents = useAgentStore(s => s.agents);
  const agentOrder = useAgentStore(s => s.agentOrder);
  const [deploying, setDeploying] = useState<string | null>(null);
  const [activeInstanceId, setActiveInstanceId] = useState<string | null>(null);
  const [showDelete, setShowDelete] = useState(false);
  const [deleteAgents, setDeleteAgents] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);

  // Reset state and load ALL instance data when group changes
  useEffect(() => {
    if (group && group.instances.length > 0) {
      setActiveInstanceId(group.instances[0].id);
      // Load content + path for every instance in parallel
      setLoadingContent(true);
      setInstanceData(new Map());
      Promise.all(
        group.instances.map((inst) =>
          api.getExtensionContent(inst.id)
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
    } else {
      setActiveInstanceId(null);
      setInstanceData(new Map());
    }
    setShowDelete(false);
    setDeleteAgents(new Set());
  }, [selectedId]);

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
          <h3 className="text-lg font-semibold">{group.name}</h3>
          <div className="mt-1 flex items-center gap-2">
            <KindBadge kind={group.kind} />
            {group.trust_score != null && <TrustBadge score={group.trust_score} size="sm" />}
          </div>
        </div>
        <button onClick={() => setSelectedId(null)} aria-label="Close extension details" className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground">
          <X size={18} />
        </button>
      </div>

      {/* Scrollable body */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
      {group.description && (
        <p className="text-sm text-muted-foreground">{group.description}</p>
      )}
      {/* Metadata */}
      <div className="mt-4 space-y-2 text-sm">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Calendar size={14} />
          <span>Installed {formatDate(group.installed_at)}</span>
        </div>
        <div className="flex items-center gap-2 text-muted-foreground">
          <Clock size={14} />
          <span>Last used {group.kind === "skill" ? (group.last_used_at ? formatRelativeTime(group.last_used_at) : "Never") : "\u2014"}</span>
        </div>
        {group.source.origin === "git" && group.source.url && (
          <div className="flex items-center gap-2 text-muted-foreground">
            <GitBranch size={14} />
            <span className="truncate">{group.source.url}</span>
          </div>
        )}
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="text-xs">Agents:</span>
          <div className="flex flex-wrap gap-1">
            {group.agents.map((agent) => (
              <span key={agent} className="inline-flex rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
                {agentDisplayName(agent)}
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* Agent Details (per-agent breakdown) */}
      {group.instances.length > 0 && (
        <div className="mt-4">
          <h4 className="mb-2 text-xs font-medium text-muted-foreground">Agent Details</h4>
          <div className="space-y-3">
            {group.instances.map((instance) => {
              const status = updateStatuses.get(instance.id);
              const data = instanceData.get(instance.id);
              return (
                <div key={instance.id} className="rounded-lg border border-border bg-card p-3">
                  <div className="flex items-center justify-between">
                    <span className="text-sm font-medium">{agentDisplayName(instance.agents[0] ?? "unknown")}</span>
                    {instance.source.origin === "git" && status && (
                      <span className="text-xs">
                        {status.status === "up_to_date" ? (
                          <span className="flex items-center gap-1 text-primary">
                            <CheckCircle size={12} /> Up to date
                          </span>
                        ) : status.status === "update_available" ? (
                          <span className="flex items-center gap-1 text-primary">
                            <ArrowDownCircle size={12} /> Update available
                          </span>
                        ) : (
                          <span className="text-muted-foreground" title={status.message}>Check failed</span>
                        )}
                      </span>
                    )}
                  </div>
                  {data?.path && (
                    <div className="mt-1.5 space-y-1">
                      <div className="flex items-start gap-2 text-muted-foreground">
                        <FolderOpen size={12} className="mt-0.5 shrink-0" />
                        <span className="break-all text-xs">{data.path}</span>
                      </div>
                      {data.symlink_target && (
                        <div className="flex items-start gap-2 text-muted-foreground/70">
                          <Link size={12} className="mt-0.5 shrink-0" />
                          <span className="break-all text-xs italic">{data.symlink_target}</span>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Category */}
      <div className="mt-4">
        <h4 className="mb-2 text-xs font-medium text-muted-foreground">Category</h4>
        <select
          value={group.category ?? ""}
          onChange={(e) => updateCategory(group.groupKey, e.target.value || null)}
          aria-label="Extension category"
          className="w-full rounded-lg border border-border bg-card px-2.5 py-1.5 text-xs text-foreground focus:border-ring focus:outline-none"
        >
          <option value="">No category</option>
          {CATEGORIES.map((cat) => (
            <option key={cat} value={cat}>{cat}</option>
          ))}
        </select>
      </div>


      {/* Deploy to other agents (skill, mcp, hook) */}
      {(group.kind === "skill" || group.kind === "mcp" || group.kind === "hook") && (() => {
        const detectedAgents = sortAgents(agents.filter((a) => a.detected), agentOrder);
        const otherAgents = detectedAgents.filter((a) => !group.agents.includes(a.name));
        if (otherAgents.length === 0) return null;
        return (
          <div className="mt-4">
            <h4 className="mb-2 text-xs font-medium text-muted-foreground" title="Copy this extension's configuration to another agent on your machine">Deploy to Agent</h4>
            <div className="flex flex-wrap gap-1.5">
              {otherAgents.map((agent) => (
                <button
                  key={agent.name}
                  disabled={deploying === agent.name}
                  onClick={async () => {
                    setDeploying(agent.name);
                    try {
                      await deployToAgent(group.instances[0].id, agent.name);
                      toast.success(`Deployed to ${agentDisplayName(agent.name)}`);
                    } catch {
                      toast.error(`Failed to deploy to ${agentDisplayName(agent.name)}`);
                    } finally {
                      setDeploying(null);
                    }
                  }}
                  className="flex items-center gap-1.5 rounded-lg border border-border bg-primary/10 px-3 py-1.5 text-xs font-medium text-foreground hover:bg-primary/20 hover:border-ring disabled:opacity-50"
                >
                  {deploying === agent.name ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Download size={12} />
                  )}
                  {agentDisplayName(agent.name)}
                </button>
              ))}
            </div>
          </div>
        );
      })()}

      {/* Update status for git-sourced extensions (group-level summary) */}
      {group.source.origin === "git" && (() => {
        // Show aggregate status across all instances
        const statuses = group.instances
          .map((inst) => updateStatuses.get(inst.id))
          .filter(Boolean);
        const hasUpdate = statuses.some((s) => s!.status === "update_available");
        const allUpToDate = statuses.length > 0 && statuses.every((s) => s!.status === "up_to_date");
        return (
          <div className="mt-4 flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2">
            <span className="text-sm">Updates</span>
            {statuses.length === 0 ? (
              <span className="text-xs text-muted-foreground">Not checked</span>
            ) : hasUpdate ? (
              <span className="flex items-center gap-1 text-xs text-primary">
                <ArrowDownCircle size={14} /> Update available
              </span>
            ) : allUpToDate ? (
              <span className="flex items-center gap-1 text-xs text-primary">
                <CheckCircle size={14} /> Up to date
              </span>
            ) : (
              <span className="text-xs text-muted-foreground">Check failed</span>
            )}
          </div>
        );
      })()}

      {/* Status toggle */}
      <div className="mt-4 flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2">
        <span className="text-sm">Status</span>
        <button
          onClick={() => { toggle(group.groupKey, !group.enabled); toast.success(`Extension ${group.enabled ? "disabled" : "enabled"}`); }}
          aria-pressed={group.enabled}
          className={`rounded-full px-3 py-1 text-xs font-medium ${
            group.enabled
              ? "bg-primary/10 text-primary"
              : "bg-destructive/10 text-destructive"
          }`}
        >
          {group.enabled ? "Enabled" : "Disabled"}
        </button>
      </div>

      {/* Permissions */}
      {group.permissions.length > 0 && (
        <div className="mt-4">
          <h4 className="mb-2 text-xs font-medium text-muted-foreground">Permissions</h4>
          <div className="space-y-2">
            {group.permissions.map((p, i) => (
              <PermissionDetail key={i} perm={p} />
            ))}
          </div>
        </div>
      )}

      {/* Content / Documentation */}
      <div className="mt-4">
        <h4 className="mb-2 text-xs font-medium text-muted-foreground">
          {group.kind === "skill" ? "Documentation" : group.kind === "mcp" ? "Configuration" : group.kind === "hook" ? "Command" : "Details"}
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
        {activeInstanceId && <SkillFileSection
          instanceId={activeInstanceId}
          content={instanceData.get(activeInstanceId)?.content ?? null}
          dirPath={instanceData.get(activeInstanceId)?.path ?? null}
          loading={loadingContent}
          kind={group.kind}
        />}
      </div>

      {/* Delete trigger */}
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
      {showDelete && <DeleteDialog
        group={group}
        instanceData={instanceData}
        deleting={deleting}
        deleteAgents={deleteAgents}
        setDeleteAgents={setDeleteAgents}
        onDelete={async (agents) => {
          setDeleting(true);
          try {
            await deleteFromAgents(group.groupKey, agents);
            toast.success(agents.length === group.agents.length
              ? "Extension deleted"
              : `Deleted from ${agents.map(agentDisplayName).join(", ")}`);
            if (agents.length === group.agents.length) setSelectedId(null);
          } catch {
            toast.error("Failed to delete");
          } finally {
            setDeleting(false);
            setShowDelete(false);
          }
        }}
        onClose={() => setShowDelete(false)}
      />}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Skill file section: file tree + SKILL.md content
// ---------------------------------------------------------------------------

import type { FileEntry, ExtensionKind } from "@/lib/types";
import { ChevronRight, FolderClosed, FolderOpen as FolderOpenIcon, ExternalLink } from "lucide-react";

const MAX_FILES_PER_DIR = 3;

function SkillFileSection({
  dirPath,
  loading,
}: {
  instanceId: string;
  content: string | null;
  dirPath: string | null;
  loading: boolean;
  kind: ExtensionKind;
}) {
  const [fileTree, setFileTree] = useState<FileEntry[] | null>(null);

  useEffect(() => {
    if (!dirPath) { setFileTree(null); return; }
    api.listSkillFiles(dirPath).then(setFileTree).catch(() => setFileTree(null));
  }, [dirPath]);

  if (loading) {
    return <p className="text-xs text-muted-foreground">Loading...</p>;
  }

  if (!fileTree || fileTree.length === 0) {
    return <p className="text-xs text-muted-foreground italic">No files found</p>;
  }

  return (
    <div className="rounded-lg border border-border bg-muted/20 p-2">
      {fileTree.map((entry) => (
        <FileTreeNode key={entry.path} entry={entry} depth={0} />
      ))}
    </div>
  );
}

function FileTreeNode({ entry, depth }: { entry: FileEntry; depth: number }) {
  const [expanded, setExpanded] = useState(false);
  const children = entry.children ?? [];
  const truncated = children.length > MAX_FILES_PER_DIR;
  const visibleChildren = truncated ? children.slice(0, MAX_FILES_PER_DIR) : children;

  if (entry.is_dir) {
    return (
      <div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-foreground hover:bg-muted/60"
          style={{ paddingLeft: `${depth * 16 + 4}px` }}
        >
          <ChevronRight
            size={12}
            className={`shrink-0 text-muted-foreground transition-transform duration-150 ${expanded ? "rotate-90" : ""}`}
          />
          {expanded
            ? <FolderOpenIcon size={13} className="shrink-0 text-primary/70" />
            : <FolderClosed size={13} className="shrink-0 text-primary/70" />
          }
          <span className="truncate">{entry.name}</span>
        </button>
        {expanded && (
          <div>
            {visibleChildren.map((child) => (
              <FileTreeNode key={child.path} entry={child} depth={depth + 1} />
            ))}
            {truncated && (
              <button
                onClick={() => api.openInSystem(entry.path)}
                className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-primary hover:bg-muted/60"
                style={{ paddingLeft: `${(depth + 1) * 16 + 4}px` }}
              >
                <ExternalLink size={11} className="shrink-0" />
                <span>{children.length - MAX_FILES_PER_DIR} more — Open in Finder</span>
              </button>
            )}
          </div>
        )}
      </div>
    );
  }

  return (
    <button
      onClick={() => api.openInSystem(entry.path)}
      className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/60"
      style={{ paddingLeft: `${depth * 16 + 20}px` }}
      title={entry.path}
    >
      <File size={12} className="shrink-0" />
      <span className="truncate">{entry.name}</span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Delete confirmation dialog
// ---------------------------------------------------------------------------

import type { GroupedExtension } from "@/lib/types";

function DeleteDialog({
  group,
  instanceData,
  deleting,
  deleteAgents,
  setDeleteAgents,
  onDelete,
  onClose,
}: {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  deleting: boolean;
  deleteAgents: Set<string>;
  setDeleteAgents: (s: Set<string>) => void;
  onDelete: (agents: string[]) => void;
  onClose: () => void;
}) {
  const dlgRef = useRef<HTMLDivElement>(null);

  // Categorize instances
  const ownInstances: typeof group.instances = [];
  const sharedAgents: string[] = [];
  for (const inst of group.instances) {
    const data = instanceData.get(inst.id);
    if (data?.path && data.path.includes("/.agents/skills")) {
      sharedAgents.push(...inst.agents);
    } else {
      ownInstances.push(inst);
    }
  }
  const hasShared = sharedAgents.length > 0;
  const hasOwn = ownInstances.length > 0;

  // Escape to close
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  // Reset selection when dialog opens
  useEffect(() => { setDeleteAgents(new Set()); }, []);

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      {/* Backdrop — contained within the detail panel */}
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      {/* Dialog */}
      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label="Delete extension"
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in"
      >
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <Trash2 size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-foreground">Delete "{group.name}"</h3>
            <p className="text-xs text-muted-foreground">This action cannot be undone.</p>
          </div>
        </div>

        <div className="space-y-3">
          {/* Own-directory instances: per-agent deletion */}
          {hasOwn && (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                {ownInstances.length === 1 ? "This will permanently delete the skill file:" : "Select agents to permanently delete from:"}
              </p>
              <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
                {ownInstances.map((inst) => {
                  const agent = inst.agents[0];
                  const data = instanceData.get(inst.id);
                  const sym = data?.symlink_target;
                  const isSingle = ownInstances.length === 1;
                  return (
                    <label key={inst.id} className="flex items-start gap-2 text-xs cursor-pointer">
                      {!isSingle && (
                        <input
                          type="checkbox"
                          checked={deleteAgents.has(agent)}
                          onChange={() => {
                            const next = new Set(deleteAgents);
                            if (next.has(agent)) next.delete(agent);
                            else next.add(agent);
                            setDeleteAgents(next);
                          }}
                          className="mt-0.5 rounded border-border accent-destructive"
                        />
                      )}
                      <div className="min-w-0">
                        <span className="font-medium text-foreground">{agentDisplayName(agent)}</span>
                        {data?.path && <p className="text-muted-foreground truncate">{data.path}</p>}
                        {sym && (
                          <p className="flex items-center gap-1 text-chart-5">
                            <Link size={10} className="shrink-0" />
                            <span className="truncate">{sym}</span>
                          </p>
                        )}
                      </div>
                    </label>
                  );
                })}
              </div>
              {ownInstances.length === 1 ? (
                <button
                  disabled={deleting}
                  onClick={() => onDelete(ownInstances[0].agents)}
                  className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
                >
                  {deleting ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
                  Delete from {agentDisplayName(ownInstances[0].agents[0])}
                </button>
              ) : (
                <button
                  disabled={deleting || deleteAgents.size === 0}
                  onClick={() => onDelete(Array.from(deleteAgents))}
                  className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
                >
                  {deleting ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
                  Delete selected ({deleteAgents.size})
                </button>
              )}
            </div>
          )}

          {/* Separator */}
          {hasOwn && hasShared && <hr className="border-border" />}

          {/* Shared directory: all-or-nothing */}
          {hasShared && (
            <div className="space-y-2">
              <div className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                <span>
                  This skill is in the shared directory <span className="font-mono">~/.agents/skills/</span>.
                  Deleting it will remove access for {sharedAgents.map(agentDisplayName).join(", ")}.
                </span>
              </div>
              <button
                disabled={deleting}
                onClick={() => onDelete(sharedAgents)}
                className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
              >
                {deleting ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
                Delete from shared directory
              </button>
            </div>
          )}
        </div>

        {/* Cancel */}
        <button
          onClick={onClose}
          disabled={deleting}
          className="mt-4 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
