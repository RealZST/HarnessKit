import type { TFunction } from "i18next";
import { AlertTriangle, FolderOpen, Link, Loader2, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import type {
  ExtensionContent as ExtContent,
  Extension,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName, instanceDir } from "@/lib/types";

type DeleteItem = {
  key: string;
  agents: string[];
  /** Instance IDs this item represents — passed to the backend so deletion
   *  is scoped per-row, not per-agent (which collapses multi-scope rows). */
  ids: string[];
  paths: string[];
  mcps: string[];
  shared: boolean;
  symlink?: string;
  description?: string;
  configCleanup?: string;
};

/**
 * Build path-based delete items from skill locations.
 * Each item = one physical path, with agent names as the primary label.
 * IDs are resolved by matching (agent, source_path dir) against group instances
 * so a multi-scope skill (same agent, two scopes, two source_paths) produces
 * two distinct items — preventing the old "delete from agent X" from silently
 * removing rows in other scopes for the same agent.
 */
function buildPathItems(
  locations: [string, string, string | null][],
  instances: GroupedExtension["instances"],
): DeleteItem[] {
  const pathMap = new Map<
    string,
    { agents: string[]; ids: string[]; symlink?: string }
  >();
  for (const [agent, path, symlinkTarget] of locations) {
    const entry = pathMap.get(path) ?? { agents: [], ids: [] };
    if (!entry.agents.includes(agent)) entry.agents.push(agent);
    for (const inst of instances) {
      if (!inst.agents.includes(agent)) continue;
      if (instanceDir(inst) !== path) continue;
      if (!entry.ids.includes(inst.id)) entry.ids.push(inst.id);
    }
    if (symlinkTarget) entry.symlink = symlinkTarget;
    pathMap.set(path, entry);
  }

  const items: DeleteItem[] = [];
  for (const [path, { agents, ids, symlink }] of pathMap) {
    items.push({
      key: `path:${path}`,
      agents,
      ids,
      paths: [path],
      mcps: [],
      shared: agents.length > 1,
      symlink,
    });
  }
  return items;
}

/**
 * Build agent-based delete items from instances (for MCP, Hook, Plugin).
 */
function buildAgentItems(
  instances: GroupedExtension["instances"],
  instanceData: Map<string, ExtContent>,
  kind: string,
  name: string,
  t: TFunction<"extensions">,
): DeleteItem[] {
  return instances.map((inst) => {
    const data = instanceData.get(inst.id);
    const configPath = data?.path ?? null;
    const isConfigBased = kind === "mcp" || kind === "hook";
    const agentName = inst.agents[0];
    const desc = isConfigBased
      ? kind === "mcp"
        ? t("delete.removeMcpDesc", { name })
        : t("delete.removeHookDesc")
      : null;
    const configCleanup =
      kind === "plugin"
        ? agentName === "claude"
          ? "~/.claude/settings.json"
          : agentName === "codex"
            ? "~/.codex/config.toml"
            : agentName === "gemini"
              ? "~/.gemini/extensions/extension-enablement.json"
              : agentName === "copilot"
                ? "~/Library/Application Support/Code/User/globalStorage/state.vscdb" // TODO: Linux path differs (~/.config/Code/User/...)
                : null
        : null;
    return {
      key: `agent:${inst.agents[0]}`,
      agents: [...inst.agents],
      ids: [inst.id],
      paths: configPath ? [configPath] : [],
      mcps: [],
      shared: false,
      description: desc ?? undefined,
      configCleanup: configCleanup ?? undefined,
      symlink: data?.symlink_target ?? undefined,
    };
  });
}

export function DeleteDialog({
  group,
  instanceData,
  deleting,
  deleteAgents,
  setDeleteAgents,
  onDelete,
  onClose,
  childExtensions,
  skillLocations,
}: {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  deleting: boolean;
  deleteAgents: Set<string>;
  setDeleteAgents: (s: Set<string>) => void;
  onDelete: (ids: string[]) => void;
  onClose: () => void;
  childExtensions?: Extension[];
  skillLocations?: [string, string, string | null][];
}) {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const dlgRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  useFocusTrap(dlgRef, true);

  useEffect(() => {
    setDeleteAgents(new Set());
  }, [setDeleteAgents]);

  const displayName =
    group.kind === "hook"
      ? (() => {
          const parts = group.name.split(":");
          if (parts.length >= 3) {
            const cmd = parts.slice(2).join(":");
            return cmd
              .split(" ")
              .map((t) => t.split("/").pop() || t)
              .join(" ");
          }
          return group.name;
        })()
      : group.name;

  const isCli = group.kind === "cli";

  // ── CLI Uninstall Dialog ──
  if (isCli) {
    const binaryPath = group.instances[0]?.cli_meta?.binary_path;
    // Deduplicate children by name+kind
    const childMap = new Map<string, { name: string; kind: string }>();
    for (const child of childExtensions ?? []) {
      const key = `${child.kind}:${child.name}`;
      if (!childMap.has(key))
        childMap.set(key, { name: child.name, kind: child.kind });
    }
    const children = [...childMap.values()];

    return (
      <div
        className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
        onClick={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />
        <div
          ref={dlgRef}
          role="dialog"
          aria-modal="true"
          aria-label={t("delete.uninstallCliAria")}
          tabIndex={-1}
          className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
        >
          <div className="flex items-center gap-2 mb-4">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
              <Trash2 size={16} />
            </span>
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                {t("delete.uninstallTitle", { name: displayName })}
              </h3>
              <p className="text-xs text-muted-foreground">
                {t("delete.irreversible")}
              </p>
            </div>
          </div>

          <div className="space-y-3">
            {children.length > 0 && (
              <>
                <p className="text-xs text-muted-foreground">
                  {t("delete.alsoRemoves")}
                </p>
                <div className="space-y-1 rounded-lg border border-border bg-muted/30 p-2.5">
                  {children.map((child) => (
                    <div
                      key={`${child.kind}:${child.name}`}
                      className="flex items-center gap-2 text-xs"
                    >
                      <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-[10px] font-medium uppercase text-muted-foreground">
                        {child.kind}
                      </span>
                      <span className="text-foreground">{child.name}</span>
                    </div>
                  ))}
                </div>
              </>
            )}

            {binaryPath && (
              <div className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                <span>{t("delete.binaryRemoved", { path: binaryPath })}</span>
              </div>
            )}

            <button
              disabled={deleting}
              onClick={() => onDelete(group.instances.map((i) => i.id))}
              className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
            >
              {deleting ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Trash2 size={12} />
              )}
              {t("delete.uninstallButton", { name: displayName })}
            </button>
          </div>

          <button
            onClick={onClose}
            disabled={deleting}
            className="mt-4 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
          >
            {tc("actions.cancel")}
          </button>
        </div>
      </div>
    );
  }

  // ── Standard Delete Dialog (skill, MCP, hook, plugin) ──
  const isSkill = group.kind === "skill";

  // skillLocations is scope-agnostic on purpose (the get_skill_locations
  // API surfaces every place a skill named X exists). For deletion we
  // must restrict to paths belonging to *this* group's instances, or the
  // dialog lists e.g. a global same-named skill alongside a project one.
  // Delete is keyed on instance ids built per-(agent, source_path) — see
  // buildPathItems — so multi-scope rows are not accidentally collapsed.
  const instanceDirs = new Set(
    group.instances.map(instanceDir).filter((p): p is string => !!p),
  );
  const filteredSkillLocations =
    skillLocations && instanceDirs.size > 0
      ? skillLocations.filter(([, dir]) => instanceDirs.has(dir))
      : skillLocations;
  const usePathBased =
    isSkill && filteredSkillLocations && filteredSkillLocations.length > 0;

  // Redundant `filteredSkillLocations` check (already implied by usePathBased)
  // is what lets TS narrow inside the true branch — cleaner than a `!`.
  const items: DeleteItem[] =
    usePathBased && filteredSkillLocations
      ? buildPathItems(filteredSkillLocations, group.instances)
      : buildAgentItems(
          group.instances,
          instanceData,
          group.kind,
          group.name,
          t,
        );

  const selectedKeys = deleteAgents;
  const allSelected =
    items.length > 0 && items.every((i) => selectedKeys.has(i.key));
  const isSingle = items.length === 1;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label={t("delete.deleteAria")}
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <Trash2 size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              {t("delete.deleteTitle", { name: displayName })}
            </h3>
            <p className="text-xs text-muted-foreground">
              {t("delete.irreversible")}
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <p className="text-xs text-muted-foreground">
            {isSingle ? t("delete.permanentlyDelete") : t("delete.selectItems")}
          </p>

          <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
            {/* All Items toggle */}
            {!isSingle && (
              <label className="flex items-start gap-2 text-xs cursor-pointer pb-1.5 mb-1.5 border-b border-border/50">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={() => {
                    setDeleteAgents(
                      allSelected
                        ? new Set()
                        : new Set(items.map((i) => i.key)),
                    );
                  }}
                  className="mt-0.5 rounded border-border accent-destructive"
                />
                <span className="font-medium text-foreground">
                  {t("delete.allItems")}
                </span>
              </label>
            )}

            {/* Each deletable item */}
            {items.map((item) => (
              <label
                key={item.key}
                className={`flex items-start gap-2 text-xs ${isSingle ? "" : "cursor-pointer"}`}
              >
                {!isSingle && (
                  <input
                    type="checkbox"
                    checked={selectedKeys.has(item.key)}
                    onChange={() => {
                      const next = new Set(selectedKeys);
                      if (next.has(item.key)) next.delete(item.key);
                      else next.add(item.key);
                      setDeleteAgents(next);
                    }}
                    className="mt-0.5 rounded border-border accent-destructive"
                  />
                )}
                <div className="min-w-0">
                  <span className="font-medium text-foreground">
                    {item.agents.map(agentDisplayName).join(", ")}
                  </span>
                  {item.shared && (
                    <span className="ml-1.5 text-[10px] text-chart-5 font-medium">
                      {t("delete.sharedBadge")}
                    </span>
                  )}
                  {item.description && (
                    <p className="text-muted-foreground mt-0.5">
                      {item.description}
                    </p>
                  )}
                  {item.paths.map((p) => (
                    <p
                      key={p}
                      className="text-muted-foreground flex items-start gap-1 mt-0.5"
                    >
                      <FolderOpen size={10} className="mt-0.5 shrink-0" />
                      <span className="break-all">{p}</span>
                    </p>
                  ))}
                  {item.configCleanup && (
                    <p className="text-muted-foreground flex items-start gap-1 mt-0.5">
                      <Trash2 size={10} className="mt-0.5 shrink-0" />
                      <span className="break-all">
                        {t("delete.alsoRemovesEntry", {
                          path: item.configCleanup,
                        })}
                      </span>
                    </p>
                  )}
                  {!item.description &&
                    item.mcps.map((name) => (
                      <p key={name} className="text-muted-foreground mt-0.5">
                        {t("delete.mcpInfo", { name })}
                      </p>
                    ))}
                  {item.symlink && (
                    <p className="flex items-center gap-1 text-chart-5 mt-0.5">
                      <Link size={10} className="shrink-0" />
                      <span className="break-all">{item.symlink}</span>
                    </p>
                  )}
                </div>
              </label>
            ))}
          </div>

          {/* Symlink warnings */}
          {(() => {
            const selected = isSingle
              ? items
              : items.filter((i) => selectedKeys.has(i.key));
            const warnings: React.ReactNode[] = [];

            const symlinkItems = selected.filter((i) => i.symlink);
            if (symlinkItems.length > 0) {
              warnings.push(
                <div
                  key="symlink"
                  className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5"
                >
                  <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                  <span>
                    {t("delete.symlinkWarning", {
                      count: symlinkItems.length,
                      paths: symlinkItems.map((s) => s.symlink).join(", "),
                    })}
                  </span>
                </div>,
              );
            }

            const selectedPaths = new Set(selected.flatMap((i) => i.paths));
            const affectedSymlinks = items.filter(
              (i) =>
                i.symlink &&
                selectedPaths.has(i.symlink) &&
                !selected.includes(i),
            );
            if (affectedSymlinks.length > 0) {
              const affectedAgents = affectedSymlinks.flatMap((i) => i.agents);
              warnings.push(
                <div
                  key="broken-symlink"
                  className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5"
                >
                  <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                  <span>
                    {t("delete.symlinkPointersWarning", {
                      count: affectedAgents.length,
                      agents: affectedAgents.map(agentDisplayName).join(", "),
                    })}
                  </span>
                </div>,
              );
            }

            return warnings.length > 0 ? warnings : null;
          })()}

          {/* Delete button */}
          {isSingle ? (
            <button
              disabled={deleting}
              onClick={() => onDelete(items[0].ids)}
              className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
            >
              {deleting ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Trash2 size={12} />
              )}
              {t("delete.deleteFromAgents", {
                agents: items[0].agents.map(agentDisplayName).join(", "),
              })}
            </button>
          ) : (
            <button
              disabled={deleting || selectedKeys.size === 0}
              onClick={() => {
                const ids = new Set<string>();
                for (const item of items) {
                  if (selectedKeys.has(item.key)) {
                    for (const id of item.ids) ids.add(id);
                  }
                }
                onDelete(Array.from(ids));
              }}
              className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
            >
              {deleting ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Trash2 size={12} />
              )}
              {t("delete.removeCount", { count: selectedKeys.size })}
            </button>
          )}
        </div>

        {/* Cancel */}
        <button
          onClick={onClose}
          disabled={deleting}
          className="mt-4 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
        >
          {tc("actions.cancel")}
        </button>
      </div>
    </div>
  );
}
