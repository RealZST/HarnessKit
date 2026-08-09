import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileTreeNode } from "@/components/extensions/file-tree-node";
import { api } from "@/lib/invoke";
import type { FileEntry } from "@/lib/types";

export function SkillFileSection({
  dirPath,
  loading,
}: {
  dirPath: string | null;
  loading: boolean;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const [fileTree, setFileTree] = useState<FileEntry[] | null>(null);
  // Accordion: at most one file preview open across the whole tree, and at
  // most one root-level directory expanded (nested levels manage their own).
  const [expandedPath, setExpandedPath] = useState<string | null>(null);
  const [expandedDir, setExpandedDir] = useState<string | null>(null);

  useEffect(() => {
    setExpandedPath(null);
    setExpandedDir(null);
    if (!dirPath) {
      setFileTree(null);
      return;
    }
    api
      .listSkillFiles(dirPath)
      .then(setFileTree)
      .catch(() => setFileTree(null));
  }, [dirPath]);

  if (loading) {
    return (
      <p className="text-xs text-muted-foreground">{tc("status.loading")}</p>
    );
  }

  if (!fileTree || fileTree.length === 0) {
    return (
      <p className="text-xs text-muted-foreground italic">
        {t("skillFile.noFiles")}
      </p>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-muted/20 p-2">
      {fileTree.map((entry) => (
        <FileTreeNode
          key={entry.path}
          entry={entry}
          depth={0}
          expandedPath={expandedPath}
          onToggle={(path) =>
            setExpandedPath((current) => (current === path ? null : path))
          }
          dirExpanded={expandedDir === entry.path}
          onToggleDir={(path) =>
            setExpandedDir((current) => (current === path ? null : path))
          }
        />
      ))}
    </div>
  );
}
