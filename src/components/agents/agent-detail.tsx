import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Plus, X, FolderPlus } from "lucide-react";
import { agentDisplayName, type ConfigCategory, CONFIG_CATEGORY_LABELS } from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { ConfigSection } from "./config-section";
import { ExtensionsSummaryCard } from "./extensions-summary-card";

const CATEGORY_ORDER: ConfigCategory[] = ["rules", "memory", "settings", "ignore"];


export function AgentDetail() {
  const navigate = useNavigate();
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const addCustomPath = useAgentConfigStore((s) => s.addCustomPath);
  const agent = agentDetails.find((a) => a.name === selectedAgent);
  const [showAddForm, setShowAddForm] = useState(false);
  const [customPath, setCustomPath] = useState("");
  const [customCategory, setCustomCategory] = useState<ConfigCategory>("settings");

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

      {/* Add Custom Path */}
      {!showAddForm ? (
        <button
          onClick={() => setShowAddForm(true)}
          className="flex items-center gap-1.5 mb-5 px-3 py-1.5 text-[12px] text-muted-foreground hover:text-foreground border border-dashed border-border rounded-lg hover:bg-muted/50 transition-colors"
        >
          <FolderPlus size={13} />
          Add Custom Path
        </button>
      ) : (
        <div className="mb-5 rounded-lg border border-border p-3 space-y-2.5">
          <div className="flex items-center justify-between">
            <span className="text-[12px] font-medium text-foreground">Add Custom Path</span>
            <button onClick={() => { setShowAddForm(false); setCustomPath(""); }} className="text-muted-foreground hover:text-foreground">
              <X size={14} />
            </button>
          </div>
          <input
            type="text"
            placeholder="Path (e.g. ~/.claude/my-config.json)"
            value={customPath}
            onChange={(e) => setCustomPath(e.target.value)}
            className="w-full rounded-md border border-border bg-card px-3 py-1.5 text-[12px] placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          />
          <div className="flex items-center gap-2">
            <select
              value={customCategory}
              onChange={(e) => setCustomCategory(e.target.value as ConfigCategory)}
              className="rounded-md border border-border bg-card px-2.5 py-1.5 text-[12px] focus:outline-none focus:ring-1 focus:ring-ring"
            >
              {CATEGORY_ORDER.map((cat) => (
                <option key={cat} value={cat}>{CONFIG_CATEGORY_LABELS[cat]}</option>
              ))}
            </select>
            <div className="flex-1" />
            <button
              disabled={!customPath.trim()}
              onClick={async () => {
                await addCustomPath(agent.name, customPath.trim(), "", customCategory);
                setShowAddForm(false);
                setCustomPath("");
              }}
              className="rounded-md bg-primary px-3 py-1.5 text-[12px] font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-40"
            >
              Add
            </button>
          </div>
        </div>
      )}

      {CATEGORY_ORDER.map((cat) => (
        <ConfigSection key={cat} category={cat} files={byCategory.get(cat) ?? []} />
      ))}
      <ExtensionsSummaryCard counts={agent.extension_counts} agentName={agent.name} />
    </div>
  );
}
