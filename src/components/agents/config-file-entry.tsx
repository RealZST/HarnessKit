import { useEffect, useState } from "react";
import { ChevronRight, FolderOpen, Copy, Trash2, Tag, Pencil, Check, X } from "lucide-react";
import { clsx } from "clsx";
import type { AgentConfigFile, ConfigCategory } from "@/lib/types";
import { CONFIG_CATEGORY_LABELS } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";

const CATEGORY_ORDER: ConfigCategory[] = ["rules", "memory", "settings", "ignore"];

export function ConfigFileEntry({ file }: { file: AgentConfigFile }) {
  const expandedFiles = useAgentConfigStore((s) => s.expandedFiles);
  const toggleFile = useAgentConfigStore((s) => s.toggleFile);
  const fetchPreview = useAgentConfigStore((s) => s.fetchPreview);
  const openInEditor = useAgentConfigStore((s) => s.openInEditor);
  const copyPath = useAgentConfigStore((s) => s.copyPath);
  const updateCustomPath = useAgentConfigStore((s) => s.updateCustomPath);
  const removeCustomPath = useAgentConfigStore((s) => s.removeCustomPath);
  const previewCache = useAgentConfigStore((s) => s.previewCache);

  const isExpanded = expandedFiles.has(file.path);
  const preview = previewCache.get(file.path) ?? null;

  const [editing, setEditing] = useState(false);
  const [editPath, setEditPath] = useState(file.path);
  const [editCategory, setEditCategory] = useState<ConfigCategory>(file.category);

  useEffect(() => {
    if (isExpanded && preview === null && !file.is_dir) {
      fetchPreview(file.path);
    }
  }, [isExpanded, file.path, fetchPreview, preview, file.is_dir]);

  const scopeLabel = file.scope.type === "global" ? "Global" : file.scope.name;
  const scopePath = file.custom_id != null
    ? file.path
    : file.scope.type === "global"
      ? file.path.slice(0, file.path.lastIndexOf(file.file_name))
      : file.scope.path;
  const sizeLabel = file.size_bytes < 1024
    ? `${file.size_bytes} B`
    : `${(file.size_bytes / 1024).toFixed(1)} KB`;

  return (
    <div className="border-b border-border/50 last:border-b-0">
      <button
        onClick={() => toggleFile(file.path)}
        className={clsx(
          "flex w-full items-center justify-between px-4 py-2.5 text-left transition-colors hover:bg-accent/30",
          isExpanded && "bg-accent/20"
        )}
      >
        <div className="flex items-center gap-2 min-w-0">
          <ChevronRight
            size={14}
            className={clsx("shrink-0 text-muted-foreground transition-transform", isExpanded && "rotate-90")}
          />
          <span className="text-[13px] font-medium truncate">{file.file_name}</span>
          {file.custom_id != null && (
            <span className="inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary shrink-0">
              <Tag size={8} /> Custom
            </span>
          )}
          <span className="text-[11px] text-muted-foreground truncate">{scopeLabel} · {scopePath}</span>
        </div>
        {!file.is_dir && (
          <span className="text-[11px] text-muted-foreground shrink-0 ml-2">{sizeLabel}</span>
        )}
      </button>
      {isExpanded && (
        <div className="border-t border-border/30 bg-muted/30 px-4 py-3">
          {!file.is_dir && (
            preview !== null ? (
              <pre className="text-[11px] leading-relaxed text-muted-foreground font-mono whitespace-pre-wrap max-h-[200px] overflow-y-auto mb-3">
                {preview || "(empty file)"}
              </pre>
            ) : (
              <div className="text-[11px] text-muted-foreground mb-3">Loading...</div>
            )
          )}

          {/* Edit form for custom paths */}
          {editing && file.custom_id != null && (
            <div className="mb-3 space-y-2 rounded-md border border-border bg-background p-2.5">
              <input
                type="text"
                value={editPath}
                onChange={(e) => setEditPath(e.target.value)}
                placeholder="Path"
                className="w-full rounded-md border border-border bg-card px-2.5 py-1 text-[12px] focus:outline-none focus:ring-1 focus:ring-ring"
              />
              <div className="flex items-center gap-2">
                <select
                  value={editCategory}
                  onChange={(e) => setEditCategory(e.target.value as ConfigCategory)}
                  className="rounded-md border border-border bg-card px-2 py-1 text-[12px] focus:outline-none focus:ring-1 focus:ring-ring"
                >
                  {CATEGORY_ORDER.map((cat) => (
                    <option key={cat} value={cat}>{CONFIG_CATEGORY_LABELS[cat]}</option>
                  ))}
                </select>
                <div className="flex-1" />
                <button
                  onClick={(e) => { e.stopPropagation(); setEditing(false); }}
                  className="inline-flex items-center gap-1 rounded-md border border-border bg-background px-2 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
                >
                  <X size={11} /> Cancel
                </button>
                <button
                  disabled={!editPath.trim()}
                  onClick={async (e) => {
                    e.stopPropagation();
                    await updateCustomPath(file.custom_id!, editPath.trim(), "", editCategory);
                    setEditing(false);
                  }}
                  className="inline-flex items-center gap-1 rounded-md bg-primary px-2 py-1 text-[11px] font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-40"
                >
                  <Check size={11} /> Save
                </button>
              </div>
            </div>
          )}

          <div className="flex gap-2">
            <button
              onClick={(e) => { e.stopPropagation(); openInEditor(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <FolderOpen size={12} /> Reveal in Finder
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); copyPath(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <Copy size={12} /> Copy Path
            </button>
            {file.custom_id != null && (
              <>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditPath(file.path);
                    setEditCategory(file.category);
                    setEditing(!editing);
                  }}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
                >
                  <Pencil size={12} /> Edit
                </button>
                <button
                  onClick={(e) => { e.stopPropagation(); removeCustomPath(file.custom_id!); }}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium text-destructive transition-colors hover:bg-destructive/10"
                >
                  <Trash2 size={12} /> Remove
                </button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
