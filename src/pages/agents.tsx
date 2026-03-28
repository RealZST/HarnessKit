import { useEffect, useMemo, useState } from "react";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { Bot, Check, X, Package, Server, Puzzle, Webhook, AlertCircle } from "lucide-react";
import { clsx } from "clsx";
import { sortAgents, agentDisplayName, type ExtensionKind } from "@/lib/types";

const kindIcons: Record<ExtensionKind, React.ElementType> = {
  skill: Package,
  mcp: Server,
  plugin: Puzzle,
  hook: Webhook,
};

const kindLabels: Record<ExtensionKind, string> = {
  skill: "skills",
  mcp: "MCP",
  plugin: "plugins",
  hook: "hooks",
};

export default function AgentsPage() {
  const { agents, fetch: fetchAgents } = useAgentStore();
  const { extensions, fetch: fetchExtensions } = useExtensionStore();
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    fetchAgents();
    fetchExtensions();
  }, [fetchAgents, fetchExtensions]);

  // Count extensions per agent
  const extensionsByAgent = useMemo(() => {
    const map = new Map<string, number>();
    for (const agent of agents) {
      map.set(
        agent.name,
        extensions.filter((e) => e.agents.includes(agent.name)).length,
      );
    }
    return map;
  }, [agents, extensions]);

  const filteredExtensions = selected
    ? extensions.filter((e) => e.agents.includes(selected))
    : extensions;

  // Kind breakdown for the selected agent's extensions
  const kindBreakdown = useMemo(() => {
    const counts: Record<ExtensionKind, number> = {
      skill: 0,
      mcp: 0,
      plugin: 0,
      hook: 0,
    };
    for (const ext of filteredExtensions) {
      counts[ext.kind]++;
    }
    return counts;
  }, [filteredExtensions]);

  // Page summary stats
  const detectedCount = agents.filter((a) => a.detected).length;
  const totalExtensions = extensions.length;

  // Selected agent info
  const selectedAgent = agents.find((a) => a.name === selected);

  return (
    <div className="animate-fade-in flex flex-1 flex-col min-h-0 -mb-6">
      {/* Page summary — fixed at top */}
      {agents.length > 0 && (
        <p className="shrink-0 pb-4 text-sm text-muted-foreground">
          <span className="font-medium text-foreground">{detectedCount}</span>
          {" "}
          {detectedCount === 1 ? "agent" : "agents"} detected
          <span className="text-border mx-2">·</span>
          <span className="font-medium text-foreground">{totalExtensions}</span>
          {" "}
          {totalExtensions === 1 ? "extension" : "extensions"} across{" "}
          <span className="font-medium text-foreground">{detectedCount}</span>
          {" "}
          {detectedCount === 1 ? "agent" : "agents"}
        </p>
      )}

      <div className="flex flex-1 min-h-0 flex-col sm:flex-row gap-6">
        {/* Agent sidebar — independently scrollable */}
        <div className="w-full sm:w-64 shrink-0 overflow-y-auto space-y-2">
          <h3 className="text-sm font-medium text-muted-foreground mb-1 border-b border-border pb-2">
            Agents
          </h3>
          <div className="animate-fade-in space-y-1">
            {sortAgents(agents).map((agent) => {
              const extCount = extensionsByAgent.get(agent.name) ?? 0;
              const isSelected = selected === agent.name;

              return (
                <button
                  key={agent.name}
                  onClick={() =>
                    setSelected(isSelected ? null : agent.name)
                  }
                  aria-pressed={isSelected}
                  className={clsx(
                    "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors duration-200",
                    isSelected
                      ? "border-l-2 border-l-primary bg-accent text-accent-foreground"
                      : "border-l-2 border-l-transparent text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  <Bot size={16} className="shrink-0" />
                  <div className="flex-1 min-w-0 text-left">
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate">{agentDisplayName(agent.name)}</span>
                      <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
                        {extCount} ext
                      </span>
                    </div>
                    <span
                      className={clsx(
                        "text-xs",
                        agent.detected
                          ? "text-primary"
                          : "text-muted-foreground",
                      )}
                    >
                      {agent.detected ? "Active" : "Not found"}
                    </span>
                  </div>
                  {agent.detected ? (
                    <Check
                      size={16}
                      className="shrink-0 text-primary transition-colors duration-200"
                    />
                  ) : (
                    <X
                      size={16}
                      className="shrink-0 text-muted-foreground transition-colors duration-200"
                    />
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* Main content — independently scrollable */}
        <div className="flex-1 min-w-0 overflow-y-auto">
          <h2 className="text-2xl font-bold tracking-tight mb-4 select-none">
            {selected ? `${selected} Extensions` : "All Extensions"}
          </h2>

          {/* Empty state for undetected agent */}
          {selected && selectedAgent && !selectedAgent.detected ? (
            <div className="rounded-xl border border-dashed border-border bg-card/30 px-6 py-10 text-center">
              <AlertCircle
                size={32}
                className="mx-auto text-muted-foreground/40"
                aria-hidden="true"
              />
              <h3 className="mt-3 text-base font-medium text-foreground">
                Agent not detected
              </h3>
              <p className="mt-1 text-sm text-muted-foreground">
                Install{" "}
                <span className="font-medium text-foreground">
                  {selectedAgent.name}
                </span>{" "}
                to manage its extensions here.
              </p>
            </div>
          ) : (
            <>
              {/* Extension breakdown by kind */}
              {selected && filteredExtensions.length > 0 && (
                <div className="flex flex-wrap items-center gap-x-4 gap-y-1 mb-4">
                  {(
                    Object.entries(kindBreakdown) as [ExtensionKind, number][]
                  ).map(([kind, count]) => {
                    if (count === 0) return null;
                    const Icon = kindIcons[kind];
                    return (
                      <span
                        key={kind}
                        className="inline-flex items-center gap-1.5 text-sm text-muted-foreground"
                      >
                        <Icon
                          size={14}
                          strokeWidth={1.75}
                          className="text-muted-foreground/60"
                          aria-hidden="true"
                        />
                        <span className="tabular-nums font-medium text-foreground">
                          {count}
                        </span>
                        <span>{kindLabels[kind]}</span>
                      </span>
                    );
                  })}
                </div>
              )}

              <ExtensionTable data={filteredExtensions} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
