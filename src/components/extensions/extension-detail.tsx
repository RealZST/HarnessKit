import { clsx } from "clsx";
import {
  AlertTriangle,
  Calendar,
  Copy,
  Download,
  Folder,
  FolderOpen,
  GitBranch,
  Info,
  Loader2,
  Pencil,
  ShieldCheck,
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
import { AgentMascot } from "@/components/shared/agent-mascot/agent-mascot";
import { HermesCategoryPicker } from "@/components/shared/hermes-category-picker";
import { ScopeTargetField } from "@/components/shared/scope-target-field";
import {
  canInstallAtScope,
  canReceiveMcpTransport,
} from "@/lib/agent-capabilities";
import { copyPathToClipboard } from "@/lib/copy-path";
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
import {
  agentsInScope,
  findCliChildren,
  instancesInScope,
  pickSourceInstance,
  resolveInstallTargetScope,
} from "@/stores/extension-helpers";
import { useExtensionStore } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";
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
  const { t: tm } = useTranslation("marketplace");
  // Copy-path strings are shared with the Agents page rows.
  const { t: ta } = useTranslation("agents");
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
  const scope = useScopeStore((s) => s.current);
  // Install to Agent targets the active scope. In All-scopes mode the user
  // must pick a target via ScopeTargetField (null until picked) — the same
  // contract as the Marketplace install panel.
  const [installTargetScope, setInstallTargetScope] =
    useState<ConfigScope | null>(null);
  const effectiveTarget: ConfigScope | null =
    scope.type === "all"
      ? installTargetScope
      : resolveInstallTargetScope(scope);
  const sourceInstance = group
    ? pickSourceInstance(group.instances, effectiveTarget ?? { type: "global" })
    : undefined;
  // Agents that already hold a copy of this group in the target scope —
  // rendered as installed (mascot + check) rather than hidden. Before an
  // All-mode target is picked (effectiveTarget null) nothing is marked
  // installed: picking a target should only ever ADD check badges, never
  // flip an "installed" agent back to installable.
  const agentsInTargetScope = new Set(
    effectiveTarget && group
      ? instancesInScope(group.instances, effectiveTarget).flatMap(
          (i) => i.agents,
        )
      : [],
  );
  // Instances visible in the ACTIVE scope (All = everything) — drives the
  // DOCUMENTATION tabs; DetailPaths applies the same projection internally.
  const scopedInstances = group ? instancesInScope(group.instances, scope) : [];
  // Flash animation for a just-completed install (Marketplace parity).
  const [justInstalled, setJustInstalled] = useState<Set<string>>(new Set());
  const prefersReducedMotion = () =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const flashInstalled = (agentName: string) => {
    if (prefersReducedMotion()) return;
    setJustInstalled((prev) => new Set(prev).add(agentName));
    setTimeout(() => {
      setJustInstalled((prev) => {
        const next = new Set(prev);
        next.delete(agentName);
        return next;
      });
    }, 500);
  };
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

  // Reset install-target state when the active scope changes: a pending
  // Hermes install would otherwise silently retarget the new scope, and a
  // previously picked All-mode target no longer matches the picker UI.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `scope` is the trigger — the effect must re-run on every scope switch even though the body doesn't read it.
  useEffect(() => {
    setHermesCategoryPicker(false);
    setInstallTargetScope(null);
  }, [scope]);

  // Re-anchor the documentation tab when scope or group changes — the
  // selected instance may not exist in the new scope's projection.
  // biome-ignore lint/correctness/useExhaustiveDependencies: scopedInstances is derived from scope+group each render; the in-body guard makes redundant runs no-ops.
  useEffect(() => {
    if (!group) return;
    if (
      activeInstanceId &&
      scopedInstances.some((i) => i.id === activeInstanceId)
    )
      return;
    setActiveInstanceId(
      scopedInstances[0]?.id ?? group.instances[0]?.id ?? null,
    );
  }, [scope, group?.groupKey, activeInstanceId]);

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

      {/* Scrollable body — keyed by group so switching extensions remounts
          the container and the scroll position starts at the top. */}
      <div
        key={group.groupKey}
        className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4"
      >
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
            {agentsInScope(group, scope).map((agent) => (
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
              agents.filter((a) => a.detected && a.enabled),
              agentOrder,
            );
            if (detectedAgents.length === 0) return null;
            return (
              <div className="mt-3">
                <div className="mb-2 flex items-center gap-2">
                  <h4 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                    {t("detail.installToAgent")}
                  </h4>
                  {/* Single-scope mode: inline "📁 scope" hint next to the
                   * header; All-scopes mode renders the required picker on
                   * its own row below (Marketplace parity). */}
                  {scope.type !== "all" && (
                    <ScopeTargetField
                      value={effectiveTarget}
                      onChange={setInstallTargetScope}
                    />
                  )}
                </div>
                {scope.type === "all" && (
                  <div className="mb-2.5">
                    <ScopeTargetField
                      value={effectiveTarget}
                      onChange={setInstallTargetScope}
                    />
                  </div>
                )}
                <div className="flex flex-wrap gap-1.5" aria-live="polite">
                  {detectedAgents.map((agent) => {
                    // All gating reads the backend-derived capabilities
                    // (AgentInfo.capabilities) — same source of truth the
                    // service deploys with.
                    const scopeForCheck = effectiveTarget ?? scope;
                    const hookUnsupported =
                      group.kind === "hook" &&
                      !agent.capabilities.hooks_supported;
                    // Kiro loads workspace hooks but no released version
                    // loads user-level ~/.kiro/hooks/ (kirodotdev/Kiro#5440),
                    // so global-targeted hook installs stay blocked while
                    // project-scope installs go through.
                    const globalHookBlocked =
                      group.kind === "hook" &&
                      effectiveTarget?.type === "global" &&
                      !agent.capabilities.global_hook_install;
                    const scopeIncapable = !canInstallAtScope(
                      agent,
                      group.kind,
                      scopeForCheck,
                    );
                    // Remote (HTTP/SSE) MCP servers can only go to agents
                    // whose config can express that transport — e.g. Codex
                    // takes Streamable HTTP but not SSE. The deployer
                    // enforces the same rule; greying out here means users
                    // never hit that error (issue #105).
                    const mcpTransport =
                      group.kind === "mcp"
                        ? group.instances[0]?.mcp_transport
                        : undefined;
                    const transportUnsupported =
                      group.kind === "mcp" &&
                      !canReceiveMcpTransport(agent, mcpTransport);
                    const blocked =
                      hookUnsupported ||
                      globalHookBlocked ||
                      scopeIncapable ||
                      transportUnsupported;
                    // Already has a copy in the target scope: shown as
                    // installed (check icon) instead of hidden, so the user
                    // can see WHERE this extension already lives.
                    const isInstalled = agentsInTargetScope.has(agent.name);
                    const isFlashing = justInstalled.has(agent.name);
                    const isHermes =
                      agent.name === "hermes" && group.kind === "skill";
                    const disabled =
                      !effectiveTarget ||
                      deploying !== null ||
                      blocked ||
                      isInstalled;
                    return (
                      <button
                        key={agent.name}
                        disabled={disabled}
                        aria-disabled={disabled}
                        title={
                          !effectiveTarget
                            ? tm("detail.selectScopeFirst")
                            : hookUnsupported
                              ? t("detail.hooksNotSupported")
                              : globalHookBlocked
                                ? t("detail.kiroGlobalHooksPending")
                                : scopeIncapable
                                  ? t("detail.projectScopeUnsupported", {
                                      agent: agentDisplayName(agent.name),
                                      kind: group.kind,
                                    })
                                  : transportUnsupported
                                    ? t("detail.remoteTransportUnsupported", {
                                        agent: agentDisplayName(agent.name),
                                        transport: (
                                          mcpTransport ?? "http"
                                        ).toUpperCase(),
                                      })
                                    : undefined
                        }
                        onClick={async () => {
                          if (blocked || isInstalled || !effectiveTarget)
                            return;
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
                              // A CLI bundle can mix kinds (skills + MCP).
                              // Skip children the target agent can't take —
                              // wrong scope, or a remote MCP transport the
                              // agent can't express — instead of failing
                              // mid-loop with a partial install.
                              let skipped = 0;
                              for (const child of children) {
                                if (seen.has(child.name + child.kind)) continue;
                                seen.add(child.name + child.kind);
                                if (
                                  !canInstallAtScope(
                                    agent,
                                    child.kind,
                                    scopeForCheck,
                                  ) ||
                                  (child.kind === "mcp" &&
                                    !canReceiveMcpTransport(
                                      agent,
                                      child.mcp_transport,
                                    ))
                                ) {
                                  skipped += 1;
                                  continue;
                                }
                                await installToAgent(
                                  child.id,
                                  agent.name,
                                  effectiveTarget,
                                );
                              }
                              if (skipped > 0) {
                                toast.info(
                                  t("detail.cliChildrenSkipped", {
                                    count: skipped,
                                  }),
                                );
                              }
                            } else if (sourceInstance) {
                              await installToAgent(
                                sourceInstance.id,
                                agent.name,
                                effectiveTarget,
                              );
                            }
                            flashInstalled(agent.name);
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
                        className={clsx(
                          "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-[background-color,border-color] duration-300 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-primary/10 disabled:hover:border-border",
                          isFlashing
                            ? "border-primary/40 bg-primary/20 text-foreground"
                            : isInstalled
                              ? "border-primary/20 bg-primary/10 text-foreground"
                              : "border-border bg-primary/10 text-foreground hover:bg-primary/20 hover:border-ring",
                        )}
                      >
                        <div className={isInstalled ? "" : "opacity-90"}>
                          <AgentMascot name={agent.name} size={14} />
                        </div>
                        {agentDisplayName(agent.name)}
                        {isInstalled ? (
                          <ShieldCheck
                            size={14}
                            className="animate-scale-in text-primary shrink-0"
                          />
                        ) : deploying === agent.name ? (
                          <Loader2
                            size={12}
                            className="animate-spin shrink-0 text-muted-foreground"
                          />
                        ) : blocked ? (
                          <span className="text-[10px] opacity-60 ml-0.5">
                            (N/A)
                          </span>
                        ) : (
                          <Download
                            size={12}
                            className="shrink-0 text-muted-foreground"
                          />
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
                          if (!sourceInstance || !effectiveTarget) return;
                          const category =
                            hermesDeployCategory.trim() || "local";
                          setDeploying("hermes");
                          try {
                            await installToAgent(
                              sourceInstance.id,
                              "hermes",
                              effectiveTarget,
                              category,
                            );
                            flashInstalled("hermes");
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
          scope={scope}
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
                  if (!activePath) return null;
                  // Web mode can't open Finder, so the same slot offers
                  // Copy Path instead (same split as the Agents page rows).
                  return isDesktop() ? (
                    <button
                      onClick={() => api.revealInFileManager(activePath)}
                      className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                    >
                      <FolderOpen size={12} />
                      {t("detail.openInFinder")}
                    </button>
                  ) : (
                    <button
                      onClick={() => copyPathToClipboard(activePath)}
                      className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                    >
                      <Copy size={12} />
                      {ta("file.copyPath")}
                    </button>
                  );
                })()}
              </div>
              {/* Agent tabs for switching instance content — only the
               * active scope's instances (same projection as PATHS). */}
              {scopedInstances.length > 1 && (
                <div className="mb-2 flex flex-wrap gap-1">
                  {scopedInstances.map((instance) => (
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

              {/* File tree with expandable per-file previews */}
              {activeInstanceId && (
                <SkillFileSection
                  dirPath={instanceData.get(activeInstanceId)?.path ?? null}
                  loading={loadingContent}
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
            scope={scope}
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
