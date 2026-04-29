import { FileSearch, FolderPlus, FolderSearch, Plus, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { openDirectoryPicker, openFilePicker } from "@/lib/dialog";
import { isDesktop } from "@/lib/transport";
import {
  agentDisplayName,
  type ConfigCategory,
  type ConfigScope,
  type ExtensionCounts,
  scopeKey,
  scopeLabel,
} from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { useExtensionStore } from "@/stores/extension-store";
import { ConfigSection } from "./config-section";
import { ExtensionsSummaryCard } from "./extensions-summary-card";

const CATEGORY_ORDER: ConfigCategory[] = [
  "settings",
  "workflow",
  "rules",
  "memory",
  "ignore",
];

export function AgentDetail() {
  const navigate = useNavigate();
  const agentDetails = useAgentConfigStore((s) => s.agentDetails);
  const selectedAgent = useAgentConfigStore((s) => s.selectedAgent);
  const addCustomPath = useAgentConfigStore((s) => s.addCustomPath);
  const allExtensions = useExtensionStore((s) => s.extensions);
  const agent = agentDetails.find((a) => a.name === selectedAgent);
  const [showAddForm, setShowAddForm] = useState(false);
  const [customPath, setCustomPath] = useState("");
  /** Active scope filter: null = no filter, otherwise a `scopeKey` value. */
  const [activeScope, setActiveScope] = useState<string | null>(null);

  // Reset filter whenever the user switches to a different agent.
  useEffect(() => {
    setActiveScope(null);
  }, [selectedAgent]);

  if (!agent) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
        Select an agent to view its configuration
      </div>
    );
  }

  // Collect all distinct scopes seen in this agent's config files; preserve
  // first-seen order so "Global" stays leading and projects keep stable order.
  const distinctScopes: ConfigScope[] = [];
  const seenScopeKeys = new Set<string>();
  for (const file of agent.config_files) {
    const key = scopeKey(file.scope);
    if (!seenScopeKeys.has(key)) {
      seenScopeKeys.add(key);
      distinctScopes.push(file.scope);
    }
  }

  const matchesScope = (s: ConfigScope) =>
    activeScope === null || scopeKey(s) === activeScope;

  const customFiles = agent.config_files.filter(
    (f) => f.custom_id != null && matchesScope(f.scope),
  );
  const nonCustomFiles = agent.config_files.filter(
    (f) => f.custom_id == null && matchesScope(f.scope),
  );
  const byCategory = new Map<ConfigCategory, typeof agent.config_files>();
  for (const cat of CATEGORY_ORDER) byCategory.set(cat, []);
  for (const file of nonCustomFiles) {
    const list = byCategory.get(file.category);
    if (list) list.push(file);
  }

  // Recompute extension counts honoring the scope filter so the summary card
  // stays in sync with the rest of the page. When no filter is active we trust
  // the backend-provided totals (cheaper and matches existing behavior).
  const filteredExtensionCounts: ExtensionCounts = useMemo(() => {
    if (activeScope === null) return agent.extension_counts;
    const counts = { skill: 0, mcp: 0, plugin: 0, hook: 0, cli: 0 };
    for (const ext of allExtensions) {
      if (!ext.agents.includes(agent.name)) continue;
      if (scopeKey(ext.scope) !== activeScope) continue;
      counts[ext.kind] += 1;
    }
    return counts;
  }, [activeScope, allExtensions, agent.name, agent.extension_counts]);

  return (
    <div className="flex-1 overflow-y-auto overscroll-contain p-5">
      <div className="flex items-start justify-between mb-6">
        <div>
          <h2 className="text-2xl font-bold tracking-tight">
            {agentDisplayName(agent.name)}
          </h2>
          {!agent.detected && (
            <p className="text-[12px] text-muted-foreground mt-0.5">
              Not detected
            </p>
          )}
        </div>
        <div className="flex gap-1.5">
          {distinctScopes.map((scope) => {
            const key = scopeKey(scope);
            const isActive = activeScope === key;
            return (
              <button
                key={key}
                onClick={() => setActiveScope(isActive ? null : key)}
                aria-pressed={isActive}
                title={
                  isActive
                    ? `Showing only ${scopeLabel(scope)} — click to clear`
                    : `Filter by ${scopeLabel(scope)}`
                }
                className={
                  "text-[11px] px-2 py-0.5 rounded-md border transition-colors " +
                  (isActive
                    ? "border-primary bg-primary/15 text-primary"
                    : "border-border bg-muted/50 hover:bg-muted")
                }
              >
                {scopeLabel(scope)}
              </button>
            );
          })}
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
            <span className="text-[12px] font-medium text-foreground">
              Add Custom Path
            </span>
            <button
              onClick={() => {
                setShowAddForm(false);
                setCustomPath("");
              }}
              className="text-muted-foreground hover:text-foreground"
            >
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
            {isDesktop() && (
              <button
                onClick={async () => {
                  const selected = await openFilePicker({
                    title: "Select file",
                  });
                  if (selected) setCustomPath(selected);
                }}
                className="shrink-0 rounded-md border border-border bg-card px-2.5 py-1.5 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                title="Browse file..."
              >
                <FileSearch size={14} />
              </button>
            )}
            {isDesktop() && (
              <button
                onClick={async () => {
                  const selected = await openDirectoryPicker({
                    title: "Select folder",
                  });
                  if (selected) setCustomPath(selected);
                }}
                className="shrink-0 rounded-md border border-border bg-card px-2.5 py-1.5 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                title="Browse folder..."
              >
                <FolderSearch size={14} />
              </button>
            )}
            <button
              disabled={!customPath.trim()}
              onClick={async () => {
                await addCustomPath(
                  agent.name,
                  customPath.trim(),
                  "",
                  "settings",
                );
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

      {CATEGORY_ORDER.map((cat) => {
        const files = byCategory.get(cat) ?? [];
        // When a scope filter is active, hide sections that have nothing in
        // the selected scope so the page collapses cleanly instead of
        // showing rows of "0" headers.
        if (activeScope !== null && files.length === 0) return null;
        return <ConfigSection key={cat} category={cat} files={files} />;
      })}
      {customFiles.length > 0 && (
        <ConfigSection
          key="custom"
          category={"custom" as ConfigCategory}
          files={customFiles}
        />
      )}
      <ExtensionsSummaryCard
        counts={filteredExtensionCounts}
        agentName={agent.name}
      />
    </div>
  );
}
