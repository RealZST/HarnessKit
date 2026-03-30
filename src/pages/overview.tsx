import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useExtensionStore } from "@/stores/extension-store";
import { api } from "@/lib/invoke";
import { useAuditStore } from "@/stores/audit-store";
import { useAgentStore } from "@/stores/agent-store";
import {
  Package,
  Server,
  Puzzle,
  Webhook,
  Shield,
  ShoppingBag,
  Bot,
  RefreshCw,
  Clock,
  Sparkles,
  FilePenLine,
  TrendingUp,
  Lightbulb,
  BarChart3,
} from "lucide-react";
import { Hint } from "@/components/shared/hint";
import type { DashboardStats, Extension, AgentDetail } from "@/lib/types";
import { agentDisplayName, formatRelativeTime, sortAgents } from "@/lib/types";
import { buildGroups } from "@/stores/extension-store";
import { AgentCard } from "@/components/shared/agent-card";

// ---------------------------------------------------------------------------
// Tip of the Day types & helpers
// ---------------------------------------------------------------------------

interface Tip {
  agent: string;
  tip: string;
}

const TIPS_URL =
  "https://raw.githubusercontent.com/RealZST/harnesskit-tips/main/tips.json";
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
    if (cached) return JSON.parse(cached) as Tip[];
    return [];
  }
}

// ---------------------------------------------------------------------------
// Recent Activity types
// ---------------------------------------------------------------------------

