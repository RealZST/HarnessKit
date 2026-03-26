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
    <div className="flex gap-6">
      <div className="w-56 space-y-2">
        <h3 className="text-sm font-medium text-zinc-400 mb-3">Detected Agents</h3>
        {agents.map((agent) => (
          <button
            key={agent.name}
            onClick={() => setSelected(selected === agent.name ? null : agent.name)}
            className={clsx(
              "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm transition-colors",
              selected === agent.name
                ? "bg-zinc-800 text-zinc-100"
                : "text-zinc-400 hover:bg-zinc-900"
            )}
          >
            <Bot size={16} />
            <span className="flex-1 text-left">{agent.name}</span>
            {agent.detected ? (
              <Check size={14} className="text-green-400" />
            ) : (
              <X size={14} className="text-zinc-600" />
            )}
          </button>
        ))}
      </div>
      <div className="flex-1">
        <h2 className="text-xl font-semibold mb-4">
          {selected ? `${selected} Extensions` : "All Extensions"}
        </h2>
        <ExtensionTable data={filteredExtensions} />
      </div>
    </div>
  );
}
