import { useEffect, useMemo, useState } from "react";
import { useAuditStore } from "@/stores/audit-store";
import { useExtensionStore } from "@/stores/extension-store";
import { TrustBadge } from "@/components/shared/trust-badge";
import { RefreshCw, ChevronRight, CircleCheck, CircleAlert, Shield, ScanSearch, BarChart3, ShieldAlert } from "lucide-react";

const AUDIT_RULES = [
  { id: "prompt-injection", label: "Prompt Injection", severity: "Critical", deduction: 25 },
  { id: "rce", label: "Remote Code Execution", severity: "Critical", deduction: 25 },
  { id: "credential-theft", label: "Credential Theft", severity: "Critical", deduction: 25 },
  { id: "plaintext-secrets", label: "Plaintext Secrets", severity: "Critical", deduction: 25 },
  { id: "safety-bypass", label: "Safety Bypass", severity: "Critical", deduction: 25 },
  { id: "dangerous-commands", label: "Dangerous Commands", severity: "High", deduction: 15 },
  { id: "broad-permissions", label: "Broad Permissions", severity: "High", deduction: 15 },
  { id: "untrusted-source", label: "Untrusted Source", severity: "Medium", deduction: 8 },
  { id: "supply-chain", label: "Supply Chain Risk", severity: "Medium", deduction: 8 },
  { id: "outdated", label: "Outdated (90+ days)", severity: "Low", deduction: 3 },
  { id: "unknown-source", label: "Unknown Source", severity: "Low", deduction: 3 },
  { id: "duplicate-conflict", label: "Duplicate / Conflict", severity: "Low", deduction: 3 },
] as const;

function severityBadgeClass(severity: string): string {
  switch (severity) {
    case "Critical": return "bg-destructive/10 text-destructive";
    case "High": return "bg-chart-5/10 text-chart-5 font-semibold";
    case "Medium": return "bg-chart-4/10 text-chart-4";
    case "Low": return "bg-muted text-muted-foreground";
    default: return "";
  }
}

function trustTier(score: number): { label: string; colorClass: string } {
  if (score >= 80) return { label: "Safe", colorClass: "text-primary" };
  if (score >= 60) return { label: "Low Risk", colorClass: "text-chart-4" };
  if (score >= 40) return { label: "Medium Risk", colorClass: "text-chart-5" };
  return { label: "Critical", colorClass: "text-destructive" };
}

