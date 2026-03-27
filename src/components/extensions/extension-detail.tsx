import { useEffect, useState } from "react";
import { useExtensionStore } from "@/stores/extension-store";
import { KindBadge } from "@/components/shared/kind-badge";
import { TrustBadge } from "@/components/shared/trust-badge";
import { api } from "@/lib/invoke";
import { X, File, Globe, Terminal, Database, Key, Calendar, GitBranch, ArrowDownCircle, CheckCircle, FolderOpen, Download, Loader2 } from "lucide-react";
import { tagColor, CATEGORIES } from "@/components/extensions/extension-filters";
import { useAgentStore } from "@/stores/agent-store";
import type { Permission } from "@/lib/types";

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
  const { extensions, selectedId, setSelectedId, toggle, updateStatuses, allTags, updateTags, updateCategory, deployToAgent } = useExtensionStore();
  const ext = extensions.find((e) => e.id === selectedId);
  const [content, setContent] = useState<string | null>(null);
  const [dirPath, setDirPath] = useState<string | null>(null);
  const [loadingContent, setLoadingContent] = useState(false);
  const [tagInput, setTagInput] = useState("");
  const { agents } = useAgentStore();
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
      className="w-96 shrink-0 sticky top-0 self-start max-h-[calc(100vh-3rem)] overflow-y-auto overscroll-contain rounded-xl border border-border bg-card p-5 shadow-sm"
    >
      <div className="flex items-start justify-between">
        <div>
          <h3 className="text-lg font-semibold">{ext.name}</h3>
          <div className="mt-1 flex items-center gap-2">
            <KindBadge kind={ext.kind} />
            {ext.trust_score != null && <TrustBadge score={ext.trust_score} size="sm" />}
          </div>
        </div>
        <button onClick={() => setSelectedId(null)} className="rounded-lg p-1 text-muted-foreground hover:text-foreground">
          <X size={18} />
        </button>
      </div>

      {ext.description && (
        <p className="mt-3 text-sm text-muted-foreground">{ext.description}</p>
      )}

      {/* Metadata */}
      <div className="mt-4 space-y-2 text-sm">
        <div className="flex items-center gap-2 text-muted-foreground">
          <Calendar size={14} />
          <span>Installed {formatDate(ext.installed_at)}</span>
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
                <button onClick={() => updateTags(ext.id, ext.tags.filter((t) => t !== tag))} className="hover:opacity-70">
                  <X size={10} />
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
            <h4 className="mb-2 text-xs font-medium text-muted-foreground">Deploy to Agent</h4>
            <div className="flex flex-wrap gap-1.5">
              {otherAgents.map((agent) => (
                <button
                  key={agent.name}
                  disabled={deploying === agent.name}
                  onClick={async () => {
                    setDeploying(agent.name);
                    try {
                      await deployToAgent(ext.id, agent.name);
                    } finally {
                      setDeploying(null);
                    }
                  }}
                  className="flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs text-foreground hover:border-ring hover:bg-accent disabled:opacity-50"
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
              <span className="flex items-center gap-1 text-xs text-green-600 dark:text-green-400">
                <CheckCircle size={14} /> Up to date
              </span>
            ) : status.status === "update_available" ? (
              <span className="flex items-center gap-1 text-xs text-blue-600 dark:text-blue-400">
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
          onClick={() => toggle(ext.id, !ext.enabled)}
          className={`rounded-full px-3 py-1 text-xs font-medium ${
            ext.enabled
              ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400"
              : "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400"
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
          {ext.kind === "skill" ? "Skill Documentation" : ext.kind === "mcp" ? "Server Configuration" : ext.kind === "hook" ? "Hook Command" : "Details"}
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
  );
}
