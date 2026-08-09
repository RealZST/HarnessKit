import {
  ChevronRight,
  Copy,
  ExternalLink,
  File,
  FolderClosed,
  FolderOpen as FolderOpenIcon,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useScrollPassthrough } from "@/hooks/use-scroll-passthrough";
import { copyPathToClipboard } from "@/lib/copy-path";
import { api } from "@/lib/invoke";
import { isDesktop } from "@/lib/transport";
import type { FileEntry } from "@/lib/types";

const MAX_FILES_PER_DIR = 3;

interface FileAccordion {
  /** Path of the single expanded file in this tree (accordion), if any. */
  expandedPath: string | null;
  onToggle: (path: string) => void;
}

export function FileTreeNode({
  entry,
  depth,
  expandedPath,
  onToggle,
  dirExpanded,
  onToggleDir,
}: FileAccordion & {
  entry: FileEntry;
  depth: number;
  /** Whether this directory node is the one expanded among its siblings. */
  dirExpanded: boolean;
  onToggleDir: (path: string) => void;
}) {
  return entry.is_dir ? (
    <DirNode
      entry={entry}
      depth={depth}
      expandedPath={expandedPath}
      onToggle={onToggle}
      dirExpanded={dirExpanded}
      onToggleDir={onToggleDir}
    />
  ) : (
    <FileNode
      entry={entry}
      depth={depth}
      expandedPath={expandedPath}
      onToggle={onToggle}
    />
  );
}

function DirNode({
  entry,
  depth,
  expandedPath,
  onToggle,
  dirExpanded,
  onToggleDir,
}: FileAccordion & {
  entry: FileEntry;
  depth: number;
  dirExpanded: boolean;
  onToggleDir: (path: string) => void;
}) {
  const { t } = useTranslation("extensions");
  // Per-level accordion for THIS directory's subdirectories: at most one
  // child dir open. Resets automatically when this node unmounts (i.e.
  // when an ancestor collapses).
  const [expandedChildDir, setExpandedChildDir] = useState<string | null>(null);
  const children = entry.children ?? [];
  const truncated = children.length > MAX_FILES_PER_DIR;
  const visibleChildren = truncated
    ? children.slice(0, MAX_FILES_PER_DIR)
    : children;

  return (
    <div>
      <button
        onClick={() => onToggleDir(entry.path)}
        className="flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-foreground hover:bg-muted/60"
        style={{ paddingLeft: `${depth * 16 + 4}px` }}
      >
        <ChevronRight
          size={12}
          className={`shrink-0 text-muted-foreground transition-transform duration-150 ${dirExpanded ? "rotate-90" : ""}`}
        />
        {dirExpanded ? (
          <FolderOpenIcon size={13} className="shrink-0 text-primary/70" />
        ) : (
          <FolderClosed size={13} className="shrink-0 text-primary/70" />
        )}
        <span className="truncate">{entry.name}</span>
      </button>
      {dirExpanded && (
        <div>
          {visibleChildren.map((child) => (
            <FileTreeNode
              key={child.path}
              entry={child}
              depth={depth + 1}
              expandedPath={expandedPath}
              onToggle={onToggle}
              dirExpanded={expandedChildDir === child.path}
              onToggleDir={(path) =>
                setExpandedChildDir((current) =>
                  current === path ? null : path,
                )
              }
            />
          ))}
          {isDesktop() && (
            <button
              onClick={() => api.revealInFileManager(entry.path)}
              className="flex items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-primary hover:bg-muted/60"
              style={{ paddingLeft: `${(depth + 1) * 16 + 4}px` }}
            >
              <ExternalLink size={11} className="shrink-0" />
              <span>
                {truncated
                  ? t("fileTree.moreFiles", {
                      count: children.length - MAX_FILES_PER_DIR,
                    })
                  : t("fileTree.openInFinder")}
              </span>
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function FileNode({
  entry,
  depth,
  expandedPath,
  onToggle,
}: FileAccordion & { entry: FileEntry; depth: number }) {
  // Files expand in place to a read-only preview (accordion: the tree root
  // keeps a single expandedPath). Open/copy actions live inside the
  // expanded block, mirroring the Agents page config rows.
  const isExpanded = expandedPath === entry.path;

  // If this node unmounts while expanded (its parent folder collapsed or
  // was displaced by a sibling), clear the shared accordion so re-expanding
  // the folder shows the file collapsed again.
  const unmountRef = useRef({ isExpanded, path: entry.path, onToggle });
  unmountRef.current = { isExpanded, path: entry.path, onToggle };
  useEffect(
    () => () => {
      const { isExpanded: open, path, onToggle: toggle } = unmountRef.current;
      if (open) toggle(path);
    },
    [],
  );

  return (
    <div>
      <button
        onClick={() => onToggle(entry.path)}
        className={`flex w-full items-center gap-1.5 rounded px-1 py-0.5 text-xs text-muted-foreground hover:text-foreground hover:bg-muted/60 ${isExpanded ? "bg-muted/60 text-foreground" : ""}`}
        style={{ paddingLeft: `${depth * 16 + 20}px` }}
        title={entry.path}
      >
        <File size={12} className="shrink-0" />
        <span className="truncate">{entry.name}</span>
      </button>
      {isExpanded && <FilePreview path={entry.path} />}
    </div>
  );
}

/** Read-only preview + actions for the expanded file. Fetches once per
 *  mount; binary or unreadable files surface as "preview unavailable". */
function FilePreview({ path }: { path: string }) {
  const { t } = useTranslation("extensions");
  const { t: ta } = useTranslation("agents");
  const { t: tc } = useTranslation("common");
  const handleNestedWheel = useScrollPassthrough();
  const [preview, setPreview] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .readConfigFilePreview(path)
      .then((content) => {
        if (!cancelled) setPreview(content);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  const actionButton =
    "inline-flex items-center gap-1.5 rounded-md border border-border bg-background px-2.5 py-1 text-[11px] font-medium transition-colors hover:bg-accent";

  return (
    <div className="my-1 rounded-md border border-border/50 bg-muted/30 px-2.5 py-2">
      {failed ? (
        <div className="mb-2 text-[11px] text-muted-foreground">
          {ta("file.previewUnavailable")}
        </div>
      ) : preview === null ? (
        <div className="mb-2 text-[11px] text-muted-foreground">
          {tc("status.loading")}
        </div>
      ) : (
        <pre
          onWheel={handleNestedWheel}
          className="mb-2 max-h-[200px] overflow-y-auto whitespace-pre-wrap font-mono text-[11px] leading-relaxed text-muted-foreground"
        >
          {preview || ta("file.emptyFile")}
        </pre>
      )}
      <div className="flex gap-2">
        {isDesktop() && (
          <button
            onClick={() => api.openInSystem(path)}
            className={actionButton}
          >
            <ExternalLink size={11} /> {t("fileTree.open")}
          </button>
        )}
        <button
          onClick={() => copyPathToClipboard(path)}
          className={actionButton}
        >
          <Copy size={11} /> {ta("file.copyPath")}
        </button>
      </div>
    </div>
  );
}
