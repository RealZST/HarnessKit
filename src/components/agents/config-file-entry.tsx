import { useEffect } from "react";
import { ChevronRight, ExternalLink, Copy } from "lucide-react";
import { clsx } from "clsx";
import type { AgentConfigFile } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";

export function ConfigFileEntry({ file }: { file: AgentConfigFile }) {
  const expandedFiles = useAgentConfigStore((s) => s.expandedFiles);
  const toggleFile = useAgentConfigStore((s) => s.toggleFile);
  const fetchPreview = useAgentConfigStore((s) => s.fetchPreview);
  const openInEditor = useAgentConfigStore((s) => s.openInEditor);
  const copyPath = useAgentConfigStore((s) => s.copyPath);
  const previewCache = useAgentConfigStore((s) => s.previewCache);

  const isExpanded = expandedFiles.has(file.path);
  const preview = previewCache.get(file.path) ?? null;

  useEffect(() => {
    if (isExpanded && preview === null) {
      fetchPreview(file.path);
    }
  }, [isExpanded, file.path, fetchPreview, preview]);

  const scopeLabel = file.scope.type === "global" ? "Global" : file.scope.name;
  const scopePath = file.scope.type === "global"
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
          <span className="text-[11px] text-muted-foreground truncate">{scopeLabel} · {scopePath}</span>
        </div>
        <span className="text-[11px] text-muted-foreground shrink-0 ml-2">{sizeLabel}</span>
      </button>
      {isExpanded && (
        <div className="border-t border-border/30 bg-muted/30 px-4 py-3">
          {preview !== null ? (
            <pre className="text-[11px] leading-relaxed text-muted-foreground font-mono whitespace-pre-wrap max-h-[200px] overflow-y-auto mb-3">
              {preview || "(empty file)"}
            </pre>
          ) : (
            <div className="text-[11px] text-muted-foreground mb-3">Loading...</div>
          )}
          <div className="flex gap-2">
            <button
              onClick={(e) => { e.stopPropagation(); openInEditor(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <ExternalLink size={12} /> Open in Editor
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); copyPath(file.path); }}
              className="inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent"
            >
              <Copy size={12} /> Copy Path
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
