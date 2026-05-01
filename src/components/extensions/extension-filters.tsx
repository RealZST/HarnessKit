import { clsx } from "clsx";
import { Search, X } from "lucide-react";
import { useEffect, useMemo } from "react";
import { agentDisplayName, type ExtensionKind, sortAgents } from "@/lib/types";
import { isWeb as web, webSelectStyle } from "@/lib/web-select";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { useScopeStore } from "@/stores/scope-store";

const TAG_COLORS = [
  "bg-primary/10 text-primary",
  "bg-chart-1/10 text-chart-1",
  "bg-chart-2/10 text-chart-2",
  "bg-chart-3/10 text-chart-3",
  "bg-chart-4/10 text-chart-4",
  "bg-chart-5/10 text-chart-5",
  "bg-secondary/20 text-secondary-foreground",
  "bg-accent text-accent-foreground",
];

export function tagColor(index: number): string {
  return TAG_COLORS[index % TAG_COLORS.length];
}

const kinds: (ExtensionKind | null)[] = [
  null,
  "skill",
  "mcp",
  "plugin",
  "hook",
  "cli",
];
const kindLabel: Record<ExtensionKind, string> = {
  skill: "skill",
  mcp: "MCP",
  plugin: "plugin",
  hook: "hook",
  cli: "CLI",
};

/** Per-agent background + text colors for the active filter state. */
const AGENT_FILTER_COLORS: Record<string, string> = {
  claude: "bg-agent-claude/15 text-agent-claude border-agent-claude/30",
  codex: "bg-agent-codex/15 text-agent-codex border-agent-codex/30",
  gemini: "bg-agent-gemini/15 text-agent-gemini border-agent-gemini/30",
  cursor: "bg-agent-cursor/15 text-agent-cursor border-agent-cursor/30",
  antigravity:
    "bg-agent-antigravity/15 text-agent-antigravity border-agent-antigravity/30",
  copilot: "bg-agent-copilot/15 text-agent-copilot border-agent-copilot/30",
  windsurf: "bg-agent-windsurf/15 text-agent-windsurf border-agent-windsurf/30",
};

export function ExtensionFilters() {
  const kindFilter = useExtensionStore((s) => s.kindFilter);
  const setKindFilter = useExtensionStore((s) => s.setKindFilter);
  const agentFilter = useExtensionStore((s) => s.agentFilter);
  const setAgentFilter = useExtensionStore((s) => s.setAgentFilter);
  const searchQuery = useExtensionStore((s) => s.searchQuery);
  const setSearchQuery = useExtensionStore((s) => s.setSearchQuery);
  const packFilter = useExtensionStore((s) => s.packFilter);
  const setPackFilter = useExtensionStore((s) => s.setPackFilter);
  const extensions = useExtensionStore((s) => s.extensions);
  const grouped = useExtensionStore((s) => s.grouped);
  const filtered = useExtensionStore((s) => s.filtered);
  const scope = useScopeStore((s) => s.current);
  // Source dropdown options + counts are scoped: a project shouldn't show
  // packs that only exist globally (and vice versa). We deliberately don't
  // narrow by kind/agent/tag/search — those filter the rows further; the
  // dropdown options should stay stable as the user toggles them.
  const { scopedPacks, packCounts } = useMemo(() => {
    const counts = new Map<string, number>();
    for (const g of grouped()) {
      if (!g.pack) continue;
      if (scope.type !== "all") {
        const targetKey =
          scope.type === "global" ? "global" : scope.path;
        const matches = g.instances.some((i) => {
          const k = i.scope.type === "global" ? "global" : i.scope.path;
          return k === targetKey;
        });
        if (!matches) continue;
      }
      counts.set(g.pack, (counts.get(g.pack) ?? 0) + 1);
    }
    return {
      scopedPacks: [...counts.keys()].sort(),
      packCounts: counts,
    };
  }, [grouped, extensions, scope]);
  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const enabledAgents = useMemo(
    () =>
      sortAgents(
        agents.filter((a) => a.enabled),
        agentOrder,
      ),
    [agents, agentOrder],
  );
  const resultCount = filtered().length;

  // Clear packFilter when the selected pack no longer exists in the current
  // scope — otherwise the dropdown shows a stale value not in options and
  // results read empty.
  useEffect(() => {
    if (packFilter && !scopedPacks.includes(packFilter)) {
      setPackFilter(null);
    }
  }, [packFilter, scopedPacks, setPackFilter]);

  return (
    <div className="space-y-2.5">
      {/* Filters: kind pills + result count + dropdowns + search */}
      <div className="flex items-center gap-2">
        {kinds.map((kind) => (
          <button
            key={kind ?? "all"}
            onClick={() => setKindFilter(kind)}
            aria-pressed={kindFilter === kind}
            className={clsx(
              "shrink-0 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
              kindFilter === kind
                ? "bg-primary text-primary-foreground"
                : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground",
            )}
          >
            {kind ? kindLabel[kind] : "All"}
          </button>
        ))}
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {resultCount} result{resultCount !== 1 ? "s" : ""}
        </span>
        {(kindFilter || agentFilter || packFilter || searchQuery) && (
          <button
            onClick={() => {
              setKindFilter(null);
              setAgentFilter(null);
              setPackFilter(null);
              setSearchQuery("");
            }}
            className="shrink-0 rounded-md bg-muted/60 px-2 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
          >
            Clear filters
          </button>
        )}
        <div className="flex-1" />
        {enabledAgents.length > 0 && (
          <select
            value={agentFilter ?? ""}
            onChange={(e) => setAgentFilter(e.target.value || null)}
            aria-label="Filter by agent"
            style={webSelectStyle}
            className={clsx(
              "shrink-0 border px-3 text-xs capitalize focus:outline-none transition-colors",
              web ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
              agentFilter && AGENT_FILTER_COLORS[agentFilter]
                ? `${AGENT_FILTER_COLORS[agentFilter]}${web ? " font-medium" : ""}`
                : "border-border bg-card text-foreground focus:border-ring",
            )}
          >
            <option value="">All Agents</option>
            {enabledAgents.map((agent) => (
              <option key={agent.name} value={agent.name}>
                {agentDisplayName(agent.name)}
              </option>
            ))}
          </select>
        )}
        {scopedPacks.length > 0 && (
          <select
            value={packFilter ?? ""}
            onChange={(e) => setPackFilter(e.target.value || null)}
            aria-label="Filter by source"
            style={webSelectStyle}
            className={clsx(
              "w-36 shrink-0 overflow-hidden text-ellipsis border border-border bg-card px-3 text-xs text-foreground focus:border-ring focus:outline-none",
              web ? "rounded-[6px] h-[26px]" : "rounded-lg py-1.5",
            )}
          >
            <option value="">All Sources</option>
            {scopedPacks.map((pack) => (
              <option key={pack} value={pack}>
                {pack} ({packCounts.get(pack) ?? 0})
              </option>
            ))}
          </select>
        )}
        <div className="relative shrink-0 w-44">
          <Search
            size={14}
            className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground"
          />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search..."
            title="Search by name or description"
            aria-label="Search extensions"
            className="w-full rounded-lg border border-border bg-card py-1.5 pl-8 pr-8 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery("")}
              aria-label="Clear search"
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X size={14} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
