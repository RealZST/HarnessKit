import {
  ChevronRight,
  ExternalLink,
  File,
  FolderClosed,
  FolderOpen as FolderOpenIcon,
} from "lucide-react";
import { useState } from "react";
import { api } from "@/lib/invoke";
import { isDesktop } from "@/lib/transport";
import type { FileEntry } from "@/lib/types";

const MAX_FILES_PER_DIR = 3;

export function FileTreeNode({
  entry,
  depth,
}: {
  entry: FileEntry;
  depth: number;
}) {
  const [expanded, setExpanded] = useState(false);
  const children = entry.children ?? [];
  const truncated = children.length > MAX_FILES_PER_DIR;
  const visibleChildren = truncated
    ? children.slice(0, MAX_FILES_PER_DIR)
    : children;

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
          {expanded ? (
            <FolderOpenIcon size={13} className="shrink-0 text-primary/70" />
          ) : (
            <FolderClosed size={13} className="shrink-0 text-primary/70" />
          )}
          <span className="truncate">{entry.name}</span>
        </button>
        {expanded && (
          <div>
            {visibleChildren.map((child) => (
              <FileTreeNode key={child.path} entry={child} depth={depth + 1} />
            ))}
            {isDesktop() && (truncated ? (
              <button
                onClick={() => api.openInSystem(entry.path)}
                className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-primary hover:bg-muted/60"
                style={{ paddingLeft: `${(depth + 1) * 16 + 4}px` }}
              >
                <ExternalLink size={11} className="shrink-0" />
                <span>
                  {children.length - MAX_FILES_PER_DIR} more — Open in Finder
                </span>
              </button>
            ) : (
              <button
                onClick={() => api.openInSystem(entry.path)}
                className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-primary hover:bg-muted/60"
                style={{ paddingLeft: `${(depth + 1) * 16 + 4}px` }}
              >
                <ExternalLink size={11} className="shrink-0" />
                <span>Open in Finder</span>
              </button>
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <button
      onClick={() => isDesktop() && api.openInSystem(entry.path)}
      className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/60"
      style={{ paddingLeft: `${depth * 16 + 20}px` }}
      title={entry.path}
    >
      <File size={12} className="shrink-0" />
      <span className="truncate">{entry.name}</span>
    </button>
  );
}