interface ActivityItem {
  type: "extension" | "config";
  label: string;
  sublabel: string;
  timestamp: number;
  navigateTo: string;
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
      <Icon size={14} strokeWidth={1.75} className="text-muted-foreground/60" aria-hidden="true" />
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
}: {
  icon: React.ElementType;
  label: string;
  sublabel: string;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className="group flex items-center gap-3 rounded-lg border border-border/60 bg-card/50 px-4 py-3 text-left transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm hover:scale-[1.01]"
    >
      <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-muted/60 text-muted-foreground transition-colors duration-200 group-hover:bg-primary/10 group-hover:text-primary">
        <Icon size={17} strokeWidth={1.75} />
      </span>
      <div className="min-w-0">
        <span className="block text-sm font-medium text-foreground">{label}</span>
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
    <div className="space-y-10" aria-live="polite">
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
  const navigate = useNavigate();
  const [extensions, setExtensions] = useState<Extension[]>([]);
  const [extLoading, setExtLoading] = useState(true);
  const checkUpdates = useExtensionStore(s => s.checkUpdates);
  const auditResults = useAuditStore(s => s.results);
  const loadCached = useAuditStore(s => s.loadCached);
  const runAudit = useAuditStore(s => s.runAudit);
  const agents = useAgentStore(s => s.agents);
  const fetchAgents = useAgentStore(s => s.fetch);
  const agentOrder = useAgentStore(s => s.agentOrder);

  const [agentConfigs, setAgentConfigs] = useState<AgentDetail[]>([]);
  const [tips, setTips] = useState<Tip[]>([]);

  useEffect(() => {
    // Fetch ALL extensions (unfiltered) for overview stats
    api.listExtensions()
      .then(setExtensions)
      .catch(() => {})
      .finally(() => setExtLoading(false));
    loadCached();
    fetchAgents();
    api.listAgentConfigs().then(setAgentConfigs).catch(() => {});
    fetchTips().then(setTips);
  }, [loadCached, fetchAgents]);

  // Filter extensions to only those belonging to enabled agents
  const enabledAgentNames = useMemo(
    () => new Set(agents.filter((a) => a.enabled).map((a) => a.name)),
    [agents],
  );
  const visibleExtensions = useMemo(
    () => extensions.filter((e) => e.agents.some((a) => enabledAgentNames.has(a))),
    [extensions, enabledAgentNames],
  );

  // Group extensions so identical skills across agents count as one
  const visibleGroups = useMemo(
    () => buildGroups(visibleExtensions),
    [visibleExtensions],
  );

  // Dashboard stats — derived client-side from grouped extension data
  const stats = useMemo<DashboardStats | null>(() => {
    if (extLoading && extensions.length === 0) return null;

    const skill_count = visibleGroups.filter((g) => g.kind === "skill").length;
    const mcp_count = visibleGroups.filter((g) => g.kind === "mcp").length;
    const plugin_count = visibleGroups.filter((g) => g.kind === "plugin").length;
    const hook_count = visibleGroups.filter((g) => g.kind === "hook").length;

    // Issue counts from audit
    let critical_issues = 0;
    let high_issues = 0;
    let medium_issues = 0;
    let low_issues = 0;
    for (const r of auditResults) {
      for (const f of r.findings) {
        switch (f.severity) {
          case "Critical": critical_issues++; break;
          case "High": high_issues++; break;
          case "Medium": medium_issues++; break;
          case "Low": low_issues++; break;
        }
      }
    }

    return {
      total_extensions: visibleGroups.length,
      skill_count,
      mcp_count,
      plugin_count,
      hook_count,
      critical_issues,
      high_issues,
      medium_issues,
      low_issues,
      updates_available: 0,
    };
  }, [visibleGroups, auditResults, extLoading, extensions.length]);

  const enabledAgentCount = useMemo(() => agents.filter((a) => a.enabled).length, [agents]);

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
    () => sortAgents(
      agents
        .filter((a) => a.enabled)
        .map((a) => ({ ...a, extension_count: agentExtCounts.get(a.name) ?? 0 })),
      agentOrder,
    ),
    [agents, agentExtCounts, agentOrder],
  );

  // -----------------------------------------------------------------------
  // Section A: Recent Activity
  // -----------------------------------------------------------------------
  const activityItems = useMemo<ActivityItem[]>(() => {
    const items: ActivityItem[] = [];
    const now = Date.now();
    const fourteenDays = 14 * 24 * 60 * 60 * 1000;
    const sevenDays = 7 * 24 * 60 * 60 * 1000;

    // Recently installed extensions (within last 14 days), deduplicated by name
    const seenExtNames = new Set<string>();
    for (const ext of visibleExtensions) {
      if (seenExtNames.has(ext.name)) continue;
      const installedMs = now - new Date(ext.installed_at).getTime();
      if (installedMs < fourteenDays) {
        seenExtNames.add(ext.name);
        items.push({
          type: "extension",
          label: ext.name,
          sublabel: `Installed ${formatRelativeTime(ext.installed_at)}`,
          timestamp: new Date(ext.installed_at).getTime(),
          navigateTo: "/extensions",
        });
      }
    }

    // Recently modified config files (within last 7 days)
    for (const agent of agentConfigs) {
      for (const cfg of agent.config_files) {
        if (!cfg.modified_at) continue;
        const modifiedMs = now - new Date(cfg.modified_at).getTime();
        if (modifiedMs < sevenDays) {
          items.push({
            type: "config",
            label: cfg.file_name,
            sublabel: `${agentDisplayName(agent.name)} \u00B7 Modified ${formatRelativeTime(cfg.modified_at)}`,
            timestamp: new Date(cfg.modified_at).getTime(),
            navigateTo: "/agents",
          });
        }
      }
    }

    // Sort newest first, limit to 3
    items.sort((a, b) => b.timestamp - a.timestamp);
    return items.slice(0, 3);
  }, [visibleExtensions, agentConfigs]);

  const hasActivity = activityItems.length > 0;

  // -----------------------------------------------------------------------
  // Section B: Usage Insights
  // -----------------------------------------------------------------------
  const usageInsights = useMemo(() => {
    const allSkills = visibleExtensions.filter((e) => e.kind === "skill");
    const usedSkills = allSkills.filter((e) => e.last_used_at);
    if (usedSkills.length === 0) return null;

    // Most active = most recent last_used_at
    const sorted = [...usedSkills].sort(
      (a, b) => new Date(b.last_used_at!).getTime() - new Date(a.last_used_at!).getTime(),
    );
    const mostActive = sorted[0];

    // Longest unused
    const neverUsed = allSkills.filter((e) => !e.last_used_at);
    let longestUnused: { name: string; detail: string };
    if (neverUsed.length > 0) {
      longestUnused = { name: neverUsed[0].name, detail: "Never used" };
    } else {
      const oldest = sorted[sorted.length - 1];
      longestUnused = {
        name: oldest.name,
        detail: `Unused for ${formatRelativeTime(oldest.last_used_at!).replace(" ago", "")}`,
      };
    }

    // Recently used count (within 7 days)
    const sevenDays = 7 * 24 * 60 * 60 * 1000;
    const recentlyUsedCount = usedSkills.filter(
      (e) => Date.now() - new Date(e.last_used_at!).getTime() < sevenDays,
    ).length;

    return {
      mostActive: {
        name: mostActive.name,
        detail: `Used ${formatRelativeTime(mostActive.last_used_at!)}`,
      },
      longestUnused,
      recentlyUsedCount,
      totalSkills: allSkills.length,
    };
  }, [visibleExtensions]);

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
    <div className="animate-fade-in space-y-6 pb-4">
      {/* ----------------------------------------------------------------- */}
      {/* Header — editorial greeting with inline stats                     */}
      {/* ----------------------------------------------------------------- */}
      <header className="space-y-1.5">
        <h2 className="font-serif text-3xl font-bold tracking-tight text-foreground select-none">
          {stats.total_extensions === 0 && enabledAgentCount === 0 ? (
            (() => {
              const h = new Date().getHours();
              const greeting = h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
              return `${greeting} — Welcome to HarnessKit`;
            })()
          ) : (
            <>
              {enabledAgentCount > 0 && `${enabledAgentCount} agent${enabledAgentCount !== 1 ? "s" : ""}`}
              {enabledAgentCount > 0 && stats.total_extensions > 0 && (
                <span className="mx-3 text-muted-foreground/40">·</span>
              )}
              {stats.total_extensions > 0 && `${stats.total_extensions} extension${stats.total_extensions !== 1 ? "s" : ""}`}
            </>
          )}
        </h2>
        {stats.total_extensions > 0 ? (
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
            {stats.skill_count > 0 && (
              <StatChip label="skills" count={stats.skill_count} icon={Package} />
            )}
            {stats.mcp_count > 0 && (
              <StatChip label="MCP servers" count={stats.mcp_count} icon={Server} />
            )}
            {stats.plugin_count > 0 && (
              <StatChip label="plugins" count={stats.plugin_count} icon={Puzzle} />
            )}
            {stats.hook_count > 0 && (
              <StatChip label="hooks" count={stats.hook_count} icon={Webhook} />
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            Get started by browsing the marketplace or running a scan.
          </p>
        )}
        {/* Agent mascot cards */}
        {enabledAgents.length > 0 && (
          <div className="flex flex-wrap gap-3 pt-3">
            {enabledAgents.map((agent) => (
              <AgentCard
                key={agent.name}
                agent={agent}
              />
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
            Tip of the day
          </h3>
          <div className="flex items-center gap-3 rounded-xl border border-accent-foreground/10 bg-accent/60 px-4 py-3">
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Lightbulb size={15} strokeWidth={1.75} aria-hidden="true" />
            </span>
            <p className="min-w-0 flex-1 text-sm text-foreground leading-relaxed">
              {tipOfTheDay.tip}
              <span className="ml-2 inline-block translate-y-[-1px] rounded-full bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                {tipOfTheDay.agent === "general" ? "General" : agentDisplayName(tipOfTheDay.agent)}
              </span>
            </p>
          </div>
        </section>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* 2-column grid: Activity | Usage Insights                          */}
      {/* ----------------------------------------------------------------- */}
      {(hasActivity || usageInsights) && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
          {/* Recent Activity */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Recent activity
            </h3>
            <div className="rounded-xl border border-border/60 bg-card/40 divide-y divide-border/40">
              {hasActivity ? (
                activityItems.map((item, i) => (
                  <div
                    key={`${item.type}-${item.label}-${i}`}
                    className="flex items-center gap-2.5 px-3 py-2.5"
                  >
                    <span
                      className={`flex size-7 shrink-0 items-center justify-center rounded-md ${
                        item.type === "extension"
                          ? "bg-primary/8 text-primary"
                          : "bg-muted/80 text-muted-foreground"
                      }`}
                    >
                      {item.type === "extension" ? (
                        <Sparkles size={13} aria-hidden="true" />
                      ) : (
                        <FilePenLine size={13} aria-hidden="true" />
                      )}
                    </span>
                    <div className="min-w-0 flex-1">
                      <span className="truncate text-sm font-medium text-foreground block">{item.label}</span>
                      <span className="truncate text-xs text-muted-foreground block">{item.sublabel}</span>
                    </div>
                  </div>
                ))
              ) : (
                <div className="flex items-center justify-center px-3 py-6 text-xs text-muted-foreground">
                  No recent changes
                </div>
              )}
            </div>
          </section>

          {/* Usage Insights */}
          <section className="space-y-3">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Usage insights
            </h3>
            {usageInsights ? (
              <div className="rounded-xl border border-border/60 bg-card/40 divide-y divide-border/40">
                <div className="flex items-center gap-2.5 px-3 py-2.5">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                    <TrendingUp size={13} strokeWidth={1.75} aria-hidden="true" />
                  </span>
                  <div className="min-w-0">
                    <span className="block text-sm font-medium text-foreground truncate">{usageInsights.mostActive.name}</span>
                    <span className="block text-xs text-muted-foreground truncate">Most active · {usageInsights.mostActive.detail}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2.5 px-3 py-2.5">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-muted/60 text-muted-foreground">
                    <Clock size={13} strokeWidth={1.75} aria-hidden="true" />
                  </span>
                  <div className="min-w-0">
                    <span className="block text-sm font-medium text-foreground truncate">{usageInsights.longestUnused.name}</span>
                    <span className="block text-xs text-muted-foreground truncate">Longest unused · {usageInsights.longestUnused.detail}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2.5 px-3 py-2.5">
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                    <BarChart3 size={13} strokeWidth={1.75} aria-hidden="true" />
                  </span>
                  <div className="min-w-0">
                    <span className="block text-sm font-medium text-foreground">{usageInsights.recentlyUsedCount} of {usageInsights.totalSkills} skills</span>
                    <span className="block text-xs text-muted-foreground">Used in the last 7 days</span>
                  </div>
                </div>
              </div>
            ) : (
              <div className="rounded-xl border border-border/60 bg-card/40 flex items-center justify-center px-3 py-6 text-xs text-muted-foreground">
                No usage data yet
              </div>
            )}
          </section>
        </div>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* First-run hint: audit                                              */}
      {/* ----------------------------------------------------------------- */}
      {!hasAuditData && stats.total_extensions > 0 && (
        <Hint id="overview-audit">
          Run a security audit to check your extensions for vulnerabilities and
          get trust scores. Use the Quick Actions below to get started.
        </Hint>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* First-run welcome — when no extensions and no audit               */}
      {/* ----------------------------------------------------------------- */}
      {stats.total_extensions === 0 && !hasAuditData && (
        <section className="space-y-5">
          <h3 className="font-serif text-xl font-semibold tracking-tight text-foreground">
            One place for all your extensions
          </h3>
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            {([
              { icon: Bot, label: "View extensions", description: "Browse and manage extensions across your coding agents", to: "/extensions", delay: "0ms" },
              { icon: ShoppingBag, label: "Browse marketplace", description: "Discover and install skills, MCP servers, and plugins", to: "/marketplace", delay: "60ms" },
              { icon: Shield, label: "Run audit", description: "Check your extensions for security issues", to: "/audit", delay: "120ms" },
            ] as const).map((card) => (
              <button
                key={card.to}
                onClick={() => navigate(card.to)}
                className="animate-fade-in group flex flex-col items-start gap-3 rounded-xl border border-border/60 bg-card/50 p-5 text-left transition-all duration-200 hover:shadow-md hover:scale-[1.01]"
                style={{ animationDelay: card.delay }}
              >
                <span className="flex size-10 items-center justify-center rounded-lg bg-muted/60 text-muted-foreground transition-colors duration-200 group-hover:bg-primary/10 group-hover:text-primary">
                  <card.icon size={20} strokeWidth={1.75} />
                </span>
                <div>
                  <span className="block text-sm font-medium text-foreground">{card.label}</span>
                  <span className="mt-1 block text-xs text-muted-foreground">{card.description}</span>
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
        <section className="animate-scale-in rounded-xl border border-dashed border-border bg-card/30 px-6 py-10 text-center">
          <Package size={32} className="mx-auto text-muted-foreground/40" aria-hidden="true" />
          <h3 className="mt-3 text-base font-medium text-foreground">
            Your workspace is ready
          </h3>
          <p className="mt-1 text-sm text-muted-foreground">
            Browse the marketplace to discover skills, MCP servers, and plugins for your agents.
          </p>
          <div className="mt-5 flex items-center justify-center gap-3">
            <button
              onClick={() => navigate("/marketplace")}
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors duration-150 hover:bg-primary/90"
            >
              <ShoppingBag size={14} />
              Browse marketplace
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
            Quick actions
          </h3>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            <QuickAction
              icon={Shield}
              label="Run Audit"
              sublabel={
                hasAuditData
                  ? `Last run ${formatRelativeTime(auditResults.reduce((latest, r) => r.audited_at > latest ? r.audited_at : latest, auditResults[0].audited_at))}`
                  : "Scan for security issues"
              }
              onClick={() => {
                runAudit();
                navigate("/audit");
              }}
            />
            <QuickAction
              icon={RefreshCw}
              label="Check Updates"
              sublabel="Check for new versions"
              onClick={() => {
                checkUpdates();
                navigate("/extensions");
              }}
            />
            <QuickAction
              icon={ShoppingBag}
              label="Marketplace"
              sublabel="Discover skills and MCP"
              onClick={() => navigate("/marketplace")}
            />
            <QuickAction
              icon={Bot}
              label="View Agents"
              sublabel="Manage agent configs"
              onClick={() => navigate("/agents")}
            />
          </div>
        </section>
      )}

    </div>
  );
}
