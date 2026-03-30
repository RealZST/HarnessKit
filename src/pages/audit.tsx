import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useAuditStore } from "@/stores/audit-store";
import { TrustBadge } from "@/components/shared/trust-badge";
import { trustTier, trustColor } from "@/lib/types";
import type { Severity, Extension, AuditFinding } from "@/lib/types";
import { api } from "@/lib/invoke";
import { RefreshCw, ChevronRight, ChevronDown, CircleAlert, Shield, Check, Eye } from "lucide-react";

function IndeterminateBar({ className = "" }: { className?: string }) {
  return (
    <div className={`h-1 w-full overflow-hidden rounded-full bg-muted ${className}`}>
      <div className="indeterminate-bar h-full w-1/4 rounded-full bg-primary" />
    </div>
  );
}

const AUDIT_RULES = [
  { id: "prompt-injection", label: "Prompt Injection", severity: "Critical" as Severity, deduction: 25, description: "Extension content could manipulate the AI agent's behavior" },
  { id: "rce", label: "Remote Code Execution", severity: "Critical" as Severity, deduction: 25, description: "Extension could execute arbitrary code on your machine" },
  { id: "credential-theft", label: "Credential Theft", severity: "Critical" as Severity, deduction: 25, description: "Extension may attempt to access stored credentials" },
  { id: "plaintext-secrets", label: "Plaintext Secrets", severity: "Critical" as Severity, deduction: 25, description: "API keys or tokens found in plain text" },
  { id: "safety-bypass", label: "Safety Bypass", severity: "Critical" as Severity, deduction: 25, description: "Extension attempts to disable agent safety features" },
  { id: "dangerous-commands", label: "Dangerous Commands", severity: "High" as Severity, deduction: 15, description: "Extension uses potentially harmful shell commands" },
  { id: "broad-permissions", label: "Broad Permissions", severity: "High" as Severity, deduction: 15, description: "Extension requests more access than it needs" },
  { id: "untrusted-source", label: "Untrusted Source", severity: "Medium" as Severity, deduction: 8, description: "Extension comes from an unverified source" },
  { id: "supply-chain", label: "Supply Chain Risk", severity: "Medium" as Severity, deduction: 8, description: "Dependencies may introduce security risks" },
  { id: "outdated", label: "Outdated (90+ days)", severity: "Low" as Severity, deduction: 3, description: "Extension hasn't been updated in over 90 days" },
  { id: "unknown-source", label: "Unknown Source", severity: "Low" as Severity, deduction: 3, description: "Extension origin cannot be determined" },
  { id: "duplicate-conflict", label: "Duplicate / Conflict", severity: "Low" as Severity, deduction: 3, description: "Multiple extensions with overlapping functionality" },
] as const;

function severityBadgeClass(severity: string): string {
  switch (severity) {
    case "Critical": return "bg-trust-critical/10 text-trust-critical";
    case "High": return "bg-trust-high-risk/10 text-trust-high-risk font-semibold";
    case "Medium": return "bg-trust-low-risk/10 text-trust-low-risk";
    case "Low": return "bg-muted text-muted-foreground";
    default: return "";
  }
}

function severityTextColor(severity: string): string {
  switch (severity) {
    case "Critical": return "text-trust-critical";
    case "High": return "text-trust-high-risk";
    case "Medium": return "text-trust-low-risk";
    case "Low": return "text-muted-foreground";
    default: return "";
  }
}

const SEVERITY_ORDER: Record<string, number> = { Critical: 0, High: 1, Medium: 2, Low: 3 };

