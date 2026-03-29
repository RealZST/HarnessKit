import { sortAgents, agentDisplayName, type ExtensionKind } from "@/lib/types";
import { useExtensionStore } from "@/stores/extension-store";
import { useAgentStore } from "@/stores/agent-store";
import { Search } from "lucide-react";
import { clsx } from "clsx";
import { useMemo } from "react";

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

export const CATEGORIES = [
  "Coding", "Testing", "DevOps", "Data", "Design",
  "Writing", "Education", "Finance", "Security",
  "Productivity", "Research", "Other",
] as const;

const kinds: (ExtensionKind | null)[] = [null, "skill", "mcp", "plugin", "hook"];

export function ExtensionFilters() {
  const { kindFilter, setKindFilter, agentFilter, setAgentFilter, searchQuery, setSearchQuery, categoryFilter, setCategoryFilter, filtered } = useExtensionStore();
  const agents = useAgentStore((s) => s.agents);
  const enabledAgents = useMemo(() => sortAgents(agents.filter((a) => a.enabled)), [agents]);
  const resultCount = filtered().length;

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
                : "bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            )}
          >
            {kind ?? "All"}
          </button>
        ))}
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {resultCount} result{resultCount !== 1 ? "s" : ""}
        </span>
        <div className="flex-1" />
        <select
          value={categoryFilter ?? ""}
          onChange={(e) => setCategoryFilter(e.target.value || null)}
          aria-label="Filter by category"
          className="shrink-0 rounded-lg border border-border bg-card px-3 py-1.5 text-xs text-foreground focus:border-ring focus:outline-none"
        >
          <option value="">All Categories</option>
          {CATEGORIES.map((cat) => (
            <option key={cat} value={cat}>{cat}</option>
          ))}
        </select>
        {enabledAgents.length > 0 && (
          <select
            value={agentFilter ?? ""}
            onChange={(e) => setAgentFilter(e.target.value || null)}
            aria-label="Filter by agent"
            className="shrink-0 rounded-lg border border-border bg-card px-3 py-1.5 text-xs text-foreground capitalize focus:border-ring focus:outline-none"
          >
            <option value="">All Agents</option>
            {enabledAgents.map((agent) => (
              <option key={agent.name} value={agent.name}>{agentDisplayName(agent.name)}</option>
            ))}
          </select>
        )}
        <div className="relative shrink-0 w-44">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search..."
            aria-label="Search extensions"
            className="w-full rounded-lg border border-border bg-card py-1.5 pl-8 pr-3 text-xs placeholder:text-muted-foreground focus:border-ring focus:outline-none"
          />
        </div>
      </div>

    </div>
  );
}
