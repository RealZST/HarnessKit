import { clsx } from "clsx";
import {
  Check,
  ChevronRight,
  CircleAlert,
  ExternalLink,
  Eye,
  RefreshCw,
  Search,
  Shield,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Hint } from "@/components/shared/hint";
import { TrustBadge } from "@/components/shared/trust-badge";
import { useScope } from "@/hooks/use-scope";
import { api } from "@/lib/invoke";
import type { ConfigScope, Extension } from "@/lib/types";
import {
  extensionGroupKey,
  formatRelativeTime,
  type TrustTier,
  trustTier,
} from "@/lib/types";
import { isWeb, webSelectStyle } from "@/lib/web-select";
import { useAuditStore } from "@/stores/audit-store";
import { buildGroups } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";
import {
  AUDIT_RULES,
  type GroupedResult,
  maxSeverity,
  rulesForKind,
  severityBadgeClass,
  severityIconColor,
} from "./audit-utils";

/** kebab-case rule id → camelCase i18n key (e.g. "prompt-injection" → "promptInjection") */
function ruleI18nKey(id: string): string {
  return id.replace(/-(\w)/g, (_, c: string) => c.toUpperCase());
}

function IndeterminateBar({ className = "" }: { className?: string }) {
  return (
    <div
      className={`h-1 w-full overflow-hidden rounded-full bg-muted ${className}`}
    >
      <div className="indeterminate-bar h-full w-1/4 rounded-full bg-primary" />
    </div>
  );
}

