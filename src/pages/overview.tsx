import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useExtensionStore } from "@/stores/extension-store";
import { useAuditStore } from "@/stores/audit-store";
import {
  Package,
  Server,
  Puzzle,
  Webhook,
  Shield,
  ShoppingBag,
  RefreshCw,
  ArrowRight,
  AlertTriangle,
  Clock,
  Sparkles,
} from "lucide-react";
import { TrustBadge } from "@/components/shared/trust-badge";
import { KindBadge } from "@/components/shared/kind-badge";
import { Hint } from "@/components/shared/hint";
import type { DashboardStats, Extension, AuditResult } from "@/lib/types";
import { trustTier, formatRelativeTime } from "@/lib/types";

// ---------------------------------------------------------------------------
// Needs-attention logic
// ---------------------------------------------------------------------------

interface AttentionItem {
  extension: Extension;
  reason: "low_trust" | "recently_added";
  detail: string;
  /** Lower = more urgent */
  priority: number;
}

function deriveAttentionItems(
  extensions: Extension[],
  auditResults: AuditResult[],
): AttentionItem[] {
  const auditMap = new Map<string, AuditResult>();
  for (const r of auditResults) {
    auditMap.set(r.extension_id, r);
  }

  const items: AttentionItem[] = [];

  for (const ext of extensions) {
    const audit = auditMap.get(ext.id);

    // Low trust score — anything below 60 is noteworthy
    if (audit && audit.trust_score < 60) {
      const tier = trustTier(audit.trust_score);
      items.push({
        extension: ext,
        reason: "low_trust",
        detail:
          tier === "Critical"
            ? `Trust score ${audit.trust_score} — critical findings`
            : `Trust score ${audit.trust_score} — needs review`,
        priority: audit.trust_score,
      });
      continue; // Don't double-list
    }

    // Recently added (within last 7 days)
    const installedMs = Date.now() - new Date(ext.installed_at).getTime();
    const sevenDays = 7 * 24 * 60 * 60 * 1000;
    if (installedMs < sevenDays) {
      items.push({
        extension: ext,
        reason: "recently_added",
        detail: `Added ${formatRelativeTime(ext.installed_at)}`,
        priority: 200 - Math.floor(installedMs / (60 * 60 * 1000)), // newer = higher priority
      });
    }
  }

  // Sort: most urgent first
  items.sort((a, b) => a.priority - b.priority);
  return items.slice(0, 6);
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

function AttentionRow({ item, onClick }: { item: AttentionItem; onClick: () => void }) {
  const isLowTrust = item.reason === "low_trust";

  return (
    <button
      onClick={onClick}
      className="group flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors duration-150 hover:bg-muted/50"
    >
      {/* Status indicator */}
      <span
        className={`flex size-8 shrink-0 items-center justify-center rounded-lg ${
          isLowTrust
            ? "bg-destructive/8 text-destructive"
            : "bg-primary/8 text-primary"
        }`}
      >
        {isLowTrust ? <AlertTriangle size={15} aria-hidden="true" /> : <Sparkles size={15} aria-hidden="true" />}
      </span>

      {/* Extension info */}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-foreground">
            {item.extension.name}
          </span>
          <KindBadge kind={item.extension.kind} />
        </div>
        <p className="mt-0.5 truncate text-xs text-muted-foreground">{item.detail}</p>
      </div>

      {/* Trust badge or timestamp */}
      <div className="shrink-0">
        {isLowTrust && item.extension.trust_score != null ? (
          <TrustBadge score={item.extension.trust_score} size="sm" />
        ) : (
          <span className="text-xs text-muted-foreground">
            {formatRelativeTime(item.extension.installed_at)}
          </span>
        )}
      </div>

      <ArrowRight
        size={14}
        className="shrink-0 text-muted-foreground/40 transition-transform duration-150 group-hover:translate-x-0.5 group-hover:text-muted-foreground"
        aria-hidden="true"
      />
    </button>
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
      className="group flex items-center gap-3 rounded-lg border border-border/60 bg-card/50 px-4 py-3 text-left transition-all duration-200 hover:border-border hover:bg-card hover:shadow-sm"
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

      {/* Attention skeleton */}
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
  const extensions = useExtensionStore(s => s.extensions);
  const fetchExtensions = useExtensionStore(s => s.fetch);
  const extLoading = useExtensionStore(s => s.loading);
  const checkUpdates = useExtensionStore(s => s.checkUpdates);
  const auditResults = useAuditStore(s => s.results);
  const loadCached = useAuditStore(s => s.loadCached);
  const runAudit = useAuditStore(s => s.runAudit);

  useEffect(() => {
    fetchExtensions();
    loadCached();
  }, [fetchExtensions, loadCached]);

  // Dashboard stats — derived client-side from extension data
  const stats = useMemo<DashboardStats | null>(() => {
    if (extLoading && extensions.length === 0) return null;

    const skill_count = extensions.filter((e) => e.kind === "skill").length;
    const mcp_count = extensions.filter((e) => e.kind === "mcp").length;
    const plugin_count = extensions.filter((e) => e.kind === "plugin").length;
    const hook_count = extensions.filter((e) => e.kind === "hook").length;

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
      total_extensions: extensions.length,
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
  }, [extensions, auditResults, extLoading]);

  const attentionItems = useMemo(
    () => deriveAttentionItems(extensions, auditResults),
    [extensions, auditResults],
  );

  const issueCount = stats
    ? stats.critical_issues + stats.high_issues
    : 0;

  // Track previous issue count to trigger "all clear" pulse only on transition
  const prevIssueCountRef = useRef<number | null>(null);
  const prevHasAuditDataRef = useRef(false);
  const [showPulse, setShowPulse] = useState(false);

  const prefersReducedMotion = () =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  useEffect(() => {
    const hasAudit = auditResults.length > 0;
    const prevCount = prevIssueCountRef.current;
    const prevHadAudit = prevHasAuditDataRef.current;

    // Trigger pulse when:
    // 1. Transitioning from issues > 0 to issues === 0
    // 2. First audit completion with no issues (prevCount was null, now 0)
    const justCleared =
      hasAudit &&
      issueCount === 0 &&
      ((prevCount !== null && prevCount > 0) ||
        (prevCount === null && !prevHadAudit));

    if (justCleared && !prefersReducedMotion()) {
      setShowPulse(true);
      const timer = setTimeout(() => setShowPulse(false), 800);
      return () => clearTimeout(timer);
    }

    prevIssueCountRef.current = issueCount;
    prevHasAuditDataRef.current = hasAudit;
  }, [issueCount, auditResults.length]);

  if (!stats) {
    return <OverviewSkeleton />;
  }

  const hasAttention = attentionItems.length > 0;
  const hasAuditData = auditResults.length > 0;

  return (
    <div className="animate-fade-in space-y-10">
      {/* ----------------------------------------------------------------- */}
      {/* Header — editorial greeting with inline stats                     */}
      {/* ----------------------------------------------------------------- */}
      <header className="space-y-1.5">
        <h2 className="font-serif text-3xl font-bold tracking-tight text-foreground">
          {stats.total_extensions === 0
            ? "Welcome to HarnessKit"
            : `${stats.total_extensions} extensions`}
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

            {/* Audit summary inline, if data exists */}
            {hasAuditData && issueCount > 0 && (
              <>
                <span className="text-border">|</span>
                <span className="inline-flex items-center gap-1.5 text-sm">
                  <AlertTriangle size={13} className="text-chart-5" aria-hidden="true" />
                  <span className="text-muted-foreground">
                    {issueCount} {issueCount === 1 ? "issue" : "issues"} found
                  </span>
                </span>
              </>
            )}
            {hasAuditData && issueCount === 0 && (
              <>
                <span className="text-border">|</span>
                <span className="inline-flex items-center gap-1.5 text-sm text-primary">
                  <span className={`inline-flex ${showPulse ? "all-clear-pulse" : ""}`}>
                    <Shield size={13} aria-hidden="true" />
                  </span>
                  All clear
                </span>
              </>
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            Get started by browsing the marketplace or running a scan.
          </p>
        )}
      </header>

      {/* ----------------------------------------------------------------- */}
      {/* First-run hint: audit                                              */}
      {/* ----------------------------------------------------------------- */}
      {!hasAuditData && stats.total_extensions > 0 && (
        <Hint id="overview-audit">
          Run a security audit to check your extensions for vulnerabilities and
          get trust scores. Use the Quick Actions below or press ⌘4.
        </Hint>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* Needs Attention                                                    */}
      {/* ----------------------------------------------------------------- */}
      {hasAttention && (
        <section className="animate-slide-in-right space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
              Needs attention
            </h3>
            {attentionItems.some((i) => i.reason === "low_trust") && (
              <button
                onClick={() => navigate("/audit")}
                className="text-xs font-medium text-primary hover:underline"
              >
                View audit
              </button>
            )}
          </div>

          <div className="rounded-xl border border-border/60 bg-card/40 divide-y divide-border/40">
            {attentionItems.map((item) => (
              <AttentionRow
                key={item.extension.id}
                item={item}
                onClick={() => navigate("/extensions")}
              />
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
            No extensions yet
          </h3>
          <p className="mt-1 text-sm text-muted-foreground">
            HarnessKit scans your coding agents and marketplace for extensions to manage.
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
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <QuickAction
              icon={Shield}
              label="Run Audit"
              sublabel={
                hasAuditData
                  ? `Last run scanned ${auditResults.length} extensions`
                  : "Scan extensions for security issues"
              }
              onClick={() => {
                runAudit();
                navigate("/audit");
              }}
            />
            <QuickAction
              icon={ShoppingBag}
              label="Browse Marketplace"
              sublabel="Discover skills and MCP servers"
              onClick={() => navigate("/marketplace")}
            />
            <QuickAction
              icon={RefreshCw}
              label="Check Updates"
              sublabel="See if any extensions have new versions"
              onClick={() => {
                checkUpdates();
                navigate("/extensions");
              }}
            />
          </div>
        </section>
      )}

      {/* ----------------------------------------------------------------- */}
      {/* Quiet footer — timestamp feel                                     */}
      {/* ----------------------------------------------------------------- */}
      {stats.total_extensions > 0 && (
        <footer className="flex items-center gap-1.5 pt-2 text-xs text-muted-foreground/60">
          <Clock size={11} aria-hidden="true" />
          <span>
            {hasAuditData
              ? `Last audit ${formatRelativeTime(
                  auditResults.reduce((latest, r) =>
                    r.audited_at > latest ? r.audited_at : latest,
                  ""),
                )}`
              : "No audit run yet"}
          </span>
        </footer>
      )}
    </div>
  );
}
