import { FolderGit2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { humanizeError } from "@/lib/errors";
import type { GroupedExtension, RepositoryWithItems } from "@/lib/types";
import { useRepositoryStore } from "@/stores/repository-store";
import { toast } from "@/stores/toast-store";

/** In the detail panel of an installed skill: its repository, when it was
 *  installed from Git or the Marketplace. */
export function RepositoryLink({ group }: { group: GroupedExtension }) {
  const ids = new Set(group.instances.map((inst) => inst.id));
  // A Marketplace install records `owner/repo`, which `add` accepts as a
  // GitHub repository.
  const source = group.instances.find((inst) =>
    ["git", "marketplace"].includes(inst.install_meta?.install_type ?? ""),
  )?.install_meta;
  return (
    <TrackOrView
      isSource={(repo) => repo.extension_ids.some((id) => ids.has(id))}
      sourceUrl={source?.url_resolved ?? source?.url ?? undefined}
    />
  );
}

/** In the detail panel of a skill in Discover, whose `source` is
 *  `owner/repo` on GitHub. */
export function MarketplaceRepositoryLink({ source }: { source: string }) {
  const key = `github.com/${source.toLowerCase()}`;
  return (
    <TrackOrView isSource={(repo) => repo.key === key} sourceUrl={source} />
  );
}

/** Opens the tracked repository `isSource` picks, or offers to track
 *  `sourceUrl`. Nothing shows until the tracked repositories are known. */
function TrackOrView({
  isSource,
  sourceUrl,
}: {
  isSource: (repo: RepositoryWithItems) => boolean;
  sourceUrl: string | undefined;
}) {
  const { t } = useTranslation("repositories");
  const navigate = useNavigate();
  const repositories = useRepositoryStore((s) => s.repositories);
  const loaded = useRepositoryStore((s) => s.loaded);
  const [tracking, setTracking] = useState(false);
  const fetch = useRepositoryStore((s) => s.fetch);
  const add = useRepositoryStore((s) => s.add);

  // Mounted per skill (keyed by the caller): an install or a new repository
  // changes the answer.
  useEffect(() => {
    fetch().catch(() => {});
  }, [fetch]);

  const repo = repositories.find(isSource);
  if (!loaded || (!repo && !sourceUrl)) return null;

  // Once added, the store lists the repository and the button becomes the
  // link to it.
  const track = (url: string) => {
    if (tracking) return;
    setTracking(true);
    add(url)
      .catch((e) => toast.error(humanizeError(e)))
      .finally(() => setTracking(false));
  };

  return (
    <button
      type="button"
      onClick={() =>
        repo
          ? navigate(`/marketplace?view=repositories&repo=${repo.id}`)
          : sourceUrl && track(sourceUrl)
      }
      aria-disabled={tracking}
      className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-muted-foreground shadow-sm transition-colors hover:bg-accent hover:text-foreground aria-disabled:pointer-events-none aria-disabled:opacity-50"
    >
      <FolderGit2 size={13} />
      {repo
        ? t("viewRepository")
        : tracking
          ? t("adding")
          : t("trackRepository")}
    </button>
  );
}