export default function AuditPage() {
  const { results, loading, loadCached, runAudit } = useAuditStore();
  const [openId, setOpenId] = useState<string | null>(null);
  const [showAllRules, setShowAllRules] = useState<Set<string>>(new Set());
  const [expandedRules, setExpandedRules] = useState<Set<string>>(new Set());
  const [collapsedSeverities, setCollapsedSeverities] = useState<Set<string>>(new Set(["Critical", "High", "Medium", "Low"]));
  const severityRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const [allExtensions, setAllExtensions] = useState<Extension[]>([]);

  useEffect(() => {
    loadCached();
    // Fetch ALL extensions (unfiltered) for name resolution
    api.listExtensions().then(setAllExtensions).catch(() => {});
  }, [loadCached]);

  const nameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.name);
    }
    return map;
  }, [allExtensions]);

  const avgScore = results.length > 0
    ? Math.round(results.reduce((s, r) => s + r.trust_score, 0) / results.length)
    : null;
  const avgTier = avgScore !== null ? trustTier(avgScore) : null;
  const avgColor = avgScore !== null ? trustColor(avgScore) : "";

  const withFindings = results.filter(r => r.findings.length > 0).length;

  const sortedResults = useMemo(
    () => [...results].sort((a, b) => a.trust_score - b.trust_score),
    [results]
  );

  // Group results by extension name to deduplicate same extension across agents
  const groupedResults = useMemo(() => {
    const groups = new Map<string, { name: string; ids: string[]; findings: AuditFinding[]; trust_score: number }>();
    for (const result of sortedResults) {
      const name = nameMap.get(result.extension_id) ?? result.extension_id;
      const existing = groups.get(name);
      if (existing) {
        // Merge: keep lowest trust score, combine unique findings
        existing.ids.push(result.extension_id);
        existing.trust_score = Math.min(existing.trust_score, result.trust_score);
        for (const f of result.findings) {
          if (!existing.findings.some(ef => ef.rule_id === f.rule_id)) {
            existing.findings.push(f);
          }
        }
      } else {
        groups.set(name, {
          name,
          ids: [result.extension_id],
          findings: [...result.findings],
          trust_score: result.trust_score,
        });
      }
    }
    return [...groups.values()];
  }, [sortedResults, nameMap]);

  function scrollToExtensionResult(extensionId: string) {
    setOpenId(extensionId);
    // Scroll to the element after a short delay to let it render
    setTimeout(() => {
      const el = document.getElementById(`audit-result-${extensionId}`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 100);
  }

  // Cross-extension findings grouped by severity
  const crossExtensionFindings = useMemo(() => {
    const groups: Record<string, { rule: typeof AUDIT_RULES[number]; extensions: { name: string; id: string }[] }> = {};

    for (const result of results) {
      const extName = nameMap.get(result.extension_id) ?? result.extension_id;
      for (const finding of result.findings) {
        const rule = AUDIT_RULES.find(r => r.id === finding.rule_id);
        if (!rule) continue;

        if (!groups[rule.id]) {
          groups[rule.id] = { rule, extensions: [] };
        }
        // Deduplicate by extension name
        const existing = groups[rule.id].extensions;
        if (!existing.some(e => e.name === extName)) {
          existing.push({ name: extName, id: result.extension_id });
        }
      }
    }

    return Object.values(groups).sort(
      (a, b) => SEVERITY_ORDER[a.rule.severity] - SEVERITY_ORDER[b.rule.severity]
    );
  }, [results, nameMap]);

  // Group cross-extension findings by severity level
  const findingsBySeverity = useMemo(() => {
    const grouped: Record<string, typeof crossExtensionFindings> = {};
    for (const finding of crossExtensionFindings) {
      const sev = finding.rule.severity;
      if (!grouped[sev]) grouped[sev] = [];
      grouped[sev].push(finding);
    }
    return grouped;
  }, [crossExtensionFindings]);

  const severityLevels = ["Critical", "High", "Medium", "Low"] as const;

  function toggleSeverityCollapse(severity: string) {
    setCollapsedSeverities(prev => {
      const next = new Set(prev);
      if (next.has(severity)) next.delete(severity);
      else next.add(severity);
      return next;
    });
  }


  function toggleShowAllRules(extId: string) {
    setShowAllRules(prev => {
      const next = new Set(prev);
      if (next.has(extId)) next.delete(extId);
      else next.add(extId);
      return next;
    });
  }

  return (
    <div className="animate-fade-in flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-2xl font-bold tracking-tight select-none">Security Audit</h2>
            <button
              onClick={runAudit}
              disabled={loading}
              className="flex items-center gap-2 rounded-lg border border-border bg-card px-4 py-2 text-sm font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
            >
              <RefreshCw size={14} className={loading ? "animate-spin" : ""} aria-hidden="true" />
              {loading ? "Auditing..." : "Run Audit"}
            </button>
          </div>
        </div>

        {/* Compact summary row */}
        {results.length > 0 && (
          <div className="space-y-1">
            <p className="text-sm text-muted-foreground">
              <span className="font-medium text-foreground">{results.length}</span> extensions scanned
              {avgScore !== null && (
                <>
                  {" · Avg score "}
                  <span className={`font-medium ${avgColor}`}>{avgScore}</span>
                  {avgTier && (
                    <span className={`${avgColor}`}> ({avgTier === "LowRisk" ? "Low Risk" : avgTier === "HighRisk" ? "High Risk" : avgTier})</span>
                  )}
                </>
              )}
              {withFindings > 0 ? (
                <> · <span className="font-medium text-foreground">{withFindings}</span> need attention</>
              ) : (
                <> · All clean</>
              )}
            </p>
            <p className="text-xs text-muted-foreground">
              Trust scores (0–100) reflect 12 security checks. 80+ is safe, 60–79 is low risk, 40–59 needs review, below 40 is critical.
            </p>
          </div>
        )}
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto space-y-6">
      {/* Cross-extension findings summary */}
      {crossExtensionFindings.length > 0 && (
        <div className="space-y-4">
          <h3 className="text-sm font-semibold text-foreground">Findings across extensions</h3>

          {/* Severity summary bar */}
          <div className="flex items-center gap-3 text-sm">
            {severityLevels.map(severity => {
              const count = findingsBySeverity[severity]?.length ?? 0;
              if (count === 0) return null;
              return (
                <span
                  key={severity}
                  className={`font-medium ${severityTextColor(severity)}`}
                >
                  {count} {severity}
                </span>
              );
            }).filter(Boolean).reduce<ReactNode[]>((acc, el, i) => {
              if (i > 0) acc.push(<span key={`sep-${i}`} className="text-muted-foreground/40">·</span>);
              acc.push(el);
              return acc;
            }, [])}
          </div>

          {severityLevels.map(severity => {
            const items = findingsBySeverity[severity];
            if (!items || items.length === 0) return null;
            const isCollapsed = collapsedSeverities.has(severity);

            return (
              <div key={severity} className="space-y-1.5" ref={el => { severityRefs.current[severity] = el; }}>
                <button
                  onClick={() => toggleSeverityCollapse(severity)}
                  className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground uppercase tracking-wide hover:text-foreground transition-colors cursor-pointer"
                >
                  <ChevronDown size={12} className={`transition-transform duration-200 ${isCollapsed ? "-rotate-90" : ""}`} />
                  {severity}
                  <span className="normal-case tracking-normal font-normal">({items.length})</span>
                </button>
                {isCollapsed ? null : (
                  <div className="space-y-1">
                    {items.map(({ rule, extensions: exts }) => {
                      const isExpanded = expandedRules.has(rule.id);
                      const visible = isExpanded ? exts : exts.slice(0, 3);
                      const remaining = exts.length - 3;

                      return (
                        <div
                          key={rule.id}
                          title={rule.description}
                          className="flex items-start gap-2.5 py-1 text-sm"
                        >
                          <span className={`mt-0.5 shrink-0 rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>
                            {exts.length}
                          </span>
                          <span className="text-foreground">
                            {rule.label}
                            <span className="text-muted-foreground">
                              {" in "}
                              {visible.map((ext, i) => (
                                <span key={ext.id}>
                                  {i > 0 && ", "}
                                  <button
                                    onClick={() => scrollToExtensionResult(ext.id)}
                                    className="text-foreground hover:text-primary hover:underline cursor-pointer transition-colors"
                                  >
                                    {ext.name}
                                  </button>
                                </span>
                              ))}
                              {remaining > 0 && !isExpanded && (
                                <>
                                  {" "}
                                  <button
                                    onClick={() => setExpandedRules(prev => { const next = new Set(prev); next.add(rule.id); return next; })}
                                    className="text-muted-foreground hover:text-primary hover:underline cursor-pointer transition-colors"
                                  >
                                    +{remaining} more
                                  </button>
                                </>
                              )}
                              {isExpanded && exts.length > 3 && (
                                <>
                                  {" "}
                                  <button
                                    onClick={() => setExpandedRules(prev => { const next = new Set(prev); next.delete(rule.id); return next; })}
                                    className="text-muted-foreground hover:text-primary hover:underline cursor-pointer transition-colors"
                                  >
                                    show less
                                  </button>
                                </>
                              )}
                            </span>
                          </span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Per-extension list */}
      <div className="space-y-1.5">
        {loading && results.length === 0 && (
          <div className="py-12 px-6" aria-live="polite" role="status">
            <p className="text-sm font-medium text-foreground">Running security audit...</p>
            <p className="mt-1 text-sm text-muted-foreground">Scanning your extensions for security issues.</p>
            <div className="mt-4">
              <IndeterminateBar className="max-w-xs" />
            </div>
          </div>
        )}
        {!loading && results.length === 0 && (
          <div className="py-12 px-6" aria-live="polite" role="status">
            <h3 className="text-lg font-semibold text-foreground">Ready to audit</h3>
            <p className="mt-1 text-sm text-muted-foreground">Scan your extensions for vulnerabilities, dangerous commands, and trust scores.</p>
            <button
              onClick={runAudit}
              className="mt-4 flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
            >
              <Shield size={14} aria-hidden="true" />
              Run Audit
            </button>
          </div>
        )}
        {groupedResults.map((group) => {
          const primaryId = group.ids[0];
          const isOpen = openId === primaryId;
          const failedRuleIds = new Set(group.findings.map((f) => f.rule_id));
          const hasFindings = group.findings.length > 0;
          const showingAll = showAllRules.has(primaryId);
          const failedRules = AUDIT_RULES.filter(r => failedRuleIds.has(r.id));
          const passedCount = AUDIT_RULES.length - failedRules.length;

          // Clean extensions: minimal row, no expandable card
          if (!hasFindings) {
            return (
              <div
                key={primaryId}
                id={`audit-result-${primaryId}`}
                className="flex items-center justify-between rounded-lg px-4 py-2.5 text-sm transition-colors duration-150 hover:bg-muted/30"
              >
                <div className="flex items-center gap-3">
                  <Check size={14} className="text-primary" aria-hidden="true" />
                  <span className="text-muted-foreground">{group.name}</span>
                </div>
                <span className="text-xs text-muted-foreground">Clean</span>
              </div>
            );
          }

          // Extensions with findings: expandable row
          return (
            <div key={primaryId} id={`audit-result-${primaryId}`} className="rounded-xl border border-border bg-card shadow-sm">
              <button
                onClick={() => setOpenId(isOpen ? null : primaryId)}
                aria-expanded={isOpen}
                aria-label={`${isOpen ? "Collapse" : "Expand"} ${group.name} audit results`}
                className="flex w-full cursor-pointer items-center justify-between rounded-xl px-4 py-3 transition-all duration-150 hover:bg-muted/50 hover:shadow-sm"
              >
                <div className="flex items-center gap-3">
                  <ChevronRight size={16} className={`text-muted-foreground transition-transform duration-200 ${isOpen ? "rotate-90" : ""}`} />
                  <span className="font-medium">{group.name}</span>
                  <span className="text-xs text-muted-foreground">
                    {group.findings.length} {group.findings.length === 1 ? "finding" : "findings"}
                  </span>
                </div>
                <TrustBadge score={group.trust_score} size="sm" />
              </button>
              <div
                className="grid transition-[grid-template-rows] duration-[250ms]"
                style={{ gridTemplateRows: isOpen ? '1fr' : '0fr' }}
              >
                <div className="overflow-hidden">
                  <div className="border-t border-border px-4 py-3">
                    <div className="grid gap-1.5">
                      {/* Show failed rules */}
                      {failedRules.map((rule) => (
                        <div
                          key={rule.id}
                          title={rule.description}
                          className="flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors duration-150 hover:bg-muted/30"
                        >
                          <CircleAlert size={16} className="shrink-0 text-trust-critical" aria-hidden="true" />
                          <span className="flex-1 text-foreground">{rule.label}</span>
                          <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>
                            {rule.severity}
                          </span>
                        </div>
                      ))}

                      {/* Show all rules toggle */}
                      {showingAll && (
                        <>
                          <div className="my-1 border-t border-border/50" />
                          {AUDIT_RULES.filter(r => !failedRuleIds.has(r.id)).map((rule) => (
                            <div
                              key={rule.id}
                              title={rule.description}
                              className="flex items-center gap-3 rounded-lg px-3 py-1.5 text-sm text-muted-foreground"
                            >
                              <Check size={14} className="shrink-0 text-primary/60" aria-hidden="true" />
                              <span className="flex-1">{rule.label}</span>
                              <span className="text-xs">Pass</span>
                            </div>
                          ))}
                        </>
                      )}

                      {/* Toggle link */}
                      <div className="mt-1 flex items-center gap-4">
                        <button
                          onClick={() => toggleShowAllRules(primaryId)}
                          className="flex items-center gap-1.5 px-3 text-xs text-muted-foreground transition-colors duration-150 hover:text-foreground"
                        >
                          <Eye size={12} aria-hidden="true" />
                          {showingAll ? "Show failures only" : `Show all ${AUDIT_RULES.length} rules (${passedCount} passed)`}
                        </button>
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          );
        })}
      </div>
      </div>
    </div>
  );
}
