import { FileSearch, FolderPlus, FolderSearch, Plus, X } from "lucide-react";
import { useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useScope } from "@/hooks/use-scope";
import { openDirectoryPicker, openFilePicker } from "@/lib/dialog";
import { isDesktop } from "@/lib/transport";
import {
  agentDisplayName,
  type ConfigCategory,
  type ConfigScope,
  type ExtensionCounts,
  scopeLabel,
} from "@/lib/types";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { useExtensionStore } from "@/stores/extension-store";
import { ConfigSection } from "./config-section";
import { ExtensionsSummaryCard } from "./extensions-summary-card";
import { SectionAnchorRail } from "./section-anchor-rail";

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
  const { scope } = useScope();
  const agent = agentDetails.find((a) => a.name === selectedAgent);
  const [showAddForm, setShowAddForm] = useState(false);
  const [customPath, setCustomPath] = useState("");

  const matchesScope = (s: ConfigScope) => {
    if (scope.type === "all") return true;
    if (scope.type === "global") return s.type === "global";
    // scope.type === "project"
    return s.type === "project" && s.path === scope.path;
  };

  // Client-side compute scope-filtered counts for THIS agent so the summary
  // card reflects the global scope rather than the system-wide Rust totals.
  // The scope check is inlined (rather than calling matchesScope) so the
  // useMemo dependency list stays minimal — `scope` is the only reactive input
  // beyond `allExtensions` and `agent`.
  const scopedCounts = useMemo<ExtensionCounts>(() => {
    const c: ExtensionCounts = { skill: 0, mcp: 0, plugin: 0, hook: 0, cli: 0 };
    if (!agent) return c;
    for (const ext of allExtensions) {
      if (!ext.agents.includes(agent.name)) continue;
      const s = ext.scope;
      if (scope.type === "global" && s.type !== "global") continue;
      if (
        scope.type === "project" &&
        !(s.type === "project" && s.path === scope.path)
      ) {
        continue;
      }
      c[ext.kind] = (c[ext.kind] ?? 0) + 1;
    }
    return c;
  }, [allExtensions, agent, scope]);

  if (!agent) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
        Select an agent to view its configuration
      </div>
    );
  }

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

  // Scope-aware empty state: when scoped to a specific project and the agent
  // has no config files in that scope, render a focused empty card instead of
  // a stack of empty section headers.
  const totalVisible = nonCustomFiles.length + customFiles.length;
  const isProjectScopeEmpty =
    scope.type === "project" && totalVisible === 0;

  const summaryActiveScope =
    scope.type === "all"
      ? null
      : scope.type === "global"
        ? "global"
        : scope.path;

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

      {isProjectScopeEmpty ? (
        <div className="m-4 rounded-xl border border-dashed p-6 text-center">
          <p className="text-sm font-medium">
            {agentDisplayName(agent.name)} has no configuration in{" "}
            {scopeLabel(scope as ConfigScope)}
          </p>
        </div>
      ) : (
        <>
          {CATEGORY_ORDER.map((cat) => {
            const files = byCategory.get(cat) ?? [];
            // When the active scope hides everything in a category, collapse
            // the section instead of rendering a "0" header. Always show
            // categories when scope is "all" so empty categories render once
            // across scopes.
            if (scope.type !== "all" && files.length === 0) return null;
            return (
              <ConfigSection
                key={cat}
                category={cat}
                files={files}
                agentName={agent.name}
              />
            );
          })}
          {customFiles.length > 0 && (
            <ConfigSection
              key="custom"
              category={"custom" as ConfigCategory}
              files={customFiles}
              agentName={agent.name}
            />
          )}
        </>
      )}
      <ExtensionsSummaryCard
        counts={scopedCounts}
        agentName={agent.name}
        activeScope={summaryActiveScope}
      />
      <SectionAnchorRail
        revisionKey={`${agent.name}|${summaryActiveScope ?? "all"}|${nonCustomFiles.length}|${customFiles.length}`}
      />
    </div>
  );
}
