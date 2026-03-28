import { useEffect, useState } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { KindBadge } from "@/components/shared/kind-badge";
import { TrustBadge } from "@/components/shared/trust-badge";
import { api } from "@/lib/invoke";
import { X, File, Globe, Terminal, Database, Key, Calendar, Clock, GitBranch, ArrowDownCircle, CheckCircle, FolderOpen, Download, Loader2 } from "lucide-react";
import { formatRelativeTime } from "@/lib/types";
import { tagColor, CATEGORIES } from "@/components/extensions/extension-filters";
import { useAgentStore } from "@/stores/agent-store";
import type { Permission } from "@/lib/types";
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
  const extensions = useExtensionStore(s => s.extensions);
  const selectedId = useExtensionStore(s => s.selectedId);
  const setSelectedId = useExtensionStore(s => s.setSelectedId);
  const toggle = useExtensionStore(s => s.toggle);
  const updateStatuses = useExtensionStore(s => s.updateStatuses);
  const allTags = useExtensionStore(s => s.allTags);
  const updateTags = useExtensionStore(s => s.updateTags);
  const updateCategory = useExtensionStore(s => s.updateCategory);
  const deployToAgent = useExtensionStore(s => s.deployToAgent);
  const ext = extensions.find((e) => e.id === selectedId);
  const [content, setContent] = useState<string | null>(null);
  const [dirPath, setDirPath] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);
  const [tagInput, setTagInput] = useState("");
  const agents = useAgentStore(s => s.agents);
  const [deploying, setDeploying] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedId) return;
    setContent(null);
    setDirPath(null);
    setLoadingContent(true);
    api.getExtensionContent(selectedId)
      .then((res) => { setContent(res.content); setDirPath(res.path); })
      .catch(() => { setContent(null); setDirPath(null); })
      .finally(() => setLoadingContent(false));
  }, [selectedId]);

  if (!ext) return null;

  return (
    <div
      onWheel={(e) => e.stopPropagation()}
      className="flex h-full flex-col rounded-xl border border-border bg-card shadow-sm"
    >
      {/* Fixed header */}
      <div className="shrink-0 flex items-start justify-between border-b border-border px-5 py-4">
        <div>
          <h3 className="text-lg font-semibold">{ext.name}</h3>
          <div className="mt-1 flex items-center gap-2">
            <KindBadge kind={ext.kind} />
            {ext.trust_score != null && <TrustBadge score={ext.trust_score} size="sm" />}
          </div>
        </div>
        <button onClick={() => setSelectedId(null)} aria-label="Close extension details" className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground">
          <X size={18} />
        </button>
      </div>

      {/* Scrollable body */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
      {ext.description && (
        <p className="text-sm text-muted-foreground">{ext.description}</p>
      )}
      {/* Metadata */}
      <div className="mt-4 space-y-2 text-sm">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Calendar size={14} />
          <span>Installed {formatDate(ext.installed_at)}</span>
        </div>
        <div className="flex items-center gap-2 text-muted-foreground">
          <Clock size={14} />
          <span>Last used {ext.kind === "skill" ? (ext.last_used_at ? formatRelativeTime(ext.last_used_at) : "Never") : "—"}</span>
        </div>
        {ext.source.origin === "git" && ext.source.url && (
          <div className="flex items-center gap-2 text-muted-foreground">
            <GitBranch size={14} />
            <span className="truncate">{ext.source.url}</span>
          </div>
        )}
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="text-xs">Agents:</span>
          <span>{ext.agents.join(", ")}</span>
        </div>
        {dirPath && (
          <div className="flex items-start gap-2 text-muted-foreground">
            <FolderOpen size={14} className="mt-0.5 shrink-0" />
            <span className="break-all text-xs">{dirPath}</span>
          </div>
        )}
      </div>

      {/* Category */}
      <div className="mt-4">
        <h4 className="mb-2 text-xs font-medium text-muted-foreground">Category</h4>
        <select
          value={ext.category ?? ""}
          onChange={(e) => updateCategory(ext.id, e.target.value || null)}
          aria-label="Extension category"
          className="w-full rounded-lg border border-border bg-card px-2.5 py-1.5 text-xs text-foreground focus:border-ring focus:outline-none"
        >
          <option value="">No category</option>
          {CATEGORIES.map((cat) => (
            <option key={cat} value={cat}>{cat}</option>
          ))}
        </select>
      </div>

      {/* Tags */}
      <div className="mt-4">
        <h4 className="mb-2 text-xs font-medium text-muted-foreground">Tags</h4>
        <div className="flex flex-wrap gap-1.5">
          {ext.tags.map((tag) => {
            const idx = allTags.indexOf(tag);
            return (
              <span key={tag} className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium ${tagColor(idx >= 0 ? idx : 0)}`}>
                {tag}
                <button onClick={() => updateTags(ext.id, ext.tags.filter((t) => t !== tag))} className="p-1.5 hover:opacity-70">
                  <X size={14} />
                </button>
              </span>
            );
          })}
        </div>
        <div className="mt-2 flex gap-1.5">
          <input
            type="text"
            value={tagInput}
            onChange={(e) => setTagInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && tagInput.trim()) {
                const tag = tagInput.trim().toLowerCase();
                if (!ext.tags.includes(tag)) {
                  updateTags(ext.id, [...ext.tags, tag]);
                }
                setTagInput("");
              }
            }}
            list="tag-suggestions"
            placeholder="Add tag..."
            aria-label="Add tag"
            className="flex-1 rounded-lg border border-border bg-card px-2.5 py-1 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
          <datalist id="tag-suggestions">
            {allTags.filter((t) => !ext.tags.includes(t)).map((t) => (
              <option key={t} value={t} />
            ))}
          </datalist>
        </div>
      </div>

      {/* Deploy to other agents (skill, mcp, hook) */}
      {(ext.kind === "skill" || ext.kind === "mcp" || ext.kind === "hook") && (() => {
        const detectedAgents = agents.filter((a) => a.detected);
        const otherAgents = detectedAgents.filter((a) => !ext.agents.includes(a.name));
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
                      await deployToAgent(ext.id, agent.name);
                      toast.success(`Deployed to ${agent.name}`);
                    } catch {
                      toast.error(`Failed to deploy to ${agent.name}`);
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
                  {agent.name}
                </button>
              ))}
            </div>
          </div>
        );
      })()}

      {/* Update status for git-sourced extensions */}
      {ext.source.origin === "git" && (() => {
        const status = updateStatuses.get(ext.id);
        return (
          <div className="mt-4 flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2">
            <span className="text-sm">Updates</span>
            {!status ? (
              <span className="text-xs text-muted-foreground">Not checked</span>
            ) : status.status === "up_to_date" ? (
              <span className="flex items-center gap-1 text-xs text-primary">
                <CheckCircle size={14} /> Up to date
              </span>
            ) : status.status === "update_available" ? (
              <span className="flex items-center gap-1 text-xs text-primary">
                <ArrowDownCircle size={14} /> Update available
              </span>
            ) : (
              <span className="text-xs text-muted-foreground" title={status.message}>Check failed</span>
            )}
          </div>
        );
      })()}

      {/* Status toggle */}
      <div className="mt-4 flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2">
        <span className="text-sm">Status</span>
        <button
          onClick={() => { toggle(ext.id, !ext.enabled); toast.success(`Extension ${ext.enabled ? "disabled" : "enabled"}`); }}
          aria-pressed={ext.enabled}
          className={`rounded-full px-3 py-1 text-xs font-medium ${
            ext.enabled
              ? "bg-primary/10 text-primary"
              : "bg-destructive/10 text-destructive"
          }`}
        >
          {ext.enabled ? "Enabled" : "Disabled"}
        </button>
      </div>

      {/* Permissions */}
      {ext.permissions.length > 0 && (
        <div className="mt-4">
          <h4 className="mb-2 text-xs font-medium text-muted-foreground">Permissions</h4>
          <div className="space-y-2">
            {ext.permissions.map((p, i) => (
              <PermissionDetail key={i} perm={p} />
            ))}
          </div>
        </div>
      )}

      {/* Content / Documentation */}
      <div className="mt-4">
        <h4 className="mb-2 text-xs font-medium text-muted-foreground">
          {ext.kind === "skill" ? "Documentation" : ext.kind === "mcp" ? "Configuration" : ext.kind === "hook" ? "Command" : "Details"}
        </h4>
        <div className="rounded-lg border border-border bg-card p-3">
          {loadingContent ? (
            <p className="text-xs text-muted-foreground">Loading...</p>
          ) : content ? (
            <pre className="whitespace-pre-wrap text-xs text-muted-foreground max-h-80 overflow-y-auto">{content}</pre>
          ) : (
            <p className="text-xs text-muted-foreground italic">No content available</p>
          )}
        </div>
      </div>
      </div>
    </div>
  );
}
