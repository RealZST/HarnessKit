import {
  Bot,
  FilePenLine,
  Lightbulb,
  Package,
  Puzzle,
  RefreshCw,
  Server,
  Shield,
  ShoppingBag,
  Terminal,
  Webhook,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { AgentCard } from "@/components/shared/agent-card";
import { api } from "@/lib/invoke";
import type { AgentDetail, DashboardStats } from "@/lib/types";
import {
  agentDisplayName,
  extensionGroupKey,
  formatRelativeTime,
  sortAgents,
} from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useAuditStore } from "@/stores/audit-store";
import { buildGroups, useExtensionStore } from "@/stores/extension-store";
import { toast } from "@/stores/toast-store";

// ---------------------------------------------------------------------------
// Tip of the Day types & helpers
// ---------------------------------------------------------------------------

interface Tip {
  agent: string;
  tip: string;
  source?: string;
}

const TIPS_URL =
  "https://raw.githubusercontent.com/RealZST/harnesskit-resources/main/tips/tips.json";
const TIPS_CACHE_KEY = "harnesskit-tips-cache";

async function fetchTips(): Promise<Tip[]> {
  try {
    const res = await fetch(TIPS_URL);
    if (!res.ok) throw new Error("fetch failed");
    const tips: Tip[] = await res.json();
    localStorage.setItem(TIPS_CACHE_KEY, JSON.stringify(tips));
    return tips;
  } catch {
    const cached = localStorage.getItem(TIPS_CACHE_KEY);
    if (cached) {
      try {
        return JSON.parse(cached) as Tip[];
      } catch {
        localStorage.removeItem(TIPS_CACHE_KEY);
      }
    }
    return [];
  }
}

// ---------------------------------------------------------------------------
// Recent Activity types
// ---------------------------------------------------------------------------

interface ActivityItem {
  type: "extension" | "config";
  kind?: string;
  label: string;
  sublabel: string;
  timestamp: number;
  /** Click handler that should setScope (so the destination page sees the
   *  right scope) BEFORE navigating. Overview is scope-agnostic, so deep
   *  links must carry their own scope context. */
  onSelect: () => void;
}

function formatTerminalCount(value: number) {
  return value >= 100 ? String(value) : String(value).padStart(2, "0");
}

// ---------------------------------------------------------------------------
// Small composable pieces
// ---------------------------------------------------------------------------

function StatChip({
  label,
  count,
  icon: Icon,
}: {
  label: string;
  count: number;
  icon: React.ElementType;
}) {
  return (
    <span className="inline-flex items-center gap-1.5 text-sm text-muted-foreground">
      <Icon
        size={14}
        strokeWidth={1.75}
        className="text-muted-foreground/60"
        aria-hidden="true"
      />
      <span className="tabular-nums font-medium text-foreground">{count}</span>
      <span>{label}</span>
    </span>
  );
}

function QuickAction({
  icon: Icon,
  label,
  sublabel,
  onClick,
  loading,
}: {
  icon: React.ElementType;
  label: string;
  sublabel: string;
  onClick: () => void;
  loading?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={loading}
      className="group flex items-center gap-3 rounded-lg border border-border/60 bg-card/50 px-4 py-3 text-left transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm disabled:opacity-70 disabled:pointer-events-none"
    >
      <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted/60 text-muted-foreground transition-colors duration-200 group-hover:bg-primary/10 group-hover:text-primary">
        <Icon
          size={17}
          strokeWidth={1.75}
          className={
            loading
              ? Icon === RefreshCw
                ? "origin-center animate-spin"
                : "animate-scanning"
              : ""
          }
        />
      </span>
      <div className="min-w-0">
        <span className="block text-sm font-medium text-foreground">
          {label}
        </span>
        <span className="block text-xs text-muted-foreground">{sublabel}</span>
      </div>
    </button>
  );
}

// ---------------------------------------------------------------------------
// Loading skeleton
// ---------------------------------------------------------------------------

