import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Plus, X, FolderPlus, FolderSearch, FileSearch } from "lucide-react";
import { agentDisplayName, type ConfigCategory } from "@/lib/types";
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

  if (!agent) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
        Select an agent to view its configuration
      </div>
    );
  }

  const customFiles = agent.config_files.filter((f) => f.custom_id != null);
  const nonCustomFiles = agent.config_files.filter((f) => f.custom_id == null);
  const byCategory = new Map<ConfigCategory, typeof agent.config_files>();
  for (const cat of CATEGORY_ORDER) byCategory.set(cat, []);
  for (const file of nonCustomFiles) {
    const list = byCategory.get(file.category);
    if (list) list.push(file);
  }

  const scopes = new Set<string>();
  for (const file of agent.config_files) {
    scopes.add(file.scope.type === "global" ? "Global" : file.scope.name);
  }

  return (
    <div className="flex-1 overflow-y-auto overscroll-contain p-5">
      <div className="flex items-start justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">{agentDisplayName(agent.name)}</h2>
          {!agent.detected && (
            <p className="text-[12px] text-muted-foreground mt-0.5">Not detected</p>
          )}
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
          <button
            onClick={() => setShowAddForm(true)}
            className="flex items-center gap-1 text-[11px] px-2 py-0.5 rounded-md border border-dashed border-border text-muted-foreground hover:bg-muted/50 transition-colors"
          >
            <FolderPlus size={10} />
            Add Custom Path
          </button>
        </div>
      </div>

      {/* Add Custom Path form */}
      {showAddForm && (
        <div className="mb-5 rounded-lg border border-border p-3 space-y-2.5">
          <div className="flex items-center justify-between">
            <span className="text-[12px] font-medium text-foreground">Add Custom Path</span>
            <button onClick={() => { setShowAddForm(false); setCustomPath(""); }} className="text-muted-foreground hover:text-foreground">
              <X size={14} />
            </button>
          </div>
          <div className="flex items-center gap-1.5">
            <input
              type="text"
              placeholder="Paste a file or folder path..."
              value={customPath}
              onChange={(e) => setCustomPath(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && customPath.trim()) {
                  addCustomPath(agent.name, customPath.trim(), "", "settings");
                  setShowAddForm(false);
                  setCustomPath("");
                }
              }}
              className="flex-1 rounded-md border border-border bg-card px-3 py-1.5 text-[12px] placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <button
              onClick={async () => {
                try {
                  const { open } = await import("@tauri-apps/plugin-dialog");
                  const selected = await open({ title: "Select file" });
                  if (typeof selected === "string") setCustomPath(selected);
                } catch {}
              }}
              className="shrink-0 rounded-md border border-border bg-card px-2.5 py-1.5 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              title="Browse file..."
            >
              <FileSearch size={14} />
            </button>
            <button
              onClick={async () => {
                try {
                  const { open } = await import("@tauri-apps/plugin-dialog");
                  const selected = await open({ directory: true, title: "Select folder" });
                  if (typeof selected === "string") setCustomPath(selected);
                } catch {}
              }}
              className="shrink-0 rounded-md border border-border bg-card px-2.5 py-1.5 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
              title="Browse folder..."
            >
              <FolderSearch size={14} />
            </button>
            <button
              disabled={!customPath.trim()}
              onClick={async () => {
                await addCustomPath(agent.name, customPath.trim(), "", "settings");
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
      {customFiles.length > 0 && (
        <ConfigSection key="custom" category={"custom" as ConfigCategory} files={customFiles} />
      )}
      <ExtensionsSummaryCard counts={agent.extension_counts} agentName={agent.name} />
    </div>
  );
}
