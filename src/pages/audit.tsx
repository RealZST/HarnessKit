import { useEffect } from "react";
import { useAuditStore } from "@/stores/audit-store";
import { TrustBadge } from "@/components/shared/trust-badge";
import { severityColor } from "@/lib/types";
import { Shield, RefreshCw } from "lucide-react";

export default function AuditPage() {
  const { results, loading, runAudit } = useAuditStore();

  useEffect(() => { runAudit(); }, [runAudit]);

  const totalFindings = results.reduce((sum, r) => sum + r.findings.length, 0);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-xl font-semibold">Security Audit</h2>
        <button
          onClick={runAudit}
          disabled={loading}
          className="flex items-center gap-2 rounded-lg bg-zinc-800 px-4 py-2 text-sm text-zinc-200 hover:bg-zinc-700 disabled:opacity-50"
        >
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          {loading ? "Auditing..." : "Run Audit"}
        </button>
      </div>

      <div className="grid grid-cols-3 gap-4">
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <p className="text-sm text-zinc-400">Extensions Scanned</p>
          <p className="mt-1 text-2xl font-bold">{results.length}</p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <p className="text-sm text-zinc-400">Total Findings</p>
          <p className="mt-1 text-2xl font-bold">{totalFindings}</p>
        </div>
        <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-4">
          <p className="text-sm text-zinc-400">Avg Trust Score</p>
          <p className="mt-1 text-2xl font-bold">
            {results.length > 0
              ? Math.round(results.reduce((s, r) => s + r.trust_score, 0) / results.length)
              : "--"}
          </p>
        </div>
      </div>

      <div className="space-y-3">
        {results.map((result) => (
          <details key={result.extension_id} className="group rounded-xl border border-zinc-800 bg-zinc-900/50">
            <summary className="flex cursor-pointer items-center justify-between px-4 py-3">
              <div className="flex items-center gap-3">
                <Shield size={16} className="text-zinc-500" />
                <span className="font-medium">{result.extension_id}</span>
                <span className="text-xs text-zinc-500">{result.findings.length} findings</span>
              </div>
              <TrustBadge score={result.trust_score} size="sm" />
            </summary>
            <div className="border-t border-zinc-800 px-4 py-3 space-y-2">
              {result.findings.length === 0 && (
                <p className="text-sm text-green-400">No issues found</p>
              )}
              {result.findings.map((f, i) => (
                <div key={i} className="flex items-start gap-3 text-sm">
                  <span className={`font-mono text-xs font-bold ${severityColor(f.severity)}`}>
                    {f.severity.toUpperCase()}
                  </span>
                  <div>
                    <p className="text-zinc-200">{f.message}</p>
                    {f.location && <p className="text-xs text-zinc-500">{f.location}</p>}
                  </div>
                </div>
              ))}
            </div>
          </details>
        ))}
      </div>
    </div>
  );
}