export default function AuditPage() {
  const { results, loading, loadCached, runAudit } = useAuditStore();
  const { extensions, fetch: fetchExtensions } = useExtensionStore();
  const [openId, setOpenId] = useState<string | null>(null);

  useEffect(() => {
    fetchExtensions();
    loadCached();
  }, [fetchExtensions, loadCached]);

  const nameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of extensions) {
      map.set(ext.id, ext.name);
    }
    return map;
  }, [extensions]);

  const avgScore = results.length > 0
    ? Math.round(results.reduce((s, r) => s + r.trust_score, 0) / results.length)
    : null;
  const avgTier = avgScore !== null ? trustTier(avgScore) : null;

  const tierCounts = useMemo(() => {
    const counts = { safe: 0, low: 0, medium: 0, critical: 0 };
    for (const r of results) {
      if (r.trust_score >= 80) counts.safe++;
      else if (r.trust_score >= 60) counts.low++;
      else if (r.trust_score >= 40) counts.medium++;
      else counts.critical++;
    }
    return counts;
  }, [results]);

  const withFindings = results.filter(r => r.findings.length > 0).length;
  const clean = results.length - withFindings;

  const sortedResults = useMemo(
    () => [...results].sort((a, b) => a.trust_score - b.trust_score),
    [results]
  );

  return (
    <div className="animate-fade-in space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-2xl font-bold tracking-tight">Security Audit</h2>
        <button
          onClick={runAudit}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          {loading ? "Auditing..." : "Run Audit"}
        </button>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div
          title="Number of extensions analyzed in the last audit run"
          className="group rounded-xl border border-border border-t-2 border-t-primary/20 bg-card p-4 shadow-sm transition-shadow duration-200 hover:shadow-md"
        >
          <div className="flex items-center justify-between">
            <p className="text-sm text-muted-foreground">Extensions Scanned</p>
            <span className="rounded-lg bg-muted/50 p-1.5 text-muted-foreground/60"><ScanSearch size={16} /></span>
          </div>
          <p className="mt-1 text-3xl font-bold tabular-nums">{results.length}</p>
          {results.length > 0 && (
            <p className="mt-1 text-xs text-muted-foreground">
              {withFindings} with findings · {clean} clean
            </p>
          )}
        </div>

        <div
          title="Average trust score (0-100) across all scanned extensions"
          className="group rounded-xl border border-border border-t-2 border-t-primary/20 bg-card p-4 shadow-sm transition-shadow duration-200 hover:shadow-md"
        >
          <div className="flex items-center justify-between">
            <p className="text-sm text-muted-foreground">Avg Trust Score</p>
            <span className="rounded-lg bg-muted/50 p-1.5 text-muted-foreground/60"><BarChart3 size={16} /></span>
          </div>
          <div className="mt-1 flex items-baseline gap-2">
            <p className={`text-3xl font-bold tabular-nums ${avgTier?.colorClass ?? ""}`}>
              {avgScore ?? "--"}
            </p>
            {avgTier && (
              <span className={`text-sm font-medium ${avgTier.colorClass}`}>{avgTier.label}</span>
            )}
          </div>
        </div>

        <div
          title="Distribution of extensions by risk tier based on trust scores"
          className="group rounded-xl border border-border border-t-2 border-t-primary/20 bg-card p-4 shadow-sm transition-shadow duration-200 hover:shadow-md"
        >
          <div className="flex items-center justify-between">
            <p className="text-sm text-muted-foreground">Risk Distribution</p>
            <span className="rounded-lg bg-muted/50 p-1.5 text-muted-foreground/60"><ShieldAlert size={16} /></span>
          </div>
          {results.length > 0 ? (
            <p className="mt-2 text-sm font-medium leading-relaxed">
              <span className="text-primary">{tierCounts.safe} Safe</span>
              {" · "}
              <span className="text-chart-4">{tierCounts.low} Low</span>
              {" · "}
              <span className="text-chart-5">{tierCounts.medium} Med</span>
              {" · "}
              <span className="text-destructive">{tierCounts.critical} Critical</span>
            </p>
          ) : (
            <p className="mt-1 text-3xl font-bold tabular-nums">--</p>
          )}
        </div>
      </div>

      <div className="space-y-3">
        {loading && results.length === 0 && (
          <div className="py-12 px-6" aria-live="polite" role="status">
            <div className="flex items-center gap-3">
              <RefreshCw size={18} className="animate-spin text-muted-foreground" />
              <p className="text-sm font-medium text-foreground">Running security audit...</p>
            </div>
            <p className="mt-1 text-sm text-muted-foreground">Scanning your extensions for vulnerabilities.</p>
          </div>
        )}
        {!loading && results.length === 0 && (
          <div className="py-12 px-6" aria-live="polite" role="status">
            <h3 className="text-lg font-semibold text-foreground">No audit results</h3>
            <p className="mt-1 text-sm text-muted-foreground">Run a security audit to scan your extensions for vulnerabilities.</p>
            <button
              onClick={runAudit}
              className="mt-4 flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
            >
              <Shield size={14} />
              Run Audit
            </button>
          </div>
        )}
        {sortedResults.map((result) => {
          const isOpen = openId === result.extension_id;
          const failedRuleIds = new Set(result.findings.map((f) => f.rule_id));

          return (
            <div key={result.extension_id} className="rounded-xl border border-border bg-card shadow-sm">
              <button
                onClick={() => setOpenId(isOpen ? null : result.extension_id)}
                aria-expanded={isOpen}
                className="flex w-full cursor-pointer items-center justify-between rounded-xl px-4 py-3 transition-colors duration-150 hover:bg-muted/50"
              >
                <div className="flex items-center gap-3">
                  <ChevronRight size={16} className={`text-muted-foreground transition-transform duration-200 ${isOpen ? "rotate-90" : ""}`} />
                  <span className="font-medium">{nameMap.get(result.extension_id) ?? result.extension_id}</span>
                </div>
                <TrustBadge score={result.trust_score} size="sm" />
              </button>
              <div
                className="grid transition-[grid-template-rows] duration-[250ms]"
                style={{ gridTemplateRows: isOpen ? '1fr' : '0fr' }}
              >
                <div className="overflow-hidden">
                  <div className="border-t border-border px-4 py-3">
                    <div className="grid gap-2">
                      {AUDIT_RULES.map((rule) => {
                        const failed = failedRuleIds.has(rule.id);
                        return (
                          <div
                            key={rule.id}
                            className="flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors duration-150 hover:bg-muted/30"
                          >
                            {failed ? (
                              <CircleAlert size={16} className="shrink-0 text-destructive" />
                            ) : (
                              <CircleCheck size={16} className="shrink-0 text-primary" />
                            )}
                            <span className={`flex-1 ${failed ? "text-foreground" : "text-muted-foreground"}`}>{rule.label}</span>
                            {failed ? (
                              <>
                                <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>
                                  {rule.severity}
                                </span>
                                <span className="w-16 text-right font-mono text-xs tabular-nums text-destructive">−{rule.deduction} pts</span>
                              </>
                            ) : (
                              <span className="font-mono text-xs font-medium text-primary">Pass</span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
