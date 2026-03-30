import { useNavigate } from "react-router-dom";
import { Plus } from "lucide-react";
import { agentDisplayName, type ConfigCategory } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { ConfigSection } from "./config-section";
import { ExtensionsSummaryCard } from "./extensions-summary-card";

const CATEGORY_ORDER: ConfigCategory[] = ["rules", "memory", "settings", "ignore"];

export function AgentDetail() {
  const navigate = useNavigate();
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const agent = agentDetails.find((a) => a.name === selectedAgent);

  if (!agent) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
        Select an agent to view its configuration
      </div>
    );
  }

  const byCategory = new Map<ConfigCategory, typeof agent.config_files>();
  for (const cat of CATEGORY_ORDER) byCategory.set(cat, []);
  for (const file of agent.config_files) {
    const list = byCategory.get(file.category);
    if (list) list.push(file);
  }

  const scopes = new Set<string>();
  for (const file of agent.config_files) {
    scopes.add(file.scope.type === "global" ? "Global" : file.scope.name);
  }

  return (
    <div className="flex-1 overflow-y-auto p-5">
      <div className="flex items-start justify-between mb-6">
        <div>
          <h2 className="text-xl font-bold">{agentDisplayName(agent.name)}</h2>
          <p className="text-[12px] text-muted-foreground mt-0.5">
            {agent.detected ? "Detected" : "Not detected"}
          </p>
        </div>
        <div className="flex gap-1.5">
          {scopes.size > 0 && [...scopes].map((scope) => (
            <span key={scope} className="text-[11px] px-2 py-0.5 rounded-md border border-border bg-muted/50">
              {scope}
            </span>
          ))}
          <button
            onClick={() => navigate("/settings?scrollTo=project-paths")}
            className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-md border border-dashed border-border text-muted-foreground hover:bg-muted/50 transition-colors"
          >
            <Plus size={10} />
            Add Project
          </button>
        </div>
      </div>
      {CATEGORY_ORDER.map((cat) => (
        <ConfigSection key={cat} category={cat} files={byCategory.get(cat) ?? []} />
      ))}
      <ExtensionsSummaryCard counts={agent.extension_counts} agentName={agent.name} />
    </div>
  );
}