export default function AuditPage() {
  const { t } = useTranslation("audit");
  const { t: tc } = useTranslation("common");
  const hydrated = useScopeStore((s) => s.hydrated);
  const {
    results,
    loading,
    loadCached,
    runAudit,
    searchQuery,
    setSearchQuery,
    tierFilter,
    setTierFilter,
  } = useAuditStore();
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const [openId, setOpenId] = useState<string | null>(null);
  const [showAllRules, setShowAllRules] = useState<Set<string>>(new Set());
  const [expandedFindings, setExpandedFindings] = useState<Set<string>>(
    new Set(),
  );
  const toggleFinding = (key: string) =>
    setExpandedFindings((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const [allExtensions, setAllExtensions] = useState<Extension[]>([]);
  const [extensionsReady, setExtensionsReady] = useState(false);
  const { scope } = useScope();

  // Close any expanded finding row when the user switches scope — the
  // previously-open extension may not exist in the new scope.
  const prevScopeRef = useRef(scope);
  useEffect(() => {
    if (prevScopeRef.current !== scope) {
      setOpenId(null);
      prevScopeRef.current = scope;
    }
  }, [scope]);

  // Search & filter state — persisted in Zustand store so filters survive navigation

  useEffect(() => {
    if (!hydrated) return;
    loadCached();
    // Fetch ALL extensions (unfiltered) for name resolution
    api
      .listExtensions()
      .then((exts) => {
        setAllExtensions(exts);
        setExtensionsReady(true);
      })
      .catch(() => {
        setExtensionsReady(true);
      });
  }, [loadCached, hydrated]);

  // Capture ?ext= query param for deferred scrolling (resolved after groupedResults are ready)
  const pendingScrollRef = useRef<string | null>(searchParams.get("ext"));
  useEffect(() => {
    if (!pendingScrollRef.current) return;
    // Clear filters so the target extension is guaranteed to be visible
    setSearchQuery("");
    setTierFilter(null);
    if (searchParams.has("ext")) {
      searchParams.delete("ext");
      setSearchParams(searchParams, { replace: true });
    }
  }, [searchParams, setSearchParams, setSearchQuery, setTierFilter]);

  const nameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of allExtensions) {
      let name = ext.name;
      if (ext.kind === "hook") {
        const parts = name.split(":");
        if (parts.length >= 3) {
          name = parts
            .slice(2)
            .join(":")
            .split(" ")
            .map((t) => t.split("/").pop() || t)
            .join(" ");
        }
      }
      map.set(ext.id, name);
    }
    return map;
  }, [allExtensions]);

  // Map extension ID → groupKey for audit deduplication.
  // Group by extensionGroupKey (same as extensions page).
  const groupKeyMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of allExtensions) {
      map.set(ext.id, extensionGroupKey(ext));
    }
    return map;
  }, [allExtensions]);

  // Map extension ID → scope (used by the scope filter on scopedResults).
  const scopeMap = useMemo(() => {
    const map = new Map<string, ConfigScope>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.scope);
    }
    return map;
  }, [allExtensions]);

  // Apply the global scope filter to raw audit results before any other
  // derivation (counts, sorting, grouping). In All-scopes mode every result
  // passes; otherwise we keep only results whose extension lives in the
  // selected scope.
  const scopedResults = useMemo(() => {
    if (scope.type === "all") return results;
    return results.filter((r) => {
      const extScope = scopeMap.get(r.extension_id);
      if (!extScope) return false;
      if (scope.type === "global") return extScope.type === "global";
      return extScope.type === "project" && extScope.path === scope.path;
    });
  }, [results, scope, scopeMap]);

  // Count extensions that actually have audit results
  const totalExtensions = useMemo(() => {
    const auditedIds = new Set(scopedResults.map((r) => r.extension_id));
    return buildGroups(allExtensions.filter((e) => auditedIds.has(e.id)))
      .length;
  }, [allExtensions, scopedResults]);

  const sortedResults = useMemo(
    () =>
      [...scopedResults].sort((a, b) => {
        const scoreDiff = a.trust_score - b.trust_score;
        if (scoreDiff !== 0) return scoreDiff;
        const nameA = nameMap.get(a.extension_id) ?? a.extension_id;
        const nameB = nameMap.get(b.extension_id) ?? b.extension_id;
        return nameA.localeCompare(nameB);
      }),
    [scopedResults, nameMap],
  );

  // Map extension ID → agent names for display
  const agentMap = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.agents);
    }
    return map;
  }, [allExtensions]);

  // Map extension ID → kind
  const kindMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of allExtensions) {
      map.set(ext.id, ext.kind);
    }
    return map;
  }, [allExtensions]);

  // Group results by extension
  const groupedResults = useMemo<GroupedResult[]>(() => {
    const groups = new Map<string, GroupedResult>();
    for (const result of sortedResults) {
      const name = nameMap.get(result.extension_id) ?? result.extension_id;
      const key = groupKeyMap.get(result.extension_id) ?? result.extension_id;
      const agentNames = agentMap.get(result.extension_id) ?? ["unknown"];
      const agentLabel = agentNames.join(", ");

      const existing = groups.get(key);
      if (existing) {
        existing.agents.push({
          agent: agentLabel,
          id: result.extension_id,
          findings: result.findings,
          trust_score: result.trust_score,
        });
        // Add new unique findings (by rule_id) for display
        for (const f of result.findings) {
          if (!existing.findings.some((ef) => ef.rule_id === f.rule_id)) {
            existing.findings.push(f);
          }
        }
        // Use minimum score across agents (consistent with Extensions page)
        existing.trust_score = Math.min(
          ...existing.agents.map((a) => a.trust_score),
        );
      } else {
        const kind = (kindMap.get(result.extension_id) ??
          "skill") as import("@/lib/types").ExtensionKind;
        groups.set(key, {
          name,
          groupKey: key,
          kind,
          agents: [
            {
              agent: agentLabel,
              id: result.extension_id,
              findings: result.findings,
              trust_score: result.trust_score,
            },
          ],
          trust_score: result.trust_score,
          findings: [...result.findings],
          primaryId: result.extension_id,
        });
      }
    }
    return [...groups.values()].sort((a, b) => {
      const scoreDiff = a.trust_score - b.trust_score;
      if (scoreDiff !== 0) return scoreDiff;
      return a.name.localeCompare(b.name);
    });
  }, [sortedResults, nameMap, groupKeyMap, agentMap, kindMap]);

  // Apply search, severity, and trust tier filters
  const filteredResults = useMemo(() => {
    let filtered = groupedResults;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      filtered = filtered.filter((g) => g.name.toLowerCase().includes(q));
    }
    if (tierFilter) {
      filtered = filtered.filter(
        (g) => trustTier(g.trust_score) === tierFilter,
      );
    }
    return filtered;
  }, [groupedResults, searchQuery, tierFilter]);

  // Scope-aware empty state: when the user has scoped to a specific project
  // but no audit findings exist in that scope (yet results exist elsewhere),
  // surface a focused empty state instead of the generic filter UI.
  const isProjectScopeEmpty =
    scope.type === "project" &&
    scopedResults.length === 0 &&
    results.length > 0;

  const scrollToExtensionResult = useCallback(
    (extensionId: string) => {
      const group = groupedResults.find(
        (g) =>
          g.primaryId === extensionId ||
          g.agents.some((a) => a.id === extensionId),
      );
      const targetId = group?.primaryId ?? extensionId;
      setOpenId(targetId);
      requestAnimationFrame(() => {
        const el = document.getElementById(`audit-result-${targetId}`);
        if (el) el.scrollIntoView({ block: "start" });
      });
    },
    [groupedResults],
  );

  // Scroll to target extension once groupedResults and extensions are fully loaded
  useEffect(() => {
    if (
      !pendingScrollRef.current ||
      groupedResults.length === 0 ||
      !extensionsReady
    )
      return;
    const target = pendingScrollRef.current;
    pendingScrollRef.current = null;
    scrollToExtensionResult(target);
  }, [groupedResults, extensionsReady, scrollToExtensionResult]);

  function toggleShowAllRules(extId: string) {
    setShowAllRules((prev) => {
      const next = new Set(prev);
      if (next.has(extId)) next.delete(extId);
      else next.add(extId);
      return next;
    });
  }

  if (!hydrated) {
    return (
      <div className="p-4 text-sm text-muted-foreground">
        {tc("status.loading")}
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center gap-3">
          <h2 className="text-2xl font-bold tracking-tight select-none">
            {t("page.title")}
          </h2>
          <button
            onClick={runAudit}
            disabled={loading}
            className="flex items-center gap-1 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md disabled:opacity-50"
          >
            <RefreshCw
              size={12}
              className={loading ? "origin-center animate-spin" : ""}
              aria-hidden="true"
            />
            {loading ? t("page.auditing") : t("page.runAudit")}
          </button>
          {extensionsReady && results.length > 0 && (
            <p className="text-sm text-muted-foreground">
              {t("page.scanned", { count: totalExtensions })} ·{" "}
              {t("page.lastRun", {
                time: formatRelativeTime(
                  results.reduce(
                    (latest, r) =>
                      r.audited_at > latest ? r.audited_at : latest,
                    results[0].audited_at,
                  ),
                ),
              })}
            </p>
          )}
        </div>

        {extensionsReady && results.length > 0 && (
          <p className="text-xs text-muted-foreground">
            {t("page.trustScoreExplain", { count: AUDIT_RULES.length })}
          </p>
        )}

        <Hint id="audit-disclaimer">{t("page.disclaimer")}</Hint>

        {/* Search & Filters */}
        {extensionsReady && results.length > 0 && (
          <div className="flex flex-wrap items-center gap-2">
            {/* Search */}
            <div className="relative flex-1 min-w-[180px] max-w-xs">
              <Search
                size={14}
                className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
              />
              <input
                type="text"
                placeholder={t("page.searchPlaceholder")}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                aria-label={t("page.searchAria")}
                className="w-full rounded-lg border border-border bg-card py-1.5 pl-9 pr-8 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
              {searchQuery && (
                <button
                  onClick={() => setSearchQuery("")}
                  aria-label={t("page.clearSearch")}
                  className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  <X size={14} />
                </button>
              )}
            </div>

            {/* Trust tier filter */}
            <select
              value={tierFilter ?? ""}
              onChange={(e) =>
                setTierFilter((e.target.value || null) as TrustTier | null)
              }
              aria-label={t("page.filterByTier")}
              style={webSelectStyle}
              className={clsx(
                "shrink-0 border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none transition-colors",
                isWeb ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
              )}
            >
              <option value="">{t("page.allTiers")}</option>
              <option value="Safe">{t("tiers.Safe")}</option>
              <option value="LowRisk">{t("tiers.LowRisk")}</option>
              <option value="NeedsReview">{t("tiers.NeedsReview")}</option>
            </select>
            <span className="text-xs text-muted-foreground">
              {t("page.resultCount", { count: filteredResults.length })}
            </span>

            {/* Clear filters */}
            {(searchQuery || tierFilter) && (
              <button
                onClick={() => {
                  setSearchQuery("");
                  setTierFilter(null);
                }}
                className="rounded-md bg-muted/60 px-2 py-0.5 text-xs text-foreground/70 hover:bg-muted hover:text-foreground transition-colors"
              >
                {t("page.clearFilters")}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto space-y-6">
        {/* Per-extension list */}
        <div className="space-y-1.5">
          {(loading || !extensionsReady) && results.length === 0 && (
            <div className="py-12 px-6" aria-live="polite" role="status">
              <p className="text-sm font-medium text-foreground">
                {t("loadingState.running")}
              </p>
              <p className="mt-1 text-sm text-muted-foreground">
                {t("loadingState.scanning")}
              </p>
              <div className="mt-4">
                <IndeterminateBar className="max-w-xs" />
              </div>
            </div>
          )}
          {!loading && extensionsReady && results.length === 0 && (
            <div className="py-12 px-6" aria-live="polite" role="status">
              <h3 className="text-lg font-semibold text-foreground">
                {t("empty.readyTitle")}
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                {t("empty.readyDescription")}
              </p>
              <button
                onClick={runAudit}
                className="mt-4 flex items-center gap-2 rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
              >
                <Shield size={14} aria-hidden="true" />
                {t("page.runAudit")}
              </button>
            </div>
          )}
          {isProjectScopeEmpty && !loading && (
            <div className="rounded-xl border border-dashed p-8 text-center">
              <p className="text-sm font-medium">
                {t("empty.noResultsInScope", {
                  scope:
                    scope.type === "project" ? scope.name : tc("scope.global"),
                })}
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                {t("empty.nothingInstalled")}
              </p>
            </div>
          )}
          {!isProjectScopeEmpty &&
            filteredResults.length === 0 &&
            results.length > 0 &&
            !loading && (
              <div className="py-8 text-center text-sm text-muted-foreground">
                {t("empty.noFilterMatch")}
                <button
                  onClick={() => {
                    setSearchQuery("");
                    setTierFilter(null);
                  }}
                  className="ml-1 font-medium text-foreground/70 hover:text-foreground transition-colors"
                >
                  {t("page.clearFilters")}
                </button>
              </div>
            )}
          {extensionsReady &&
            filteredResults.map((group) => {
              const { primaryId } = group;
              const isOpen = openId === primaryId;
              const failedRuleIds = new Set(
                group.findings.map((f) => f.rule_id),
              );
              const hasFindings = group.findings.length > 0;
              const showingAll = showAllRules.has(primaryId);
              const applicableRules = rulesForKind(group.kind);
              const failedRules = applicableRules.filter((r) =>
                failedRuleIds.has(r.id),
              );
              const passedCount = applicableRules.length - failedRules.length;

              // Clean extensions: minimal row
              if (!hasFindings) {
                return (
                  <div
                    key={primaryId}
                    id={`audit-result-${primaryId}`}
                    className="flex items-center justify-between rounded-lg px-4 py-2.5 text-sm transition-colors duration-150 hover:bg-muted/30"
                  >
                    <div className="flex items-center gap-3">
                      <Check
                        size={14}
                        className="text-primary"
                        aria-hidden="true"
                      />
                      <span className="text-muted-foreground">
                        {group.name}
                      </span>
                    </div>
                    <span className="text-xs text-muted-foreground">
                      {t("finding.clean")}
                    </span>
                  </div>
                );
              }

              // Extensions with findings: expandable row
              return (
                <div
                  key={primaryId}
                  id={`audit-result-${primaryId}`}
                  className="rounded-xl border border-border bg-card shadow-sm"
                >
                  <button
                    onClick={() => setOpenId(isOpen ? null : primaryId)}
                    aria-expanded={isOpen}
                    aria-label={
                      isOpen
                        ? t("finding.collapseAria", { name: group.name })
                        : t("finding.expandAria", { name: group.name })
                    }
                    className="flex w-full cursor-pointer items-center justify-between rounded-xl px-4 py-3 transition-all duration-150 hover:bg-muted/50 hover:shadow-sm"
                  >
                    <div className="flex items-center gap-3">
                      <ChevronRight
                        size={16}
                        className={`text-muted-foreground transition-transform duration-200 ${isOpen ? "rotate-90" : ""}`}
                      />
                      <span className="font-medium">{group.name}</span>
                      <span className="text-xs text-muted-foreground">
                        {t("finding.count", { count: group.findings.length })}
                      </span>
                    </div>
                    <TrustBadge score={group.trust_score} size="sm" />
                  </button>
                  <div
                    className="grid transition-[grid-template-rows] duration-[250ms]"
                    style={{ gridTemplateRows: isOpen ? "1fr" : "0fr" }}
                  >
                    <div className="overflow-hidden">
                      <div className="border-t border-border px-4 py-3">
                        <div className="grid gap-1.5">
                          {failedRules.map((rule) => {
                            const findingKey = `${primaryId}:${rule.id}`;
                            const isDetailOpen =
                              expandedFindings.has(findingKey);
                            // Collect findings from ALL agents to show every location
                            const allFindings = group.agents.flatMap((a) =>
                              a.findings.filter((f) => f.rule_id === rule.id),
                            );
                            // Deduplicate by message+location
                            const seen = new Set<string>();
                            const unique = allFindings.filter((f) => {
                              const key = `${f.message}\0${f.location}`;
                              if (seen.has(key)) return false;
                              seen.add(key);
                              return true;
                            });
                            const actualSeverity = maxSeverity(allFindings);
                            const ruleKey = ruleI18nKey(rule.id);
                            const ruleLabel = t(
                              // biome-ignore lint/suspicious/noExplicitAny: dynamic rule.id → typed key, no easy literal narrowing here
                              `rules.${ruleKey}.label` as any,
                              { defaultValue: rule.label },
                            );
                            const ruleDescription = t(
                              // biome-ignore lint/suspicious/noExplicitAny: dynamic rule.id → typed key, no easy literal narrowing here
                              `rules.${ruleKey}.description` as any,
                              { defaultValue: rule.description },
                            );
                            return (
                              <div key={rule.id}>
                                <button
                                  onClick={() => toggleFinding(findingKey)}
                                  aria-expanded={isDetailOpen}
                                  aria-label={
                                    isDetailOpen
                                      ? t("finding.collapseDetailsAria", {
                                          label: ruleLabel,
                                        })
                                      : t("finding.expandDetailsAria", {
                                          label: ruleLabel,
                                        })
                                  }
                                  className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm text-left transition-colors duration-150 hover:bg-muted/30"
                                >
                                  <CircleAlert
                                    size={16}
                                    className={`shrink-0 ${severityIconColor(actualSeverity)}`}
                                    aria-hidden="true"
                                  />
                                  <span className="flex-1 text-foreground">
                                    {ruleLabel}
                                  </span>
                                  <span
                                    className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(actualSeverity)}`}
                                  >
                                    {t(`severities.${actualSeverity}`)}
                                  </span>
                                  <ChevronRight
                                    size={14}
                                    className={`shrink-0 text-muted-foreground transition-transform duration-150 ${isDetailOpen ? "rotate-90" : ""}`}
                                  />
                                </button>
                                {isDetailOpen && (
                                  <div className="ml-10 mr-3 mb-1 rounded-md bg-muted/40 px-3 py-2 space-y-1.5">
                                    <p className="text-xs text-muted-foreground">
                                      {ruleDescription}
                                    </p>
                                    {unique.map((f, i) => (
                                      <div key={i} className="text-xs">
                                        <p className="text-muted-foreground">
                                          {f.message}
                                        </p>
                                        <p className="text-muted-foreground/60 font-mono truncate">
                                          {f.location}
                                        </p>
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
                              {applicableRules
                                .filter((r) => !failedRuleIds.has(r.id))
                                .map((rule) => {
                                  const ruleKey = ruleI18nKey(rule.id);
                                  return (
                                    <div
                                      key={rule.id}
                                      title={t(
                                        // biome-ignore lint/suspicious/noExplicitAny: dynamic rule.id → typed key
                                        `rules.${ruleKey}.description` as any,
                                        { defaultValue: rule.description },
                                      )}
                                      className="flex items-center gap-3 rounded-lg px-3 py-1.5 text-sm text-muted-foreground"
                                    >
                                      <Check
                                        size={14}
                                        className="shrink-0 text-primary/60"
                                        aria-hidden="true"
                                      />
                                      <span className="flex-1">
                                        {t(
                                          // biome-ignore lint/suspicious/noExplicitAny: dynamic rule.id → typed key
                                          `rules.${ruleKey}.label` as any,
                                          { defaultValue: rule.label },
                                        )}
                                      </span>
                                      <span className="text-xs">
                                        {t("finding.pass")}
                                      </span>
                                    </div>
                                  );
                                })}
                            </>
                          )}

                          <div className="mt-1 flex items-center gap-4">
                            <button
                              onClick={() => toggleShowAllRules(primaryId)}
                              className="flex items-center gap-1.5 px-3 text-xs text-muted-foreground transition-colors duration-150 hover:text-foreground"
                            >
                              <Eye size={12} aria-hidden="true" />
                              {showingAll
                                ? t("finding.showFailuresOnly")
                                : t("finding.showAllRules", {
                                    count: applicableRules.length,
                                    passed: passedCount,
                                  })}
                            </button>
                            <button
                              onClick={() =>
                                navigate(
                                  `/extensions?groupKey=${encodeURIComponent(group.groupKey)}`,
                                )
                              }
                              className="flex items-center gap-1.5 px-3 text-xs text-muted-foreground transition-colors duration-150 hover:text-foreground"
                            >
                              <ExternalLink size={12} aria-hidden="true" />
                              {t("finding.viewExtension")}
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
