import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useAuditStore } from "@/stores/audit-store";
import { TrustBadge } from "@/components/shared/trust-badge";
import type { Severity, Extension, AuditFinding } from "@/lib/types";
import { formatRelativeTime, trustTier, type TrustTier } from "@/lib/types";
import { api } from "@/lib/invoke";
import { buildGroups } from "@/stores/extension-store";
import { RefreshCw, ChevronRight, CircleAlert, Shield, Check, Eye, Search, X } from "lucide-react";
import { Hint } from "@/components/shared/hint";

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
  { id: "permission-combo-risk", label: "Permission Combination Risk", severity: "High" as Severity, deduction: 15, description: "Dangerous combination of permissions that could enable data exfiltration or RCE" },
  { id: "cli-credential-storage", label: "CLI Credential Storage", severity: "High" as Severity, deduction: 15, description: "CLI credential file has overly permissive permissions or unknown storage location" },
  { id: "cli-network-access", label: "CLI Network Access", severity: "Medium" as Severity, deduction: 8, description: "CLI connects to many external API domains" },
  { id: "cli-binary-source", label: "CLI Binary Source", severity: "High" as Severity, deduction: 15, description: "CLI binary was installed via untrusted method or has unknown origin" },
  { id: "cli-permission-scope", label: "CLI Permission Scope", severity: "Medium" as Severity, deduction: 8, description: "CLI child skills span many permission types" },
  { id: "cli-aggregate-risk", label: "CLI Aggregate Risk", severity: "Medium" as Severity, deduction: 8, description: "CLI child skills combine network, filesystem, and shell permissions" },
  { id: "plugin-source-trust", label: "Plugin Source Trust", severity: "Medium" as Severity, deduction: 8, description: "Plugin has no standard manifest file or no tracked Git origin" },
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

