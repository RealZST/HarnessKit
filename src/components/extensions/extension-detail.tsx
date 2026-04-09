import {
  Calendar,
  Download,
  FolderOpen,
  GitBranch,
  Globe,
  Loader2,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { CliSections } from "@/components/extensions/detail-cli-sections";
import { DetailPaths } from "@/components/extensions/detail-paths";
import { DeleteDialog } from "@/components/extensions/delete-dialog";
import { DetailHeader } from "@/components/extensions/detail-header";
import { PermissionDetail } from "@/components/extensions/permission-detail";
import { SkillFileSection } from "@/components/extensions/skill-file-section";
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
  const grouped = useExtensionStore((s) => s.grouped);
  const selectedId = useExtensionStore((s) => s.selectedId);
  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const toggle = useExtensionStore((s) => s.toggle);
  const updateStatuses = useExtensionStore((s) => s.updateStatuses);
  const updateExtension = useExtensionStore((s) => s.updateExtension);
  const updatePack = useExtensionStore((s) => s.updatePack);
  const installToAgent = useExtensionStore((s) => s.installToAgent);
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
  const [activeInstanceId, setActiveInstanceId] = useState<string | null>(null);
  const [showDelete, setShowDelete] = useState(false);
  const [deleteAgents, setDeleteAgents] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);
  // All physical paths where this skill exists, keyed by agent name
  const [skillLocations, setSkillLocations] = useState<[string, string, string | null][]>([]);

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
      // For CLIs, also load content for child MCP extensions (to get config paths)
      if (group.kind === "cli") {
        const childMcps = extensions.filter(
          (e) => e.cli_parent_id === group.instances[0]?.id && e.kind === "mcp",
        );
        if (childMcps.length > 0) {
          Promise.all(
            childMcps.map((m) =>
              api.getExtensionContent(m.id)
                .then((res) => [m.id, res] as const)
                .catch(() => [m.id, null] as const),
            ),
          ).then((results) => {
            setInstanceData((prev) => {
              const next = new Map(prev);
              for (const [id, data] of results) {
                if (data) next.set(id, data);
              }
              return next;
            });
          });
        }
      }
      // Load skill locations for skills and CLIs (CLIs need child skill paths)
      if (group.kind === "skill" || group.kind === "cli") {
        const skillName = group.kind === "cli"
          ? group.instances[0]?.cli_meta?.binary_name ?? group.name
          : group.name;
        api
          .getSkillLocations(skillName)
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
      <DetailHeader
        group={group}
        updateStatuses={updateStatuses}
        updateExtension={updateExtension}
        onClose={() => setSelectedId(null)}
      />

      {/* Scrollable body */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
        {group.description && (
          <p className="text-sm text-muted-foreground">{group.description}</p>
        )}

        {/* 1. Status + Source row */}
        <div className="mt-4 flex items-center gap-2">
          <button
            onClick={() => {
              toggle(group.groupKey, !group.enabled);
              const action = group.enabled ? "disabled" : "enabled";
              toast.success(`Extension ${action}. Takes effect in new sessions`);
            }}
            aria-pressed={group.enabled}
            className={`shrink-0 rounded-full px-3 py-1 text-xs font-medium ${
              group.enabled
                ? "bg-primary/10 text-primary"
                : "bg-muted text-muted-foreground"
            }`}
          >
            {group.enabled ? "Enabled" : "Disabled"}
          </button>
          {group.source.origin === "git" && group.pack ? (
            <a
              href={`https://github.com/${group.pack}`}
              target="_blank"
              rel="noopener noreferrer"
              className="min-w-0 flex-1 truncate rounded-full bg-muted/50 px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
              title={`https://github.com/${group.pack}`}
            >
              {group.pack}
            </a>
          ) : (
            <input
              type="text"
              placeholder="No source"
              defaultValue={group.pack ?? ""}
              key={group.groupKey}
              onBlur={(e) => {
                const val = e.target.value.trim() || null;
                if (val !== group.pack) updatePack(group.groupKey, val);
              }}
              className="min-w-0 flex-1 rounded-full border border-border bg-card px-2.5 py-1 text-xs text-muted-foreground focus:border-ring focus:outline-none"
            />
          )}
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
            // For CLIs, also check child extensions' install_meta for source URL
            const childMeta = !meta && group.kind === "cli"
              ? extensions.find(
                  (e) => e.cli_parent_id === group.instances[0]?.id && e.install_meta?.url,
                )?.install_meta
              : null;
            const sourceUrl =
              meta?.url_resolved ?? meta?.url ?? childMeta?.url_resolved ?? childMeta?.url ?? group.source.url;
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
              {group.kind === "skill" || group.kind === "plugin" || group.kind === "cli"
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
          group.kind === "hook" ||
          group.kind === "cli") &&
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
                  Install to Agent
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
                          await installToAgent(
                            group.instances[0].id,
                            agent.name,
                          );
                          const msg = `Installed to ${agentDisplayName(agent.name)}. Takes effect in new sessions`;
                          toast.success(msg);
                        } catch {
                          toast.error(
                            `Failed to install to ${agentDisplayName(agent.name)}`,
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

        {/* 6+7. CLI Details + Associated Extensions */}
        <CliSections
          group={group}
          extensions={extensions}
          instanceData={instanceData}
          skillLocations={skillLocations}
        />

        {/* 8. Paths (per-agent breakdown) — skip for CLI */}
        <DetailPaths
          group={group}
          instanceData={instanceData}
          skillLocations={skillLocations}
          agentOrder={agentOrder}
        />

        {/* 9. Content / Documentation — skip for hooks and CLIs */}
        {group.kind !== "hook" && group.kind !== "cli" && group.kind !== "mcp" && (
          <div className="mt-4">
            <div className="mb-2 flex items-center justify-between">
              <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Documentation
              </h4>
              {activeInstanceId && instanceData.get(activeInstanceId)?.path && (
                <button
                  onClick={() => api.revealInFileManager(instanceData.get(activeInstanceId)!.path!)}
                  className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                >
                  <FolderOpen size={12} />
                  Open in Finder
                </button>
              )}
            </div>
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
            childExtensions={group.kind === "cli" ? extensions.filter((e) => e.cli_parent_id === group.instances[0]?.id) : undefined}
            skillLocations={group.kind === "cli" || group.kind === "skill" ? skillLocations : undefined}
            onDelete={async (agents) => {
              setDeleting(true);
              try {
                await deleteFromAgents(group.groupKey, agents);
                toast.success(
                  agents.length === group.agents.length
                    ? "Extension deleted. Takes effect in new sessions"
                    : `Deleted from ${agents.map(agentDisplayName).join(", ")}. Takes effect in new sessions`,
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
