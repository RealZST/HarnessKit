import { useEffect, useState } from "react";
import { useMarketplaceStore } from "@/stores/marketplace-store";
import { useAgentStore } from "@/stores/agent-store";
import { Search, Download, X, Loader2, Shield, ShieldCheck, ShieldAlert, TrendingUp, BadgeCheck, Server, Package } from "lucide-react";
import type { MarketplaceItem, SkillAuditInfo } from "@/lib/types";
import { clsx } from "clsx";

function formatInstalls(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

function RiskBadge({ risk }: { risk: string | null }) {
  if (!risk) return <span className="text-xs text-muted-foreground">unknown</span>;
  const color = risk === "safe" ? "text-green-600 dark:text-green-400"
    : risk === "low" ? "text-yellow-600 dark:text-yellow-400"
    : "text-red-600 dark:text-red-400";
  const Icon = risk === "safe" ? ShieldCheck : risk === "low" ? Shield : ShieldAlert;
  return (
    <span className={`flex items-center gap-1 text-xs font-medium ${color}`}>
      <Icon size={12} />{risk}
    </span>
  );
}

function AuditSection({ audit }: { audit: SkillAuditInfo }) {
  return (
    <div className="space-y-2">
      {[
        { name: "Trust Hub", data: audit.ath },
        { name: "Socket", data: audit.socket },
        { name: "Snyk", data: audit.snyk },
      ].map(({ name, data }) => (
        <div key={name} className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">{name}</span>
          <RiskBadge risk={data?.risk ?? null} />
        </div>
      ))}
      {audit.socket?.score != null && (
        <div className="flex items-center justify-between text-xs">
          <span className="text-muted-foreground">Score</span>
          <span className="font-mono font-medium">{audit.socket.score}/100</span>
        </div>
      )}
    </div>
  );
}

function ItemRow({ item, selected, onSelect }: { item: MarketplaceItem; selected: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={onSelect}
      className={clsx(
        "flex w-full items-start gap-3 rounded-xl border px-4 py-3 text-left transition-colors",
        selected
          ? "border-ring bg-accent"
          : "border-border bg-card hover:border-ring/50 hover:bg-accent/50"
      )}
    >
      {item.icon_url && (
        <img src={item.icon_url} alt="" className="mt-0.5 h-8 w-8 shrink-0 rounded-lg" />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-medium">{item.name}</span>
          {item.verified && <BadgeCheck size={14} className="shrink-0 text-blue-500" />}
          <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
            {formatInstalls(item.installs)}
          </span>
          {item.categories.slice(0, 2).map((c) => (
            <span key={c} className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{c}</span>
          ))}
        </div>
        <p className="mt-0.5 text-xs text-muted-foreground line-clamp-2">{item.description}</p>
        <p className="mt-0.5 text-xs text-muted-foreground/70">{item.source}</p>
      </div>
    </button>
  );
}

export default function MarketplacePage() {
  const {
    tab, setTab, query, setQuery, results, trending, loading, trendingLoading,
    search, loadTrending, selectedItem, selectItem, closePreview,
    previewContent, previewLoading,
    auditInfo, auditLoading,
    installing, install,
  } = useMarketplaceStore();
  const { agents, fetch: fetchAgents } = useAgentStore();
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { fetchAgents(); }, [fetchAgents]);
  useEffect(() => { loadTrending(); }, [loadTrending]);

  const handleSearch = () => { setError(null); search(); };
  const handleInstall = async (item: MarketplaceItem, targetAgent?: string) => {
    setError(null);
    try {
      await install(item, targetAgent);
      setInstalled((prev) => new Set(prev).add(`${item.id}:${targetAgent ?? ""}`));
    } catch (e) {
      setError(String(e));
    }
  };

  const detectedAgents = agents.filter((a) => a.detected);
  const displayItems = query.length >= 2 ? results : trending;
  const showTrending = query.length < 2;

  return (
    <div className="flex gap-4">
      <div className="flex-1 space-y-4 min-w-0">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold">Marketplace</h2>
          <div className="flex rounded-lg border border-border">
            <button
              onClick={() => setTab("skill")}
              className={clsx(
                "flex items-center gap-1.5 rounded-l-lg px-3 py-1.5 text-xs font-medium transition-colors",
                tab === "skill"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent"
              )}
            >
              <Package size={12} />Skills
            </button>
            <button
              onClick={() => setTab("mcp")}
              className={clsx(
                "flex items-center gap-1.5 rounded-r-lg px-3 py-1.5 text-xs font-medium transition-colors",
                tab === "mcp"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent"
              )}
            >
              <Server size={12} />MCP Servers
            </button>
          </div>
        </div>

        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder={tab === "skill" ? "Search skills..." : "Search MCP servers..."}
              className="w-full rounded-lg border border-border bg-card py-2 pl-9 pr-3 text-sm placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
          </div>
          <button
            onClick={handleSearch}
            disabled={loading || query.length < 2}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? <Loader2 size={14} className="animate-spin" /> : "Search"}
          </button>
        </div>

        {error && <p className="text-sm text-red-500">{error}</p>}

        {showTrending && !trendingLoading && trending.length > 0 && (
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <TrendingUp size={14} />
            <span>Trending {tab === "skill" ? "Skills" : "MCP Servers"}</span>
          </div>
        )}

        {(loading || trendingLoading) && displayItems.length === 0 && (
          <div className="flex justify-center py-12">
            <Loader2 size={24} className="animate-spin text-muted-foreground" />
          </div>
        )}

        {!loading && !trendingLoading && displayItems.length === 0 && query.length >= 2 && (
          <p className="py-8 text-center text-sm text-muted-foreground">No results found.</p>
        )}

        <div className="grid gap-2">
          {displayItems.map((item) => (
            <ItemRow
              key={item.id}
              item={item}
              selected={selectedItem?.id === item.id}
              onSelect={() => selectItem(item)}
            />
          ))}
        </div>
      </div>

      {/* Detail Panel */}
      {selectedItem && (
        <div
          onWheel={(e) => e.stopPropagation()}
          className="w-96 shrink-0 sticky top-0 self-start max-h-[calc(100vh-3rem)] overflow-y-auto overscroll-contain rounded-xl border border-border bg-card p-5 shadow-sm"
        >
          <div className="flex items-start justify-between">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                {selectedItem.icon_url && <img src={selectedItem.icon_url} alt="" className="h-6 w-6 rounded" />}
                <h3 className="text-lg font-semibold">{selectedItem.name}</h3>
                {selectedItem.verified && <BadgeCheck size={16} className="shrink-0 text-blue-500" />}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">{selectedItem.source}</p>
              <p className="mt-1 text-xs text-muted-foreground/70">{formatInstalls(selectedItem.installs)} uses</p>
            </div>
            <button onClick={closePreview} className="rounded-lg p-1 text-muted-foreground hover:text-foreground">
              <X size={18} />
            </button>
          </div>

          {selectedItem.description && (
            <p className="mt-3 text-sm text-muted-foreground">{selectedItem.description}</p>
          )}

          {selectedItem.categories.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-1">
              {selectedItem.categories.map((c) => (
                <span key={c} className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">{c}</span>
              ))}
            </div>
          )}

          {/* Security Audit (skills only) */}
          {selectedItem.kind === "skill" && (
            <div className="mt-4">
              <h4 className="mb-2 text-xs font-medium text-muted-foreground">Security Audit</h4>
              <div className="rounded-lg border border-border bg-card p-3">
                {auditLoading ? (
                  <div className="flex justify-center py-2"><Loader2 size={14} className="animate-spin text-muted-foreground" /></div>
                ) : auditInfo ? (
                  <AuditSection audit={auditInfo} />
                ) : (
                  <p className="text-xs text-muted-foreground italic">No audit data available</p>
                )}
              </div>
            </div>
          )}

          {/* Install to agents */}
          {detectedAgents.length > 0 && selectedItem.kind === "skill" && (
            <div className="mt-4">
              <h4 className="mb-2 text-xs font-medium text-muted-foreground">Install to Agent</h4>
              <div className="flex flex-wrap gap-1.5">
                {detectedAgents.map((agent) => (
                  <button
                    key={agent.name}
                    disabled={installing === selectedItem.id || installed.has(`${selectedItem.id}:${agent.name}`)}
                    onClick={() => handleInstall(selectedItem, agent.name)}
                    className="flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs text-foreground hover:border-ring hover:bg-accent disabled:opacity-50"
                  >
                    {installed.has(`${selectedItem.id}:${agent.name}`) ? (
                      <ShieldCheck size={12} className="text-green-500" />
                    ) : installing === selectedItem.id ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <Download size={12} />
                    )}
                    {agent.name}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* SKILL.md content (skills only) */}
          {selectedItem.kind === "skill" && (
            <div className="mt-4">
              <h4 className="mb-2 text-xs font-medium text-muted-foreground">Documentation</h4>
              <div className="rounded-lg border border-border bg-card p-3">
                {previewLoading ? (
                  <div className="flex justify-center py-8"><Loader2 size={20} className="animate-spin text-muted-foreground" /></div>
                ) : previewContent ? (
                  <pre className="whitespace-pre-wrap text-xs text-muted-foreground max-h-[40vh] overflow-y-auto">{previewContent}</pre>
                ) : (
                  <p className="text-xs text-muted-foreground italic">No preview available</p>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
