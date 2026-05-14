import { FolderOpen, GitCommitHorizontal, Link } from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  ConfigScope,
  ExtensionContent as ExtContent,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName, instanceVersion } from "@/lib/types";

interface DetailPathsProps {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  skillLocations: [string, string, string | null][];
  agentOrder: readonly string[];
}

function instanceDir(sourcePath: string): string {
  return sourcePath.replace(/\/SKILL\.md(\.disabled)?$/, "");
}

function ScopePill({
  scope,
  globalLabel,
}: {
  scope: ConfigScope;
  globalLabel: string;
}) {
  const isGlobal = scope.type === "global";
  const classes = isGlobal
    ? "bg-blue-500/15 text-blue-700 ring-blue-500/25 dark:text-blue-300"
    : "bg-emerald-500/15 text-emerald-700 ring-emerald-500/25 dark:text-emerald-300";
  return (
    <span
      className={`inline-block max-w-[160px] truncate rounded-full px-2 py-0.5 text-[10px] font-medium ring-1 ring-inset ${classes}`}
      title={isGlobal ? undefined : scope.name}
    >
      {isGlobal ? globalLabel : scope.name}
    </span>
  );
}

export function DetailPaths({
  group,
  instanceData,
  skillLocations,
  agentOrder,
}: DetailPathsProps) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
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

  // Sort instances by agent order, then global-before-project, then by
  // project name. Stable so multi-scope on the same agent clusters.
  const agentRank = new Map(agentOrder.map((a, i) => [a, i] as const));
  const sortedInstances = [...group.instances].sort((a, b) => {
    const aAgent = a.agents[0] ?? "unknown";
    const bAgent = b.agents[0] ?? "unknown";
    const aRank = agentRank.get(aAgent) ?? Number.MAX_SAFE_INTEGER;
    const bRank = agentRank.get(bAgent) ?? Number.MAX_SAFE_INTEGER;
    if (aRank !== bRank) return aRank - bRank;
    if (a.scope.type !== b.scope.type)
      return a.scope.type === "global" ? -1 : 1;
    if (a.scope.type === "project" && b.scope.type === "project")
      return a.scope.name.localeCompare(b.scope.name);
    return 0;
  });
  return (
    <div className="mt-4">
      <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        {t("detailPaths.paths")}
      </h4>
      <div className="space-y-3">
        {sortedInstances.map((inst) => {
          const agent = inst.agents[0] ?? "unknown";
          const data = instanceData.get(inst.id);
          const dir = inst.source_path ? instanceDir(inst.source_path) : null;
          const locations = dir
            ? skillLocations.filter(([a, d]) => a === agent && d === dir)
            : [];
          const version = instanceVersion(inst);
          // Hooks: surface this instance's event matcher (first colon-separated
          // segment of inst.name; the wire format is `event:matcher:command`).
          const hookEvent =
            group.kind === "hook" ? inst.name.split(":")[0] : null;
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
                <ScopePill
                  scope={inst.scope}
                  globalLabel={tc("scope.global")}
                />
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
