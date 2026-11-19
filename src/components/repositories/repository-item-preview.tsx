import { clsx } from "clsx";
import { Download } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  DetailPanel,
  SkillDocumentation,
} from "@/components/shared/detail-panel";
import { api } from "@/lib/invoke";
import type { RepositoryItem, RepositoryWithItems } from "@/lib/types";
import { RepositoryAddress } from "./repository-address";
import { INSTALL_BUTTON } from "./repository-item-row";

/** One skill of a tracked repository, read from the checkout: what it says
 *  it does, and its SKILL.md. */
export function RepositoryItemPreview({
  repo,
  item,
  onInstall,
  onClose,
}: {
  repo: RepositoryWithItems;
  item: RepositoryItem;
  onInstall: () => void;
  onClose: () => void;
}) {
  const { t: tc } = useTranslation("common");
  // A refresh moves the checkout, so the commit is part of what is shown.
  const shown = `${repo.id}:${item.item_id}:${repo.last_commit_hash}`;
  const [loaded, setLoaded] = useState<{ of: string; text: string | null }>();

  // biome-ignore lint/correctness/useExhaustiveDependencies: `shown` names the skill and the commit to read.
  useEffect(() => {
    let current = true;
    const show = (text: string | null) =>
      current && setLoaded({ of: shown, text });
    api
      .getRepositoryItemContent(repo.id, item.item_id)
      .then(show)
      .catch(() => show(null));
    return () => {
      current = false;
    };
  }, [shown]);

  return (
    <DetailPanel
      itemKey={`${repo.id}:${item.item_id}`}
      onClose={onClose}
      heading={
        <>
          <h3 className="text-lg font-semibold">{item.name}</h3>
          <div className="mt-1 text-xs text-muted-foreground">
            <RepositoryAddress repo={repo} />
          </div>
        </>
      }
    >
      {item.description && (
        <p className="text-sm text-muted-foreground">{item.description}</p>
      )}
      <button onClick={onInstall} className={clsx("mt-4", INSTALL_BUTTON)}>
        <Download size={12} className="text-primary" />
        {tc("actions.install")}
      </button>
      <SkillDocumentation
        content={loaded?.of === shown ? loaded.text : undefined}
      />
    </DetailPanel>
  );
}
