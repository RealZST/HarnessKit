import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileTreeNode } from "@/components/extensions/file-tree-node";
import { api } from "@/lib/invoke";
import type { ExtensionKind, FileEntry } from "@/lib/types";

export function SkillFileSection({
  dirPath,
  loading,
}: {
  instanceId: string;
  content: string | null;
  dirPath: string | null;
  loading: boolean;
  kind: ExtensionKind;
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const [fileTree, setFileTree] = useState<FileEntry[] | null>(null);

  useEffect(() => {
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
        <FileTreeNode key={entry.path} entry={entry} depth={0} />
      ))}
    </div>
  );
}
