import { useEffect, useState } from "react";
import { useAgentStore } from "@/stores/agent-store";
import { useExtensionStore } from "@/stores/extension-store";
import { ExtensionTable } from "@/components/extensions/extension-table";
import { Bot, Check, X } from "lucide-react";
import { clsx } from "clsx";

export default function AgentsPage() {
  const { agents, fetch: fetchAgents } = useAgentStore();
  const { extensions, fetch: fetchExtensions } = useExtensionStore();
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    fetchAgents();
    fetchExtensions();
  }, [fetchAgents, fetchExtensions]);

  const filteredExtensions = selected
    ? extensions.filter((e) => e.agents.includes(selected))
    : extensions;

  return (
    <div className="flex flex-col sm:flex-row gap-6">
      <div className="w-full sm:w-56 space-y-2">
        <h3 className="text-sm font-medium text-muted-foreground mb-1 border-b border-border pb-2">Detected Agents</h3>
        <p className="text-xs text-muted-foreground mb-3">Agents found on this machine</p>
        <div className="animate-fade-in space-y-1">
          {agents.map((agent) => (
            <button
              key={agent.name}
              onClick={() => setSelected(selected === agent.name ? null : agent.name)}
              aria-pressed={selected === agent.name}
              className={clsx(
                "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors duration-200",
                selected === agent.name
                  ? "border-l-2 border-l-primary bg-accent text-accent-foreground"
                  : "border-l-2 border-l-transparent text-muted-foreground hover:bg-muted hover:text-foreground"
              )}
            >
              <Bot size={16} />
              <span className="flex-1 text-left">{agent.name}</span>
              {agent.detected ? (
                <Check size={16} className="text-primary transition-colors duration-200" />
              ) : (
                <X size={16} className="text-muted-foreground transition-colors duration-200" />
              )}
            </button>
          ))}
        </div>
      </div>
      <div className="flex-1">
        <h2 className="text-2xl font-bold tracking-tight mb-4">
          {selected ? `${selected} Extensions` : "All Extensions"}
        </h2>
        <ExtensionTable data={filteredExtensions} />
      </div>
    </div>
  );
}