export default function AuditPage() {
  const { results, loading, loadCached, runAudit } = useAuditStore();
  const [searchParams, setSearchParams] = useSearchParams();
  const [openId, setOpenId] = useState<string | null>(null);
  const [showAllRules, setShowAllRules] = useState<Set<string>>(new Set());
  const [expandedFindings, setExpandedFindings] = useState<Set<string>>(new Set());
  const toggleFinding = (key: string) => setExpandedFindings((prev) => {
    const next = new Set(prev);
    if (next.has(key)) next.delete(key); else next.add(key);
    return next;
  });
  const [allExtensions, setAllExtensions] = useState<Extension[]>([]);

  // Search & filter state
  const [searchQuery, setSearchQuery] = useState("");
  const [severityFilter, setSeverityFilter] = useState<Severity | null>(null);
  const [tierFilter, setTierFilter] = useState<TrustTier | null>(null);

  useEffect(() => {
    loadCached();
    // Fetch ALL extensions (unfiltered) for name resolution
    api.listExtensions().then(setAllExtensions).catch(() => {});
  }, [loadCached]);

  // Handle ?ext= query param to scroll to a specific extension
  const didScrollRef = useRef(false);
  useEffect(() => {
    if (didScrollRef.current || results.length === 0) return;
    const extParam = searchParams.get("ext");
    if (extParam) {
      didScrollRef.current = true;
      searchParams.delete("ext");
      setSearchParams(searchParams, { replace: true });
      scrollToExtensionResult(extParam);
    }
  }, [results, searchParams, setSearchParams]);

  const nameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.name);
    }
    return map;
  }, [allExtensions]);

  // Use same deduplication as Overview for consistent extension count
  const totalExtensions = useMemo(() => buildGroups(allExtensions).length, [allExtensions]);


  const sortedResults = useMemo(
    () => [...results].sort((a, b) => a.trust_score - b.trust_score),
    [results]
  );

  // Map extension ID → agent names for display
  const agentMap = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.agents);
    }
    return map;
  }, [allExtensions]);

  // Group results by extension name to deduplicate same extension across agents
  interface GroupedResult {
    name: string;
    /** Per-agent sub-results. If all agents have identical findings, this has one merged entry. */
    agents: { agent: string; id: string; findings: AuditFinding[]; trust_score: number }[];
    /** Whether all agents share the same findings (can display as one). */
    uniform: boolean;
    /** Overall trust score (lowest across agents). */
    trust_score: number;
    /** Merged unique findings across all agents. */
    findings: AuditFinding[];
    /** Primary ID for keying and scroll targets. */
    primaryId: string;
  }

  const groupedResults = useMemo<GroupedResult[]>(() => {
    const groups = new Map<string, GroupedResult>();
    for (const result of sortedResults) {
      const name = nameMap.get(result.extension_id) ?? result.extension_id;
      const agentNames = agentMap.get(result.extension_id) ?? ["unknown"];
      const agentLabel = agentNames.join(", ");

      const existing = groups.get(name);
      if (existing) {
        existing.agents.push({ agent: agentLabel, id: result.extension_id, findings: result.findings, trust_score: result.trust_score });
        existing.trust_score = Math.min(existing.trust_score, result.trust_score);
        for (const f of result.findings) {
          if (!existing.findings.some(ef => ef.rule_id === f.rule_id)) {
            existing.findings.push(f);
          }
        }
      } else {
        groups.set(name, {
          name,
          agents: [{ agent: agentLabel, id: result.extension_id, findings: result.findings, trust_score: result.trust_score }],
          uniform: true,
          trust_score: result.trust_score,
          findings: [...result.findings],
          primaryId: result.extension_id,
        });
      }
    }
    // Determine if agents within each group have identical findings
    for (const group of groups.values()) {
      if (group.agents.length <= 1) {
        group.uniform = true;
      } else {
        const first = new Set(group.agents[0].findings.map(f => f.rule_id));
        group.uniform = group.agents.every(a => {
          const ruleIds = new Set(a.findings.map(f => f.rule_id));
          return ruleIds.size === first.size && [...ruleIds].every(id => first.has(id));
        });
      }
    }
    return [...groups.values()];
  }, [sortedResults, nameMap, agentMap]);

  // Apply search, severity, and trust tier filters
  const filteredResults = useMemo(() => {
    let filtered = groupedResults;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      filtered = filtered.filter((g) => g.name.toLowerCase().includes(q));
    }
    if (severityFilter) {
      filtered = filtered.filter((g) =>
        g.findings.some((f) => {
          const rule = AUDIT_RULES.find((r) => r.id === f.rule_id);
          return rule?.severity === severityFilter;
        })
      );
    }
    if (tierFilter) {
      filtered = filtered.filter((g) => trustTier(g.trust_score) === tierFilter);
    }
    return filtered;
  }, [groupedResults, searchQuery, severityFilter, tierFilter]);

  function scrollToExtensionResult(extensionId: string) {
    setOpenId(extensionId);
    // Scroll to the element after a short delay to let it render
    setTimeout(() => {
      const el = document.getElementById(`audit-result-${extensionId}`);
      if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 100);
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
              <span className="font-medium text-foreground">{totalExtensions}</span> extensions scanned · Last run {formatRelativeTime(results.reduce((latest, r) => r.audited_at > latest ? r.audited_at : latest, results[0].audited_at))}
            </p>
            <p className="text-xs text-muted-foreground">
              Trust scores (0–100) reflect {AUDIT_RULES.length} security checks. 80+ is safe, 60–79 is low risk, below 60 needs review.
            </p>
          </div>
        )}

        <Hint id="audit-disclaimer">
          Automated heuristic checks — not a substitute for professional security review.
        </Hint>

        {/* Search & Filters */}
        {results.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            {/* Search */}
            <div className="relative flex-1 min-w-[180px] max-w-xs">
              <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
              <input
                type="text"
                placeholder="Search extensions..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="w-full rounded-lg border border-border bg-card py-1.5 pl-9 pr-8 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
              {searchQuery && (
                <button onClick={() => setSearchQuery("")} className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground">
                  <X size={14} />
                </button>
              )}
            </div>

            {/* Severity filter */}
            <select
              value={severityFilter ?? ""}
              onChange={(e) => setSeverityFilter((e.target.value || null) as Severity | null)}
              className="rounded-lg border border-border bg-card px-2.5 py-1.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="">All Severities</option>
              <option value="Critical">Critical</option>
              <option value="High">High</option>
              <option value="Medium">Medium</option>
              <option value="Low">Low</option>
            </select>

            {/* Trust tier filter */}
            <select
              value={tierFilter ?? ""}
              onChange={(e) => setTierFilter((e.target.value || null) as TrustTier | null)}
              className="rounded-lg border border-border bg-card px-2.5 py-1.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            >
              <option value="">All Trust Tiers</option>
              <option value="Safe">Safe</option>
              <option value="LowRisk">Low Risk</option>
              <option value="NeedsReview">Needs Review</option>
            </select>

            {/* Clear filters */}
            {(searchQuery || severityFilter || tierFilter) && (
              <button
                onClick={() => { setSearchQuery(""); setSeverityFilter(null); setTierFilter(null); }}
                className="text-xs text-muted-foreground hover:text-foreground transition-colors"
              >
                Clear filters
              </button>
            )}
          </div>
        )}
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto space-y-6">
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
        {filteredResults.length === 0 && results.length > 0 && !loading && (
          <div className="py-8 text-center text-sm text-muted-foreground">
            No extensions match your filters.
          </div>
        )}
        {filteredResults.map((group) => {
          const { primaryId } = group;
          const isOpen = openId === primaryId;
          const failedRuleIds = new Set(group.findings.map((f) => f.rule_id));
          const hasFindings = group.findings.length > 0;
          const showingAll = showAllRules.has(primaryId);
          const failedRules = AUDIT_RULES.filter(r => failedRuleIds.has(r.id));
          const passedCount = AUDIT_RULES.length - failedRules.length;

          // Clean extensions: minimal row
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
                  {!group.uniform && (
                    <span className="text-xs text-trust-high-risk">varies by agent</span>
                  )}
                </div>
                <TrustBadge score={group.trust_score} size="sm" />
              </button>
              <div
                className="grid transition-[grid-template-rows] duration-[250ms]"
                style={{ gridTemplateRows: isOpen ? '1fr' : '0fr' }}
              >
                <div className="overflow-hidden">
                  <div className="border-t border-border px-4 py-3">
                    {group.uniform ? (
                      /* All agents have same findings — show merged view */
                      <div className="grid gap-1.5">
                        {failedRules.map((rule) => {
                          const findingKey = `${primaryId}:${rule.id}`;
                          const isDetailOpen = expandedFindings.has(findingKey);
                          // Collect findings from ALL agents to show every location
                          const allFindings = group.agents.flatMap((a) => a.findings.filter((f) => f.rule_id === rule.id));
                          // Deduplicate by message+location
                          const seen = new Set<string>();
                          const unique = allFindings.filter((f) => {
                            const key = `${f.message}\0${f.location}`;
                            if (seen.has(key)) return false;
                            seen.add(key);
                            return true;
                          });
                          return (
                            <div key={rule.id}>
                              <button
                                onClick={() => toggleFinding(findingKey)}
                                title={rule.description}
                                className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-left transition-colors duration-150 hover:bg-muted/30"
                              >
                                <CircleAlert size={16} className="shrink-0 text-trust-critical" aria-hidden="true" />
                                <span className="flex-1 text-foreground">{rule.label}</span>
                                <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>
                                  {rule.severity}
                                </span>
                                <ChevronRight size={14} className={`shrink-0 text-muted-foreground transition-transform duration-150 ${isDetailOpen ? "rotate-90" : ""}`} />
                              </button>
                              {isDetailOpen && unique.length > 0 && (
                                <div className="ml-10 mr-3 mb-1 rounded-md bg-muted/40 px-3 py-2 space-y-1.5">
                                  {unique.map((f, i) => (
                                    <div key={i} className="text-xs">
                                      <p className="text-muted-foreground">{f.message}</p>
                                      <p className="text-muted-foreground/60 font-mono truncate">{f.location}</p>
                                    </div>
                                  ))}
                                </div>
                              )}
                            </div>
                          );
                        })}

                        {showingAll && (
                          <>
                            <div className="my-1 border-t border-border/50" />
                            {AUDIT_RULES.filter(r => !failedRuleIds.has(r.id)).map((rule) => (
                              <div key={rule.id} title={rule.description} className="flex items-center gap-3 rounded-lg px-3 py-1.5 text-sm text-muted-foreground">
                                <Check size={14} className="shrink-0 text-primary/60" aria-hidden="true" />
                                <span className="flex-1">{rule.label}</span>
                                <span className="text-xs">Pass</span>
                              </div>
                            ))}
                          </>
                        )}

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
                    ) : (
                      /* Agents have different findings — show per-agent breakdown */
                      <div className="grid gap-3">
                        {group.agents.map((agentResult) => {
                          const agentFailed = new Set(agentResult.findings.map(f => f.rule_id));
                          const agentFailedRules = AUDIT_RULES.filter(r => agentFailed.has(r.id));
                          return (
                            <div key={agentResult.id} className="space-y-1.5">
                              <div className="flex items-center justify-between px-1">
                                <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                                  {agentResult.agent}
                                </span>
                                <TrustBadge score={agentResult.trust_score} size="sm" />
                              </div>
                              {agentFailedRules.length === 0 ? (
                                <div className="flex items-center gap-3 rounded-lg px-3 py-1.5 text-sm text-muted-foreground">
                                  <Check size={14} className="shrink-0 text-primary/60" aria-hidden="true" />
                                  <span>Clean</span>
                                </div>
                              ) : (
                                agentFailedRules.map((rule) => {
                                  const findingKey = `${agentResult.id}:${rule.id}`;
                                  const isDetailOpen = expandedFindings.has(findingKey);
                                  const findings = agentResult.findings.filter((f) => f.rule_id === rule.id);
                                  return (
                                    <div key={rule.id}>
                                      <button
                                        onClick={() => toggleFinding(findingKey)}
                                        title={rule.description}
                                        className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-left transition-colors duration-150 hover:bg-muted/30"
                                      >
                                        <CircleAlert size={16} className="shrink-0 text-trust-critical" aria-hidden="true" />
                                        <span className="flex-1 text-foreground">{rule.label}</span>
                                        <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>{rule.severity}</span>
                                        <ChevronRight size={14} className={`shrink-0 text-muted-foreground transition-transform duration-150 ${isDetailOpen ? "rotate-90" : ""}`} />
                                      </button>
                                      {isDetailOpen && findings.length > 0 && (
                                        <div className="ml-10 mr-3 mb-1 rounded-md bg-muted/40 px-3 py-2 space-y-1.5">
                                          {findings.map((f, i) => (
                                            <div key={i} className="text-xs">
                                              <p className="text-muted-foreground">{f.message}</p>
                                              <p className="text-muted-foreground/60 font-mono truncate">{f.location}</p>
                                            </div>
                                          ))}
                                        </div>
                                      )}
                                    </div>
                                  );
                                })
                              )}
                              {group.agents.indexOf(agentResult) < group.agents.length - 1 && (
                                <div className="my-1 border-t border-border/30" />
                              )}
                            </div>
                          );
                        })}
                      </div>
                    )}
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
