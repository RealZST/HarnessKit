import { useEffect, useRef, useState } from "react";
import { useMarketplaceStore } from "@/stores/marketplace-store";
import { useAgentStore } from "@/stores/agent-store";
import { InstallDialog } from "@/components/extensions/install-dialog";
import { Search, Download, X, Loader2, Shield, ShieldCheck, ShieldAlert, TrendingUp, BadgeCheck, Server, Package, GitBranch, Terminal, Star, ExternalLink } from "lucide-react";
import { sortAgents, agentDisplayName, type MarketplaceItem, type SkillAuditInfo } from "@/lib/types";
import { humanizeError } from "@/lib/errors";
import { Hint } from "@/components/shared/hint";
import { clsx } from "clsx";
import { toast } from "@/stores/toast-store";

function formatInstalls(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

function RiskBadge({ risk }: { risk: string | null }) {
  if (!risk) return <span className="text-xs text-muted-foreground">unknown</span>;
  const color = risk === "safe" ? "text-primary"
    : risk === "low" ? "text-muted-foreground"
    : "text-destructive";
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
        { name: "Anthropic Trust Hub", data: audit.ath },
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

function ItemRow({ item, selected, onSelect, index }: { item: MarketplaceItem; selected: boolean; onSelect: () => void; index: number }) {
  return (
    <button
      onClick={onSelect}
      aria-label={`View details for ${item.name}`}
      className={clsx(
        "animate-fade-in flex w-full items-start gap-3 rounded-xl border px-4 py-3 text-left transition-[background-color,border-color,box-shadow] duration-200",
        selected
          ? "border-ring bg-accent shadow-sm"
          : "border-border bg-card hover:border-ring/50 hover:bg-accent/50 hover:shadow-sm"
      )}
      style={{ animationDelay: `${Math.min(index * 30, 300)}ms` }}
    >
      {item.icon_url && (
        <img src={item.icon_url} alt={item.name} loading="lazy" decoding="async" className="mt-0.5 h-8 w-8 shrink-0 rounded-lg" />
      )}
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="font-medium">{item.name}</span>
          {item.verified && <BadgeCheck size={14} className="shrink-0 text-primary" />}
        </div>
        <p className="mt-0.5 text-xs text-muted-foreground line-clamp-2">{item.description}</p>
        <p className="mt-0.5 text-xs text-muted-foreground/60">
          {item.kind === "cli" && item.stars != null ? (
            <><Star size={10} className="inline -mt-0.5 mr-0.5" />{formatInstalls(item.stars)}</>
          ) : (
            <>{formatInstalls(item.installs)} installs</>
          )}
          {item.categories.length > 0 && ` · ${item.categories.slice(0, 2).join(", ")}`}
          {item.source && ` · ${item.source}`}
        </p>
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
  const { agents, fetch: fetchAgents, agentOrder } = useAgentStore();
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [justInstalled, setJustInstalled] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [showInstall, setShowInstall] = useState(false);
  const detailPanelRef = useRef<HTMLDivElement>(null);

  const prefersReducedMotion = () =>
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  useEffect(() => { fetchAgents(); }, [fetchAgents]);
  useEffect(() => { loadTrending(); }, [loadTrending]);
  useEffect(() => {
    if (selectedItem) detailPanelRef.current?.focus({ preventScroll: true });
  }, [selectedItem]);

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const handleQueryChange = (value: string) => {
    setQuery(value);
    setError(null);
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => { search(); }, 300);
  };
  const handleInstall = async (item: MarketplaceItem, targetAgent?: string) => {
    setError(null);
    try {
      const result = await install(item, targetAgent);
      const key = `${item.id}:${targetAgent ?? ""}`;
      setInstalled((prev) => new Set(prev).add(key));
      toast.success(result.was_update ? `${item.name} updated` : `${item.name} installed`);
      // Trigger flash animation
      if (!prefersReducedMotion()) {
        setJustInstalled((prev) => new Set(prev).add(key));
        setTimeout(() => {
          setJustInstalled((prev) => {
            const next = new Set(prev);
            next.delete(key);
            return next;
          });
        }, 500);
      }
    } catch (e) {
      setError(String(e));
      toast.error("Installation failed");
    }
  };

  const detectedAgents = sortAgents(agents.filter((a) => a.detected), agentOrder);
  const displayItems = query.length >= 2 ? results : trending;
  const showTrending = query.length < 2;

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      {/* Fixed header */}
      <div className="shrink-0 space-y-4 pb-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-2xl font-bold tracking-tight select-none">Marketplace</h2>
            <button
              onClick={() => setShowInstall(!showInstall)}
              className="flex items-center gap-1.5 rounded-lg border border-border bg-card px-3 py-1.5 text-xs font-medium text-foreground shadow-sm transition-[background-color,box-shadow] duration-200 hover:bg-accent hover:shadow-md"
            >
              <GitBranch size={12} />
              Install from Git
            </button>
          </div>
          <div className="flex rounded-lg border border-border">
            <button
              onClick={() => setTab("skill")}
              className={clsx(
                "flex items-center gap-1.5 rounded-l-lg px-3 py-1.5 text-xs font-medium transition-colors border-b-2",
                tab === "skill"
                  ? "bg-primary text-primary-foreground border-b-primary-foreground/50"
                  : "text-muted-foreground border-b-transparent hover:bg-accent"
              )}
            >
              <Package size={12} />Skills
            </button>
            <button
              onClick={() => setTab("cli")}
              className={clsx(
                "flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium transition-colors border-b-2",
                tab === "cli"
                  ? "bg-primary text-primary-foreground border-b-primary-foreground/50"
                  : "text-muted-foreground border-b-transparent hover:bg-accent"
              )}
            >
              <Terminal size={12} />CLI Tools
            </button>
            <button
              onClick={() => setTab("mcp")}
              className={clsx(
                "flex items-center gap-1.5 rounded-r-lg px-3 py-1.5 text-xs font-medium transition-colors border-b-2",
                tab === "mcp"
                  ? "bg-primary text-primary-foreground border-b-primary-foreground/50"
                  : "text-muted-foreground border-b-transparent hover:bg-accent"
              )}
            >
              <Server size={12} />MCP Servers
            </button>
          </div>
        </div>

        <InstallDialog open={showInstall} onClose={() => setShowInstall(false)} />

        <div className="relative max-w-md">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={query}
            onChange={(e) => handleQueryChange(e.target.value)}
            placeholder={tab === "skill" ? "Search skills..." : tab === "mcp" ? "Search MCP servers..." : "Search CLI tools..."}
            aria-label="Search marketplace"
            className="w-full rounded-lg border border-border bg-card py-2 pl-9 pr-3 text-sm placeholder:text-muted-foreground transition-[background-color,border-color,box-shadow] duration-200 focus:border-ring focus:bg-background focus:shadow-md focus:outline-none"
          />
        </div>

        <Hint id="marketplace-intro">
          Search for skills, MCP servers, and CLI tools to install across your agents. Use
          'Install from Git' to install directly from a Git URL.
        </Hint>
      </div>

      {/* Scrollable content */}
      <div className="relative flex-1 min-h-0">
      <div className="absolute inset-0 overflow-y-auto space-y-4 pb-4">
        {error && <p className="text-sm text-destructive">{humanizeError(error)}</p>}

        {showTrending && !trendingLoading && trending.length > 0 && (
          <div className="flex items-center gap-2 text-sm font-medium text-foreground">
            <TrendingUp size={14} className="text-primary" />
            <span>Trending {tab === "skill" ? "Skills" : tab === "mcp" ? "MCP Servers" : "CLI Tools"}</span>
          </div>
        )}

        {(loading || trendingLoading) && displayItems.length === 0 && (
          <div className="flex justify-center py-12" aria-live="polite" role="status">
            <Loader2 size={24} className="animate-spin text-muted-foreground" />
          </div>
        )}

        {!loading && !trendingLoading && displayItems.length === 0 && query.length >= 2 && (
          <div className="py-8 px-6">
            <p className="text-sm font-medium text-foreground">Nothing matched "{query}"</p>
            <p className="mt-1 text-xs text-muted-foreground">Try different keywords or browse the trending items below.</p>
          </div>
        )}

        <div className="grid gap-2">
          {displayItems.map((item, i) => (
            <ItemRow
              key={item.id}
              item={item}
              selected={selectedItem?.id === item.id}
              onSelect={() => selectItem(item)}
              index={i}
            />
          ))}
        </div>
      </div>

      {/* Detail Panel */}
      {selectedItem && (
        <div className="absolute right-0 top-0 bottom-0 w-96 z-10">
        <div
          ref={detailPanelRef}
          tabIndex={-1}
          onWheel={(e) => e.stopPropagation()}
          className="animate-slide-in-right flex h-full flex-col rounded-xl border border-border bg-card shadow-sm outline-none"
        >
          {/* Fixed header */}
          <div className="shrink-0 flex items-start justify-between border-b border-border px-5 py-4">
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                {selectedItem.icon_url && <img src={selectedItem.icon_url} alt={selectedItem.name} loading="lazy" decoding="async" className="h-6 w-6 rounded" />}
                <h3 className="text-lg font-semibold">{selectedItem.name}</h3>
                {selectedItem.verified && <BadgeCheck size={16} className="shrink-0 text-primary" />}
              </div>
              {selectedItem.source && (
                <a
                  href={
                    selectedItem.repo_url
                      ?? (selectedItem.kind === "mcp"
                        ? `https://smithery.ai/server/${selectedItem.source}`
                        : `https://github.com/${selectedItem.source}`)
                  }
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mt-1 inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                >
                  {selectedItem.source}
                  <ExternalLink size={10} className="shrink-0" />
                </a>
              )}
              <p className="mt-1 text-xs text-muted-foreground/70">
                {selectedItem.kind === "cli" && selectedItem.stars != null ? (
                  <><Star size={10} className="inline -mt-0.5 mr-0.5" />{formatInstalls(selectedItem.stars)}</>
                ) : (
                  <>{formatInstalls(selectedItem.installs)} uses</>
                )}
              </p>
            </div>
            <button onClick={closePreview} aria-label="Close details" className="shrink-0 rounded-lg p-2.5 text-muted-foreground hover:text-foreground">
              <X size={18} />
            </button>
          </div>

          {/* Scrollable body */}
          <div className="flex-1 min-h-0 overflow-y-auto overscroll-contain px-5 py-4">
          {selectedItem.description && (
            <p className="text-sm text-muted-foreground">{selectedItem.description}</p>
          )}

          {selectedItem.categories.length > 0 && (
            <div className="mt-3 flex flex-wrap gap-1">
              {selectedItem.categories.map((c) => (
                <span key={c} className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-accent">{c}</span>
              ))}
            </div>
          )}

          {/* MCP install guidance */}
          {selectedItem.kind === "mcp" && (
            <div className="mt-4 rounded-lg border border-primary/20 bg-primary/5 p-4">
              <p className="text-sm font-medium text-foreground">Install this MCP server</p>
              <p className="mt-1 text-xs text-muted-foreground">
                Visit Smithery for setup instructions, configuration options, and connection details.
              </p>
              <a
                href={`https://smithery.ai/server/${selectedItem.source}`}
                target="_blank"
                rel="noopener noreferrer"
                className="mt-3 inline-flex items-center gap-1.5 rounded-lg bg-primary px-3.5 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90"
              >
                <Server size={12} />
                Set up on Smithery
                <ExternalLink size={10} />
              </a>
            </div>
          )}

          {/* Security Audit (skills only) */}
          {selectedItem.kind === "skill" && (
            <div className="mt-4">
              <h4 className="mb-2 border-b border-border pb-1 text-xs font-medium text-muted-foreground">Security Audit</h4>
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
              <h4 className="mb-2 border-b border-border pb-1 text-xs font-medium text-muted-foreground">Install to Agent</h4>
              <div className="flex flex-wrap gap-1.5" aria-live="polite">
                {detectedAgents.map((agent) => {
                  const key = `${selectedItem.id}:${agent.name}`;
                  const isInstalled = installed.has(key);
                  const isFlashing = justInstalled.has(key);
                  return (
                    <button
                      key={agent.name}
                      disabled={installing === selectedItem.id || isInstalled}
                      onClick={() => handleInstall(selectedItem, agent.name)}
                      className={clsx(
                        "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition-[background-color,border-color] duration-300",
                        isFlashing
                          ? "border-primary/40 bg-primary/20 text-foreground"
                          : isInstalled
                            ? "border-primary/20 bg-primary/10 text-foreground"
                            : "border-border bg-primary/10 text-foreground hover:bg-primary/20 hover:border-ring",
                        (installing === selectedItem.id || isInstalled) && "disabled:opacity-50"
                      )}
                    >
                      {isInstalled ? (
                        <ShieldCheck size={12} className="animate-scale-in text-primary" />
                      ) : installing === selectedItem.id ? (
                        <Loader2 size={12} className="animate-spin" />
                      ) : (
                        <Download size={12} />
                      )}
                      {isInstalled ? (
                        <span className="install-success-text">Installed</span>
                      ) : (
                        agentDisplayName(agent.name)
                      )}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* SKILL.md content (skills only) */}
          {selectedItem.kind === "skill" && (
            <div className="mt-4">
              <h4 className="mb-2 border-b border-border pb-1 text-xs font-medium text-muted-foreground">Documentation</h4>
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
        </div>
        </div>
      )}
      </div>
    </div>
  );
}
