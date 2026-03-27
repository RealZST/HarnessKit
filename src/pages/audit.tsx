import { useEffect, useMemo, useState } from "react";
import { useAuditStore } from "@/stores/audit-store";
import { useExtensionStore } from "@/stores/extension-store";
import { TrustBadge } from "@/components/shared/trust-badge";
import { RefreshCw, ChevronRight, CircleCheck, CircleAlert } from "lucide-react";

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
    case "Critical": return "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400";
    case "High": return "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400";
    case "Medium": return "bg-orange-100 text-orange-700 dark:bg-orange-900/30 dark:text-orange-400";
    case "Low": return "bg-muted text-muted-foreground";
    default: return "";
  }
}

export default function AuditPage() {
  const { results, loading, runAudit } = useAuditStore();
  const { extensions, fetch: fetchExtensions } = useExtensionStore();
  const [openId, setOpenId] = useState<string | null>(null);

  useEffect(() => {
    fetchExtensions();
    // Only auto-run audit if we have no cached results
    if (results.length === 0) runAudit();
  }, [fetchExtensions, runAudit, results.length]);

  const nameMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const ext of extensions) {
      map.set(ext.id, ext.name);
    }
    return map;
  }, [extensions]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">Security Audit</h2>
        <button
          onClick={runAudit}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-muted px-4 py-2 text-sm text-foreground hover:bg-accent disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          {loading ? "Auditing..." : "Run Audit"}
        </button>
      </div>

      <div className="grid grid-cols-2 gap-4">
        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <p className="text-sm text-muted-foreground">Extensions Scanned</p>
          <p className="mt-1 text-2xl font-bold">{results.length}</p>
        </div>
        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <p className="text-sm text-muted-foreground">Avg Trust Score</p>
          <p className="mt-1 text-2xl font-bold">
            {results.length > 0
              ? Math.round(results.reduce((s, r) => s + r.trust_score, 0) / results.length)
              : "--"}
          </p>
        </div>
      </div>

      <div className="space-y-3">
        {loading && results.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
            <RefreshCw size={24} className="animate-spin" />
            <p className="mt-3 text-sm">Running security audit...</p>
          </div>
        )}
        {results.map((result) => {
          const isOpen = openId === result.extension_id;
          const failedRuleIds = new Set(result.findings.map((f) => f.rule_id));

          return (
            <div key={result.extension_id} className="rounded-xl border border-border bg-card shadow-sm">
              <button
                onClick={() => setOpenId(isOpen ? null : result.extension_id)}
                className="flex w-full cursor-pointer items-center justify-between px-4 py-3"
              >
                <div className="flex items-center gap-3">
                  <ChevronRight size={16} className={`text-muted-foreground transition-transform ${isOpen ? "rotate-90" : ""}`} />
                  <span className="font-medium">{nameMap.get(result.extension_id) ?? result.extension_id}</span>
                </div>
                <TrustBadge score={result.trust_score} size="sm" />
              </button>
              {isOpen && (
                <div className="border-t border-border px-4 py-3">
                  <div className="grid gap-2">
                    {AUDIT_RULES.map((rule) => {
                      const failed = failedRuleIds.has(rule.id);
                      return (
                        <div
                          key={rule.id}
                          className="flex items-center gap-3 rounded-lg px-3 py-2 text-sm"
                        >
                          {failed ? (
                            <CircleAlert size={16} className="shrink-0 text-red-500 dark:text-red-400" />
                          ) : (
                            <CircleCheck size={16} className="shrink-0 text-green-500 dark:text-green-400" />
                          )}
                          <span className={`flex-1 ${failed ? "text-foreground" : "text-muted-foreground"}`}>{rule.label}</span>
                          {failed ? (
                            <>
                              <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${severityBadgeClass(rule.severity)}`}>
                                {rule.severity}
                              </span>
                              <span className="w-12 text-right font-mono text-xs text-red-500 dark:text-red-400">-{rule.deduction}</span>
                            </>
                          ) : (
                            <span className="font-mono text-xs text-green-500 dark:text-green-400">Pass</span>
                          )}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
