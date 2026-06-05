import {
  AlertTriangle,
  Calendar,
  Download,
  Folder,
  FolderOpen,
  GitBranch,
  Info,
  Loader2,
  Pencil,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { DeleteDialog } from "@/components/extensions/delete-dialog";
import { CliSections } from "@/components/extensions/detail-cli-sections";
import { DetailHeader } from "@/components/extensions/detail-header";
import { DetailPaths } from "@/components/extensions/detail-paths";
import { PermissionDetail } from "@/components/extensions/permission-detail";
import { SkillFileSection } from "@/components/extensions/skill-file-section";
import { HermesCategoryPicker } from "@/components/shared/hermes-category-picker";
import i18n from "@/lib/i18n";
import { api } from "@/lib/invoke";
import { isDesktop } from "@/lib/transport";
import type { ConfigScope, ExtensionContent as ExtContent } from "@/lib/types";
import {
  agentDisplayName,
  extensionGroupKey,
  groupOwnerRepo,
  isValidPackFormat,
  normalizePack,
  scopeKey,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { findCliChildren } from "@/stores/extension-helpers";
import { useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString(i18n.resolvedLanguage ?? "en", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function ExtensionDetail() {
  const { t } = useTranslation("extensions");
  const { t: tc } = useTranslation("common");
  const grouped = useExtensionStore((s) => s.grouped);
  const selectedId = useExtensionStore((s) => s.selectedId);
  const setSelectedId = useExtensionStore((s) => s.setSelectedId);
  const toggle = useExtensionStore((s) => s.toggle);
  const updateStatuses = useExtensionStore((s) => s.updateStatuses);
  const updateExtension = useExtensionStore((s) => s.updateExtension);
  const updatePack = useExtensionStore((s) => s.updatePack);
  const installToAgent = useExtensionStore((s) => s.installToAgent);
  const deleteInstances = useExtensionStore((s) => s.deleteInstances);
  const extensions = useExtensionStore((s) => s.extensions);
  const group = grouped().find((g) => g.groupKey === selectedId);
  /** Per-instance content data keyed by instance id */
  const [instanceData, setInstanceData] = useState<Map<string, ExtContent>>(
    new Map(),
  );
  const [loadingContent, setLoadingContent] = useState(false);
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  // Cross-agent install (install_to_agent) needs a source instance to copy
  // from; v1 service::install_to_agent has no target_scope param so it uses
  // the source's scope implicitly. Without a global instance there's no
  // scope-safe source — we block. v2 will add target_scope and lift this gate.
  const globalSourceInstance = group?.instances.find(
    (i) => i.scope.type === "global",
  );
  const projectScopeBlocked = !globalSourceInstance;
  const [deploying, setDeploying] = useState<string | null>(null);
  // Hermes cross-agent deploy: show category picker before confirming install
  const [hermesCategoryPicker, setHermesCategoryPicker] = useState(false);
  const [hermesCategories, setHermesCategories] = useState<string[]>([]);
  const [hermesDeployCategory, setHermesDeployCategory] = useState("local");
  const [activeInstanceId, setActiveInstanceId] = useState<string | null>(null);
  const [showDelete, setShowDelete] = useState(false);
  const [deleteAgents, setDeleteAgents] = useState<Set<string>>(new Set());
  const [deleting, setDeleting] = useState(false);
  // Tracks whether the source pill is in edit mode (input visible). Stays
  // false in the bound / empty states so the link chip and CTA button don't
  // get an accidental input behind them.
  const [editingPack, setEditingPack] = useState(false);
  // All physical paths where this skill exists, keyed by agent name
  const [skillLocations, setSkillLocations] = useState<
    [string, string, string | null][]
  >([]);

  // Reset state and load ALL instance data when group changes. Depends only
  // on `group?.groupKey` — see ignore directive below.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `group` is a fresh reference on every render (grouped() may rebuild it); adding it (or any of its nested fields) to the dep list re-fires this effect on every render and resets edit-mode state mid-edit. groupKey is the stable identity we actually care about.
  useEffect(() => {
    if (group && group.instances.length > 0) {
      setActiveInstanceId(group.instances[0].id);
      setEditingPack(false);
      // Load content + path for every instance in parallel
      setLoadingContent(true);
      setInstanceData(new Map());
      Promise.all(
        group.instances.map((inst) =>
          api
            .getExtensionContent(inst.id)
            .then((res) => [inst.id, res] as const)
            .catch(() => [inst.id, null] as const),
        ),
      ).then((results) => {
        const map = new Map<string, ExtContent>();
        for (const [id, data] of results) {
          if (data) map.set(id, data);
        }
        setInstanceData(map);
        setLoadingContent(false);
      });
      // Load skill locations for skills
      if (group.kind === "skill") {
        api
          .getSkillLocations(group.name)
          .then(setSkillLocations)
          .catch(() => setSkillLocations([]));
      } else {
        setSkillLocations([]);
      }
    } else {
      setActiveInstanceId(null);
      setInstanceData(new Map());
      setSkillLocations([]);
    }
    setShowDelete(false);
    setDeleteAgents(new Set());
  }, [group?.groupKey]);

  // Load content + skill locations for any instances added after the initial load
  // (e.g. after a successful cross-agent install without navigating away).
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentional — only fire when instance count changes, not on every group rebuild
  useEffect(() => {
    if (!group) return;
    const unloaded = group.instances.filter((i) => !instanceData.has(i.id));
    if (unloaded.length === 0) return;
    Promise.all(
      unloaded.map((inst) =>
        api
          .getExtensionContent(inst.id)
          .then((res) => [inst.id, res] as const)
          .catch(() => [inst.id, null] as const),
      ),
    ).then((results) => {
      setInstanceData((prev) => {
        const updated = new Map(prev);
        for (const [id, data] of results) {
          if (data) updated.set(id, data);
        }
        return updated;
      });
    });
    if (group.kind === "skill") {
      api
        .getSkillLocations(group.name)
        .then(setSkillLocations)
        .catch(() => {});
    }
  }, [group?.instances.length]);

  // Reset deleteAgents when showDelete is toggled on
  useEffect(() => {
    if (showDelete && group) {
      setDeleteAgents(new Set());
    }
  }, [showDelete, group]);

  if (!group) return null;

  // Find CLI parent for child extensions (by cli_parent_id or matching pack)
  const cliParent =
    group.kind !== "cli"
      ? (() => {
          const parent = extensions.find(
            (e) =>
              e.kind === "cli" &&
              (e.id === group.instances[0]?.cli_parent_id ||
                (group.pack && e.pack === group.pack)),
          );
          if (!parent) return null;
          const parentGroupKey = extensionGroupKey(parent);
          return {
            name: parent.name,
            onNavigate: () => setSelectedId(parentGroupKey),
          };
        })()
      : null;

  return (
    <div
      onWheel={(e) => e.stopPropagation()}
      className="relative flex h-full flex-col rounded-xl border border-border bg-card shadow-sm"
    >
      {/* Fixed header */}
      <DetailHeader
        group={group}
        updateStatuses={updateStatuses}
        updateExtension={updateExtension}
        onClose={() => setSelectedId(null)}
      />

      {/* Scrollable body */}
      <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
        <p className="text-sm text-muted-foreground">
          {cliParent && (
            <>
              <span>
                {group.kind === "mcp"
                  ? t("detail.partOfMcpPrefix")
                  : t("detail.partOfSkillPrefix")}
              </span>
              <button
                onClick={cliParent.onNavigate}
                className="font-medium text-primary hover:underline"
              >
                {cliParent.name}
              </button>
              {group.description ? ". " : ""}
            </>
          )}
          {group.description}
        </p>

        {/* Codex hook trust reminder.
         * Codex CLI 0.129+ requires the user to explicitly trust each hook
         * via `codex /hooks` before it executes. We don't track trust state
         * here (Codex's hash format may evolve and we'd silently break) — a
         * static reminder is more robust than detection. Shown for any hook
         * with `codex` in its agents list. */}
        {group.kind === "hook" && group.agents.includes("codex") && (
          <div className="mt-3 flex items-start gap-2 rounded-md border border-primary/20 bg-primary/5 px-3 py-2 text-xs text-foreground">
            <Info size={14} className="mt-0.5 shrink-0 text-primary" />
            <span>{t("detail.codexHooksWarning")}</span>
          </div>
        )}

        {/* 1. Status + Source row */}
        <div className="mt-4 flex items-center gap-2">
          <button
            onClick={() => {
              toggle(group.groupKey, !group.enabled);
              const action = group.enabled
                ? t("detail.disabled")
                : t("detail.enabled");
              toast.success(t("detail.toggleSuccess", { action }));
            }}
            aria-pressed={group.enabled}
            className={`shrink-0 rounded-full px-3 py-1 text-xs font-medium ${
              group.enabled
                ? "bg-primary/10 text-primary"
                : "bg-muted text-muted-foreground"
            }`}
          >
            {group.enabled ? t("detail.enabled") : t("detail.disabled")}
          </button>
          {/* Source pill — three-state machine:
           * 1. editing: input box (autofocus, Esc cancels, blur commits)
           * 2. bound (any source URL or pack): GitHub link chip + edit icon
           * 3. empty: "+ Bind source" CTA button that enters edit mode.
           * The bound link's text is `owner/repo`, derived in groupOwnerRepo
           * from pack → source.url → install_meta.url (union of every place
           * source info can live in our data model).
           */}
          {(() => {
            const ownerRepo = groupOwnerRepo(group, extensions);
            if (editingPack) {
              // Try to commit `raw` as a new pack value. Returns whether the
              // input should leave edit mode. Caller decides what to do on
              // false (Enter shows a toast to nudge the user, blur silently
              // dismisses — otherwise click-elsewhere would re-fire the
              // warning every time the input loses focus, trapping the user
              // until they press Esc).
              const commit = (raw: string): boolean => {
                if (!raw) {
                  if (group.pack !== null) updatePack(group.groupKey, null);
                  return true;
                }
                const normalized = normalizePack(raw);
                if (!isValidPackFormat(normalized)) {
                  return false;
                }
                if (normalized !== group.pack) {
                  updatePack(group.groupKey, normalized);
                  toast.success(t("detail.bindSourceSuccess"));
                }
                return true;
              };
              return (
                <input
                  type="text"
                  // biome-ignore lint/a11y/noAutofocus: edit mode is opt-in (user clicked pencil/CTA); focusing the input they just summoned is the expected behavior, not a surprise focus trap.
                  autoFocus
                  placeholder={t("detail.bindSourcePlaceholder")}
                  defaultValue={group.pack ?? ""}
                  key={`${group.groupKey}-edit`}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") {
                      setEditingPack(false);
                    } else if (e.key === "Enter") {
                      // preventDefault to suppress any platform default for
                      // Enter on form-less inputs (none in standard browsers,
                      // but defensive against Tauri WebView quirks).
                      e.preventDefault();
                      if (commit(e.currentTarget.value.trim())) {
                        setEditingPack(false);
                      } else {
                        toast.warning(t("detail.bindSourceInvalid"));
                        // Stay in edit mode so the user can correct it. No
                        // synthetic blur fires from Enter on a form-less
                        // input, so onBlur won't race with us.
                      }
                    } else if ((e.metaKey || e.ctrlKey) && e.key === "a") {
                      // Tauri's WebView lets Cmd+A bubble past focused inputs
                      // and selects the whole document. Take it back: select
                      // the input's text manually and stop the event from
                      // propagating any further. Mirror this in any other
                      // input that has the same issue (search box, etc.).
                      e.currentTarget.select();
                      e.preventDefault();
                      e.stopPropagation();
                    }
                  }}
                  onBlur={(e) => {
                    // Click-elsewhere: try to commit; if invalid, just exit
                    // edit mode silently (no toast — the user is clicking
                    // away, not asking us to validate again).
                    commit(e.target.value.trim());
                    setEditingPack(false);
                  }}
                  className="min-w-0 flex-1 rounded-full border border-border bg-card px-2.5 py-1 text-xs text-muted-foreground focus:border-ring focus:outline-none"
                />
              );
            }
            if (ownerRepo) {
              return (
                <div className="flex min-w-0 flex-1 items-center gap-1">
                  <a
                    href={`https://github.com/${ownerRepo}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="min-w-0 flex-1 truncate rounded-full bg-muted/50 px-2.5 py-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
                    title={`https://github.com/${ownerRepo}`}
                  >
                    {ownerRepo}
                  </a>
                  <button
                    type="button"
                    onClick={() => setEditingPack(true)}
                    aria-label={t("detail.editSource")}
                    title={t("detail.editSource")}
                    className="shrink-0 rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  >
                    <Pencil size={12} />
                  </button>
                </div>
              );
            }
            return (
              <button
                type="button"
                onClick={() => setEditingPack(true)}
                className="min-w-0 flex-1 truncate rounded-full border border-dashed border-border px-2.5 py-1 text-left text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                {t("detail.bindSource")}
              </button>
            );
          })()}
        </div>

        {/* 2. Info */}
        <div className="mt-4 space-y-2 text-sm">
          <h4 className="mb-1 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {t("detail.info")}
          </h4>
          {group.instances.some(
            (inst) =>
              updateStatuses.get(inst.id)?.status === "removed_from_repo",
          ) && (
            <div className="flex items-center gap-2 text-muted-foreground">
              <AlertTriangle size={14} />
              <span>{t("detail.removedFromRepo")}</span>
            </div>
          )}
          {(() => {
            // Surface check_updates errors (repo not found, network failure,
            // invalid URL, …). Without this row a manual-bound skill pointing
            // at a non-existent repo looks indistinguishable from a healthy
            // one in the UI. Title attribute holds the full message in case
            // it overflows.
            for (const inst of group.instances) {
              const status = updateStatuses.get(inst.id);
              if (status?.status === "error") {
                return (
                  <div className="flex items-start gap-2 text-muted-foreground">
                    <AlertTriangle size={14} className="mt-0.5 shrink-0" />
                    <span className="break-words" title={status.message}>
                      {t("detail.updateCheckFailed", {
                        message: status.message,
                      })}
                    </span>
                  </div>
                );
              }
            }
            return null;
          })()}
          <div className="flex items-center gap-2 text-muted-foreground">
            <Calendar size={14} />
            <span>
              {t("detail.installed", {
                time:
                  group.kind === "skill" ||
                  group.kind === "plugin" ||
                  group.kind === "cli"
                    ? formatDate(group.installed_at)
                    : "\u2014",
              })}
            </span>
          </div>
          {(() => {
            // After Phase C dedup, a single group can span multiple scopes
            // (same skill installed both globally and in a project). Show
            // each unique scope on its own row so the user can see exactly
            // where this extension lives.
            const uniqueScopes = new Map<string, ConfigScope>();
            for (const inst of group.instances) {
              uniqueScopes.set(scopeKey(inst.scope), inst.scope);
            }
            return [...uniqueScopes.values()].map((s) => (
              <div
                key={scopeKey(s)}
                className="flex items-center gap-2 text-muted-foreground"
              >
                <Folder size={14} />
                <span className="truncate">
                  {s.type === "global" ? tc("scope.global") : s.name}
                </span>
              </div>
            ));
          })()}
          {group.source.origin === "git" &&
            group.source.url &&
            !group.instances.find((i) => i.install_meta) && (
              <div className="flex items-center gap-2 text-muted-foreground">
                <GitBranch size={14} />
                <span className="truncate">{group.source.url}</span>
              </div>
            )}
        </div>

        {/* 3. Agents + Deploy */}
        <div className="mt-4">
          <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {t("detail.agents")}
          </h4>
          <div className="flex flex-wrap gap-1">
            {group.agents.map((agent) => (
              <span
                key={agent}
                className="inline-flex rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary"
              >
                {agentDisplayName(agent)}
              </span>
            ))}
          </div>
        </div>

        {(group.kind === "skill" ||
          group.kind === "mcp" ||
          group.kind === "hook" ||
          group.kind === "cli") &&
          (() => {
            const detectedAgents = sortAgents(
              agents.filter((a) => a.detected),
              agentOrder,
            );
            const AGENTS_WITHOUT_HOOKS = new Set(["antigravity", "opencode"]);
            // "Install to Agent" implicitly targets Global scope (see
            // service::install_to_agent v1). Filter by which agents already
            // have a GLOBAL instance, not just any instance — otherwise a
            // skill that exists only in project scope (e.g., user deleted
            // the global row but kept the project copy) hides its agent
            // from the install list and you can't recreate the global row.
            const agentsWithGlobalInstance = new Set(
              group.instances
                .filter((i) => i.scope.type === "global")
                .flatMap((i) => i.agents),
            );
            const otherAgents = detectedAgents.filter(
              (a) => !agentsWithGlobalInstance.has(a.name),
            );
            if (otherAgents.length === 0) return null;
            return (
              <div className="mt-3">
                <div className="mb-2 flex items-baseline gap-2">
                  <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    {t("detail.installToAgent")}
                  </h4>
                  {projectScopeBlocked && (
                    <span className="text-[10px] text-muted-foreground/60">
                      {t("detail.globalOnly")}
                    </span>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {otherAgents.map((agent) => {
                    const hookUnsupported =
                      group.kind === "hook" &&
                      AGENTS_WITHOUT_HOOKS.has(agent.name);
                    const isHermes =
                      agent.name === "hermes" && group.kind === "skill";
                    return (
                      <button
                        key={agent.name}
                        disabled={
                          deploying === agent.name ||
                          hookUnsupported ||
                          projectScopeBlocked
                        }
                        title={
                          projectScopeBlocked
                            ? t("detail.crossAgentSoon")
                            : hookUnsupported
                              ? t("detail.hooksNotSupported")
                              : undefined
                        }
                        onClick={async () => {
                          if (hookUnsupported || projectScopeBlocked) return;
                          if (isHermes) {
                            // Show category picker before deploying
                            const cats = await api
                              .listHermesCategories()
                              .catch(() => []);
                            setHermesCategories(cats);
                            setHermesDeployCategory(cats[0] ?? "local");
                            setHermesCategoryPicker(true);
                            return;
                          }
                          setDeploying(agent.name);
                          try {
                            if (group.kind === "cli") {
                              const children = findCliChildren(
                                extensions,
                                group.instances[0]?.id,
                                group.pack,
                              );
                              const seen = new Set<string>();
                              for (const child of children) {
                                if (seen.has(child.name + child.kind)) continue;
                                seen.add(child.name + child.kind);
                                await installToAgent(child.id, agent.name);
                              }
                            } else if (globalSourceInstance) {
                              await installToAgent(
                                globalSourceInstance.id,
                                agent.name,
                              );
                            }
                            toast.success(
                              t("detail.installToSuccess", {
                                agent: agentDisplayName(agent.name),
                              }),
                            );
                          } catch {
                            toast.error(
                              t("detail.installToFailed", {
                                agent: agentDisplayName(agent.name),
                              }),
                            );
                          } finally {
                            setDeploying(null);
                          }
                        }}
                        className={
                          hookUnsupported || projectScopeBlocked
                            ? "flex items-center gap-1.5 rounded-lg border border-border px-3 py-1.5 text-xs font-medium text-muted-foreground/50 cursor-not-allowed"
                            : "flex items-center gap-1.5 rounded-lg border border-border bg-primary/10 px-3 py-1.5 text-xs font-medium text-foreground hover:bg-primary/20 hover:border-ring disabled:opacity-50"
                        }
                      >
                        {deploying === agent.name ? (
                          <Loader2 size={12} className="animate-spin" />
                        ) : (
                          <Download size={12} />
                        )}
                        {agentDisplayName(agent.name)}
                        {hookUnsupported && (
                          <span className="text-[10px] opacity-60 ml-0.5">
                            (N/A)
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>

                {/* Hermes category picker — shown after clicking the Hermes deploy button */}
                {hermesCategoryPicker && (
                  <div className="mt-2 rounded-lg border border-border bg-muted/20 p-3">
                    <p className="mb-2 text-xs font-medium text-foreground">
                      {tc("hermesCategory.choose")}
                    </p>
                    <HermesCategoryPicker
                      categories={hermesCategories}
                      value={hermesDeployCategory}
                      onChange={setHermesDeployCategory}
                      disabled={deploying === "hermes"}
                    />
                    <div className="mt-2.5 flex items-center gap-2">
                      <button
                        disabled={deploying === "hermes"}
                        onClick={async () => {
                          if (!globalSourceInstance) return;
                          const category =
                            hermesDeployCategory.trim() || "local";
                          setDeploying("hermes");
                          try {
                            await installToAgent(
                              globalSourceInstance.id,
                              "hermes",
                              category,
                            );
                            toast.success(
                              t("detail.installToSuccess", {
                                agent: agentDisplayName("hermes"),
                              }),
                            );
                            setHermesCategoryPicker(false);
                          } catch {
                            toast.error(
                              t("detail.installToFailed", {
                                agent: agentDisplayName("hermes"),
                              }),
                            );
                          } finally {
                            setDeploying(null);
                          }
                        }}
                        className="rounded-lg bg-primary px-3 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                      >
                        {deploying === "hermes" ? (
                          <Loader2
                            size={11}
                            className="animate-spin inline mr-1"
                          />
                        ) : null}
                        {tc("hermesCategory.install")}
                      </button>
                      <button
                        onClick={() => setHermesCategoryPicker(false)}
                        className="text-xs text-muted-foreground hover:text-foreground"
                      >
                        {tc("cancel")}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            );
          })()}

        {/* 5. Permissions */}
        {group.permissions.length > 0 && (
          <div className="mt-4">
            <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("detail.permissions")}
            </h4>
            <div className="space-y-2">
              {group.permissions.map((p, i) => (
                <PermissionDetail key={i} perm={p} />
              ))}
            </div>
          </div>
        )}

        {/* 6+7. CLI Details + Associated Extensions */}
        <CliSections group={group} extensions={extensions} />

        {/* 8. Paths (per-agent breakdown) — skip for CLI */}
        <DetailPaths
          group={group}
          instanceData={instanceData}
          skillLocations={skillLocations}
          agentOrder={agentOrder}
        />

        {/* 9. Content / Documentation — skip for hooks and CLIs */}
        {group.kind !== "hook" &&
          group.kind !== "cli" &&
          group.kind !== "mcp" && (
            <div className="mt-4">
              <div className="mb-2 flex items-center justify-between">
                <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  {t("detail.documentation")}
                </h4>
                {(() => {
                  const activePath = activeInstanceId
                    ? instanceData.get(activeInstanceId)?.path
                    : undefined;
                  return (
                    isDesktop() &&
                    activePath && (
                      <button
                        onClick={() => api.revealInFileManager(activePath)}
                        className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                      >
                        <FolderOpen size={12} />
                        {t("detail.openInFinder")}
                      </button>
                    )
                  );
                })()}
              </div>
              {/* Agent tabs for switching instance content */}
              {group.instances.length > 1 && (
                <div className="mb-2 flex flex-wrap gap-1">
                  {group.instances.map((instance) => (
                    <button
                      key={instance.id}
                      onClick={() => setActiveInstanceId(instance.id)}
                      className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-colors ${
                        activeInstanceId === instance.id
                          ? "bg-primary/20 text-primary"
                          : "bg-muted text-muted-foreground hover:bg-muted/80"
                      }`}
                    >
                      {agentDisplayName(instance.agents[0] ?? "unknown")}
                    </button>
                  ))}
                </div>
              )}

              {/* File tree + SKILL.md content */}
              {activeInstanceId && (
                <SkillFileSection
                  instanceId={activeInstanceId}
                  content={instanceData.get(activeInstanceId)?.content ?? null}
                  dirPath={instanceData.get(activeInstanceId)?.path ?? null}
                  loading={loadingContent}
                  kind={group.kind}
                />
              )}
            </div>
          )}

        {/* 10. Delete trigger */}
        <div className="mt-4">
          <button
            onClick={() => setShowDelete(true)}
            className="flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium text-destructive hover:bg-destructive/10"
          >
            <Trash2 size={12} />
            {t("detail.deleteButton")}
          </button>
        </div>

        {/* Delete confirmation dialog */}
        {showDelete && (
          <DeleteDialog
            group={group}
            instanceData={instanceData}
            deleting={deleting}
            deleteAgents={deleteAgents}
            setDeleteAgents={setDeleteAgents}
            childExtensions={
              group.kind === "cli"
                ? findCliChildren(
                    extensions,
                    group.instances[0]?.id,
                    group.pack,
                  )
                : undefined
            }
            skillLocations={group.kind === "skill" ? skillLocations : undefined}
            onDelete={async (ids) => {
              setDeleting(true);
              try {
                await deleteInstances(group.groupKey, ids);
                const isFullDelete = ids.length === group.instances.length;
                if (isFullDelete) {
                  toast.success(t("detail.deleteSuccess"));
                  setSelectedId(null);
                } else {
                  const affectedAgents = Array.from(
                    new Set(
                      group.instances
                        .filter((i) => ids.includes(i.id))
                        .flatMap((i) => i.agents),
                    ),
                  );
                  toast.success(
                    t("detail.deleteFromAgentsSuccess", {
                      agents: affectedAgents.map(agentDisplayName).join(", "),
                    }),
                  );
                }
              } catch {
                toast.error(t("detail.deleteFailed"));
              } finally {
                setDeleting(false);
                setShowDelete(false);
              }
            }}
            onClose={() => setShowDelete(false)}
          />
        )}
      </div>
    </div>
  );
}
