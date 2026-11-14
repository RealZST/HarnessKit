import { FolderOpen, GitCommitHorizontal, Link } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  ExtensionContent as ExtContent,
  Extension,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName, instanceDir, instanceVersion } from "@/lib/types";
import { hookEventName } from "./extension-name";

interface DetailPathsProps {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  skillLocations: [string, string, string | null][];
  agentOrder: readonly string[];
}

export function DetailPaths({
  group,
  instanceData,
  skillLocations,
  agentOrder,
}: DetailPathsProps) {
  const { t } = useTranslation("extensions");
  if (group.kind === "cli" || group.instances.length === 0) return null;

  // skillLocations is scope-agnostic on purpose (the get_skill_locations
  // API surfaces every place a skill named X exists, used by other UIs).
  // We render one card per instance; each card resolves its physical
  // path(s) by matching skillLocations against this instance's source_path
  // dir, falling back to instanceData for instances without on-disk scan
  // results.
  const hasAnyVersion = group.instances.some(
    (i) => instanceVersion(i) !== null,
  );

  const agentRank = new Map(agentOrder.map((a, i) => [a, i] as const));
  const rankOf = (inst: Extension) =>
    agentRank.get(inst.agents[0] ?? "unknown") ?? Number.MAX_SAFE_INTEGER;
  const sortedInstances = [...group.instances].sort(
    (a, b) => rankOf(a) - rankOf(b),
  );
  return (
    <div className="mt-4">
      <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        {t("detailPaths.paths")}
      </h4>
      <div className="space-y-3">
        {sortedInstances.map((inst) => {
          const agent = inst.agents[0] ?? "unknown";
          const data = instanceData.get(inst.id);
          const dir = instanceDir(inst);
          const locations = dir
            ? skillLocations.filter(([a, d]) => a === agent && d === dir)
            : [];
          const version = instanceVersion(inst);
          // Hooks: surface this instance's event matcher (first colon-separated
          // segment of inst.name; the wire format is `event:matcher:command`).
          const hookEvent =
            group.kind === "hook" ? hookEventName(inst.name) : null;
          // Normalize path display into a single list: prefer scan-discovered
          // locations (each carries its own symlink), fall back to
          // instanceData when the scanner found nothing for this instance.
          const paths: { path: string; symlink: string | null }[] =
            locations.length > 0
              ? locations.map(([, p, s]) => ({
                  path: p,
                  symlink: s ?? data?.symlink_target ?? null,
                }))
              : data?.path
                ? [{ path: data.path, symlink: data?.symlink_target ?? null }]
                : [];
          return (
            <div
              key={inst.id}
              className="rounded-lg border border-border bg-card p-3"
            >
              <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
                <span className="font-medium">{agentDisplayName(agent)}</span>
                {hasAnyVersion &&
                  (version ? (
                    <span
                      className="inline-flex items-center gap-1 rounded bg-muted/70 px-1.5 py-0.5 font-mono text-[11px] text-muted-foreground"
                      title={t("detailPaths.revisionTooltip")}
                    >
                      <GitCommitHorizontal size={11} className="opacity-70" />
                      {version}
                    </span>
                  ) : (
                    <span
                      className="text-xs text-muted-foreground/50"
                      title={t("detailPaths.versionUnknownTooltip")}
                    >
                      —
                    </span>
                  ))}
              </div>
              <div
                className={`mt-1.5 space-y-1 ${!group.enabled ? "opacity-50" : ""}`}
              >
                {paths.map(({ path, symlink }) => (
                  <div key={path}>
                    <div className="flex items-start gap-2 text-muted-foreground">
                      <FolderOpen size={12} className="mt-0.5 shrink-0" />
                      <span className="break-all text-xs">{path}</span>
                    </div>
                    {symlink && (
                      <div className="flex items-start gap-2 text-muted-foreground/70">
                        <Link size={12} className="mt-0.5 shrink-0" />
                        <span className="break-all text-xs italic">
                          {symlink}
                        </span>
                      </div>
                    )}
                  </div>
                ))}
                {hookEvent && (
                  <div className="flex items-center gap-2 text-muted-foreground mt-0.5">
                    <span className="text-xs">
                      {t("detailPaths.event", { count: 1 })}: {hookEvent}
                    </span>
                  </div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