function OverviewSkeleton() {
  return (
    <div className="space-y-10">
      {/* Header skeleton */}
      <div className="space-y-3">
        <div className="animate-shimmer h-10 w-48 rounded-lg bg-muted" />
        <div className="animate-shimmer h-5 w-80 rounded bg-muted" />
      </div>

      {/* Activity skeleton */}
      <div className="space-y-2">
        <div className="animate-shimmer h-4 w-32 rounded bg-muted" />
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="animate-shimmer h-14 rounded-lg bg-muted" />
        ))}
      </div>

      {/* Actions skeleton */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        {Array.from({ length: 3 }).map((_, i) => (
          <div key={i} className="animate-shimmer h-16 rounded-lg bg-muted" />
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

export default function OverviewPage() {
  const { t } = useTranslation("overview");
  const navigate = useNavigate();
  const extensions = useExtensionStore((s) => s.extensions);
  const extHasFetched = useExtensionStore((s) => s.hasFetched);
  const checkUpdates = useExtensionStore((s) => s.checkUpdates);
  const checkingUpdates = useExtensionStore((s) => s.checkingUpdates);
  const auditResults = useAuditStore((s) => s.results);
  const loadCached = useAuditStore((s) => s.loadCached);
  const runAudit = useAuditStore((s) => s.runAudit);
  const agents = useAgentStore((s) => s.agents);
  const fetchAgents = useAgentStore((s) => s.fetch);
  const agentOrder = useAgentStore((s) => s.agentOrder);

  const [agentConfigs, setAgentConfigs] = useState<AgentDetail[]>([]);
  const [auditLoading, setAuditLoading] = useState(false);
  // updatesLoading now comes from store as checkingUpdates
  const [tips, setTips] = useState<Tip[]>([]);
  const [localReady, setLocalReady] = useState(false);

  useEffect(() => {
    loadCached();
    // Wait for agents and config files before showing content
    Promise.all([
      fetchAgents(),
      api
        .listAgentConfigs()
        .then(setAgentConfigs)
        .catch((e) => {
          console.error("Failed to load data:", e);
        }),
    ]).then(() => setLocalReady(true));
    fetchTips()
      .then(setTips)
      .catch((e) => {
        console.error("Failed to load data:", e);
      });
  }, [loadCached, fetchAgents]);

  // Show skeleton until both extensions (fetched in App.tsx) and local data are ready.
  const initialLoaded = localReady && extHasFetched;

  // Filter extensions to only those belonging to enabled agents
  const enabledAgentNames = useMemo(
    () => new Set(agents.filter((a) => a.enabled).map((a) => a.name)),
    [agents],
  );
  const visibleExtensions = useMemo(
    () =>
      extensions.filter(
        (e) =>
          e.agents.length === 0 ||
          e.agents.some((a) => enabledAgentNames.has(a)),
      ),
    [extensions, enabledAgentNames],
  );

  // Group extensions so identical skills across agents count as one
  const visibleGroups = useMemo(
    () => buildGroups(visibleExtensions),
    [visibleExtensions],
  );

  // Dashboard stats — derived client-side from grouped extension data
  const stats = useMemo<DashboardStats | null>(() => {
    if (!initialLoaded) return null;

    const skill_count = visibleGroups.filter((g) => g.kind === "skill").length;
    const mcp_count = visibleGroups.filter((g) => g.kind === "mcp").length;
    const plugin_count = visibleGroups.filter(
      (g) => g.kind === "plugin",
    ).length;
    const hook_count = visibleGroups.filter((g) => g.kind === "hook").length;
    const cli_count = visibleGroups.filter((g) => g.kind === "cli").length;

    // Issue counts from audit
    let critical_issues = 0;
    let high_issues = 0;
    let medium_issues = 0;
    let low_issues = 0;
    for (const r of auditResults) {
      for (const f of r.findings) {
        switch (f.severity) {
          case "Critical":
            critical_issues++;
            break;
          case "High":
            high_issues++;
            break;
          case "Medium":
            medium_issues++;
            break;
          case "Low":
            low_issues++;
            break;
        }
      }
    }

    return {
      total_extensions: visibleGroups.length,
      skill_count,
      mcp_count,
      plugin_count,
      hook_count,
      cli_count,
      critical_issues,
      high_issues,
      medium_issues,
      low_issues,
      updates_available: 0,
    };
  }, [visibleGroups, auditResults, initialLoaded]);

  // Compute per-agent extension counts from grouped data
  const agentExtCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const g of visibleGroups) {
      for (const a of g.agents) {
        counts.set(a, (counts.get(a) ?? 0) + 1);
      }
    }
    return counts;
  }, [visibleGroups]);

  const enabledAgents = useMemo(
    () =>
      sortAgents(
        agents
          .filter((a) => a.enabled)
          .map((a) => ({
            ...a,
            extension_count: agentExtCounts.get(a.name) ?? 0,
          })),
        agentOrder,
      ),
    [agents, agentExtCounts, agentOrder],
  );

  // -----------------------------------------------------------------------
  // Section A: Recent Activity (agent config changes)
  // -----------------------------------------------------------------------
  const agentActivityItems = useMemo<ActivityItem[]>(() => {
    const items: ActivityItem[] = [];

    for (const agent of agentConfigs) {
      for (const cfg of agent.config_files) {
        if (!cfg.modified_at) continue;
        items.push({
          type: "config",
          label: cfg.file_name,
          sublabel: t("activity.configModified", {
            agent: agentDisplayName(agent.name),
            time: formatRelativeTime(cfg.modified_at),
          }),
          timestamp: new Date(cfg.modified_at).getTime(),
          // Pass the file's scope through the URL so Agents lands in the
          // right scope (Agents reads ?scope= and applies it locally). Doing
          // setScope + navigate in the same event handler races: React 18
          // batches both updates and the router update gets dropped.
          onSelect: () => {
            const scopeParam =
              cfg.scope.type === "global"
                ? ""
                : `&scope=${encodeURIComponent(cfg.scope.path)}`;
            navigate(
              `/agents?agent=${agent.name}&file=${encodeURIComponent(cfg.path)}${scopeParam}`,
            );
          },
        });
      }
    }

    items.sort((a, b) => b.timestamp - a.timestamp);
    return items.slice(0, 20);
  }, [agentConfigs, navigate, t]);

  // -----------------------------------------------------------------------
  // Section A-right: Recent Extensions (recently installed)
  // -----------------------------------------------------------------------
  const extensionActivityItems = useMemo<ActivityItem[]>(() => {
    const items: ActivityItem[] = [];

    // Only show types with accurate per-item install timestamps:
    // - skill: file creation time of SKILL.md
    // - plugin: plugin directory creation time
    // - cli: binary file creation time
    // MCP/Hook are excluded — their installed_at is the config FILE creation time,
    // not the time each individual entry was added.
    const accurateKinds = new Set(["skill", "plugin", "cli"]);
    const seenExtNames = new Set<string>();
    for (const ext of visibleExtensions) {
      if (!accurateKinds.has(ext.kind)) continue;
      if (seenExtNames.has(ext.name)) continue;
      seenExtNames.add(ext.name);
      items.push({
        type: "extension",
        kind: ext.kind,
        label: ext.name,
        sublabel: t("activity.extensionInstalled", {
          kind: ext.kind.toUpperCase(),
          time: formatRelativeTime(ext.installed_at),
        }),
        timestamp: new Date(ext.installed_at).getTime(),
        // Pass scope through the URL (see config-items comment above for why
        // setScope + navigate in the same handler races and loses the nav).
        onSelect: () => {
          const scopeParam =
            ext.scope.type === "global"
              ? ""
              : `&scope=${encodeURIComponent(ext.scope.path)}`;
          navigate(
            `/extensions?groupKey=${encodeURIComponent(extensionGroupKey(ext))}${scopeParam}`,
          );
        },
      });
    }

    items.sort((a, b) => b.timestamp - a.timestamp);
    return items.slice(0, 20);
  }, [visibleExtensions, navigate, t]);

  const hasActivity =
    agentActivityItems.length > 0 || extensionActivityItems.length > 0;

  // -----------------------------------------------------------------------
  // Section C: Tip of the Day
  // -----------------------------------------------------------------------
  const tipOfTheDay = useMemo(() => {
    if (tips.length === 0) return null;

    const detectedAgentNames = new Set(
      agents.filter((a) => a.detected).map((a) => a.name),
    );

    const relevant = tips.filter(
      (t) => t.agent === "general" || detectedAgentNames.has(t.agent),
    );
    if (relevant.length === 0) return null;

    const dayIndex = Math.floor(Date.now() / 86400000);
    return relevant[dayIndex % relevant.length];
  }, [tips, agents]);

  if (!stats) {
    return <OverviewSkeleton />;
  }

  const hasAuditData = auditResults.length > 0;

  return (
    <div className="space-y-6 pb-4" aria-live="polite">
      {/* ----------------------------------------------------------------- */}
      {/* Header — editorial greeting with inline stats                     */}
      {/* ----------------------------------------------------------------- */}
      <header className="space-y-2">
        {enabledAgents.length > 0 || stats.total_extensions > 0 ? (
          <div className="terminal-status select-none">
            <h2
              className="terminal-status__line"
              aria-label={t("terminal.ariaLabel", {
                agents: enabledAgents.length,
                extensions: stats.total_extensions,
              })}
            >
              <span className="terminal-status__command">
                <span className="terminal-status__prompt" aria-hidden="true">
                  &gt;
                </span>
                <span className="terminal-status__command-text">
                  {t("terminal.command")}
                </span>
              </span>
              <span className="terminal-status__output">
                <span className="terminal-status__metric">
                  <span className="terminal-status__count tabular-nums">
                    {formatTerminalCount(enabledAgents.length)}
                  </span>
                  <span className="terminal-status__label">
                    {t("terminal.agentLabel", { count: enabledAgents.length })}
                  </span>
                </span>
                <span className="terminal-status__separator" aria-hidden="true">
                  /
                </span>
                <span className="terminal-status__metric">
                  <span className="terminal-status__count tabular-nums">
                    {formatTerminalCount(stats.total_extensions)}
                  </span>
                  <span className="terminal-status__label">
                    {t("terminal.extensionLabel", {
                      count: stats.total_extensions,
                    })}
                  </span>
                </span>
              </span>
            </h2>
          </div>
        ) : (
          <h2 className="text-2xl font-bold tracking-tight text-foreground select-none">
            {t("welcome")}
          </h2>
        )}
        {stats.total_extensions > 0 ? (
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
            {stats.skill_count > 0 && (
              <StatChip
                label={t("stats.skill", { count: stats.skill_count })}
                count={stats.skill_count}
                icon={Package}
              />
            )}
            {stats.mcp_count > 0 && (
              <StatChip
                label={t("stats.mcpServer", { count: stats.mcp_count })}
                count={stats.mcp_count}
                icon={Server}
              />
            )}
            {stats.plugin_count > 0 && (
              <StatChip
                label={t("stats.plugin", { count: stats.plugin_count })}
                count={stats.plugin_count}
                icon={Puzzle}
              />
            )}
            {stats.hook_count > 0 && (
              <StatChip
                label={t("stats.hook", { count: stats.hook_count })}
                count={stats.hook_count}
                icon={Webhook}
              />
            )}
            {stats.cli_count > 0 && (
              <StatChip
                label={t("stats.cli", { count: stats.cli_count })}
                count={stats.cli_count}
                icon={Terminal}
              />
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">{t("emptyHint")}</p>
        )}
        {/* Agent mascot cards */}
        {enabledAgents.length > 0 && (
          <div className="flex flex-wrap gap-3 pt-3">
            {enabledAgents.map((agent) => (
              <AgentCard key={agent.name} agent={agent} />
            ))}
          </div>
        )}
      </header>

      {/* ----------------------------------------------------------------- */}
      {/* Tip of the Day — full-width banner                                */}
      {/* ----------------------------------------------------------------- */}
      {tipOfTheDay && (
        <section className="space-y-3">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {t("tipOfDay.title")}
          </h3>
          <div className="flex items-center gap-3 rounded-xl border border-accent-foreground/10 bg-accent/60 px-4 py-3">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Lightbulb size={15} strokeWidth={1.75} aria-hidden="true" />
            </span>
            <p className="min-w-0 flex-1 text-sm text-foreground leading-relaxed">
              {tipOfTheDay.tip}
              {tipOfTheDay.source ? (
                <a
                  href={tipOfTheDay.source}
                  target="_blank"
                  rel="noopener noreferrer"
                  title={tipOfTheDay.source}
                  className="ml-2 inline-block translate-y-[-1px] cursor-pointer rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary transition-colors hover:bg-primary/20 hover:underline"
                >
                  {tipOfTheDay.agent === "general"
                    ? t("tipOfDay.general")
                    : agentDisplayName(tipOfTheDay.agent)}
                </a>
              ) : (
                <span className="ml-2 inline-block translate-y-[-1px] rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                  {tipOfTheDay.agent === "general"
                    ? t("tipOfDay.general")
                    : agentDisplayName(tipOfTheDay.agent)}
                </span>
              )}
            </p>
          </div>
        </section>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* 2-column grid: Agent Activity | Recently Installed                */}
      {/* ----------------------------------------------------------------- */}
      {hasActivity && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {/* Recent Activity (agent config changes) */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("activity.agentActivity")}
            </h3>
            <div className="rounded-xl border border-border/60 bg-card/40 divide-y divide-border/40 max-h-[10.5rem] overflow-y-auto overscroll-contain">
              {agentActivityItems.length > 0 ? (
                agentActivityItems.map((item, i) => (
                  <button
                    key={`${item.type}-${item.label}-${i}`}
                    onClick={item.onSelect}
                    className="flex w-full items-center gap-2.5 px-3 py-2.5 text-left transition-colors hover:bg-muted/30"
                  >
                    <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
                      <FilePenLine
                        size={13}
                        strokeWidth={1.75}
                        aria-hidden="true"
                      />
                    </span>
                    <div className="min-w-0 flex-1">
                      <span className="truncate text-sm font-medium text-foreground block">
                        {item.label}
                      </span>
                      <span className="truncate text-xs text-muted-foreground block">
                        {item.sublabel}
                      </span>
                    </div>
                  </button>
                ))
              ) : (
                <div className="flex items-center justify-center px-3 py-6 text-xs text-muted-foreground">
                  {t("activity.noConfigChanges")}
                </div>
              )}
            </div>
          </section>

          {/* Recent Extensions */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              {t("activity.recentlyInstalled")}
            </h3>
            <div className="rounded-xl border border-border/60 bg-card/40 divide-y divide-border/40 max-h-[10.5rem] overflow-y-auto overscroll-contain">
              {extensionActivityItems.length > 0 ? (
                extensionActivityItems.map((item, i) => (
                  <button
                    key={`${item.type}-${item.label}-${i}`}
                    onClick={item.onSelect}
                    className="flex w-full items-center gap-2.5 px-3 py-2.5 text-left transition-colors hover:bg-muted/30"
                  >
                    <span className="flex size-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
                      {(() => {
                        const Icon =
                          item.kind === "plugin"
                            ? Puzzle
                            : item.kind === "cli"
                              ? Terminal
                              : Package;
                        return (
                          <Icon
                            size={13}
                            strokeWidth={1.75}
                            aria-hidden="true"
                          />
                        );
                      })()}
                    </span>
                    <div className="min-w-0 flex-1">
                      <span className="truncate text-sm font-medium text-foreground block">
                        {item.label}
                      </span>
                      <span className="truncate text-xs text-muted-foreground block">
                        {item.sublabel}
                      </span>
                    </div>
                  </button>
                ))
              ) : (
                <div className="flex items-center justify-center px-3 py-6 text-xs text-muted-foreground">
                  {t("activity.noInstallations")}
                </div>
              )}
            </div>
          </section>
        </div>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* First-run welcome — when no extensions and no audit               */}
      {/* ----------------------------------------------------------------- */}
      {stats.total_extensions === 0 && !hasAuditData && (
        <section className="space-y-5">
          <h3 className="font-serif text-xl font-semibold tracking-tight text-foreground">
            {t("firstRun.title")}
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {(
              [
                {
                  icon: Bot,
                  label: t("firstRun.viewExtensions.label"),
                  description: t("firstRun.viewExtensions.description"),
                  to: "/extensions",
                  delay: "0ms",
                },
                {
                  icon: ShoppingBag,
                  label: t("firstRun.browseMarketplace.label"),
                  description: t("firstRun.browseMarketplace.description"),
                  to: "/marketplace",
                  delay: "60ms",
                },
                {
                  icon: Shield,
                  label: t("firstRun.runAudit.label"),
                  description: t("firstRun.runAudit.description"),
                  to: "/audit",
                  delay: "120ms",
                },
              ] as const
            ).map((card) => (
              <button
                key={card.to}
                onClick={() => navigate(card.to)}
                className="animate-fade-in group flex flex-col items-start gap-3 rounded-xl border border-border/60 bg-card/50 p-5 text-left transition-all duration-200 hover:shadow-md"
                style={{ animationDelay: card.delay }}
              >
                <span className="flex size-10 items-center justify-center rounded-lg bg-muted/60 text-muted-foreground transition-colors duration-200 group-hover:bg-primary/10 group-hover:text-primary">
                  <card.icon size={20} strokeWidth={1.75} />
                </span>
                <div>
                  <span className="block text-sm font-medium text-foreground">
                    {card.label}
                  </span>
                  <span className="mt-1 block text-xs text-muted-foreground">
                    {card.description}
                  </span>
                </div>
              </button>
            ))}
          </div>
        </section>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* Empty state — when no extensions at all                           */}
      {/* ----------------------------------------------------------------- */}
      {stats.total_extensions === 0 && (
        <section className="animate-scale-in rounded-xl border border-dashed border-border bg-card/30 px-6 py-6 text-center">
          <Package
            size={24}
            className="mx-auto text-muted-foreground/40"
            aria-hidden="true"
          />
          <h3 className="mt-2 text-sm font-medium text-foreground">
            {t("empty.title")}
          </h3>
          <p className="mt-1 text-xs text-muted-foreground">
            {t("empty.description")}
          </p>
          <div className="mt-3 flex items-center justify-center gap-3">
            <button
              onClick={() => navigate("/marketplace")}
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors duration-150 hover:bg-primary/90"
            >
              <ShoppingBag size={14} />
              {t("empty.cta")}
            </button>
          </div>
        </section>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* Quick actions                                                      */}
      {/* ----------------------------------------------------------------- */}
      {stats.total_extensions > 0 && (
        <section className="space-y-3">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {t("quickActions.title")}
          </h3>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <QuickAction
              icon={Bot}
              label={t("quickActions.viewAgents.label")}
              sublabel={t("quickActions.viewAgents.sublabel")}
              onClick={() => navigate("/agents")}
            />
            <QuickAction
              icon={Shield}
              label={t("quickActions.runAudit.label")}
              sublabel={t("quickActions.runAudit.sublabel")}
              loading={auditLoading}
              onClick={() => {
                setAuditLoading(true);
                runAudit().finally(() => setAuditLoading(false));
              }}
            />
            <QuickAction
              icon={RefreshCw}
              label={t("quickActions.checkUpdates.label")}
              sublabel={t("quickActions.checkUpdates.sublabel")}
              loading={checkingUpdates}
              onClick={() => {
                checkUpdates().then(() => {
                  const state = useExtensionStore.getState();
                  const statuses = state.updateStatuses;
                  const count = state
                    .grouped()
                    .filter((g) =>
                      g.instances.some(
                        (inst) =>
                          statuses.get(inst.id)?.status === "update_available",
                      ),
                    ).length;
                  toast.success(
                    count > 0
                      ? t("updates.available", { count })
                      : t("updates.none"),
                  );
                });
              }}
            />
            <QuickAction
              icon={ShoppingBag}
              label={t("quickActions.marketplace.label")}
              sublabel={t("quickActions.marketplace.sublabel")}
              onClick={() => navigate("/marketplace")}
            />
          </div>
        </section>
      )}
    </div>
  );
}
