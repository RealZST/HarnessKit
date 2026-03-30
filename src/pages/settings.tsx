import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useUIStore } from "@/stores/ui-store";
import type { ThemeName } from "@/stores/ui-store";
import { useProjectStore } from "@/stores/project-store";
import { useAgentStore } from "@/stores/agent-store";
import { KindBadge } from "@/components/shared/kind-badge";
import { FolderOpen, Plus, Trash2, Loader2, ChevronDown, ChevronRight, Pencil } from "lucide-react";
import { clsx } from "clsx";
import { api } from "@/lib/invoke";
import { agentDisplayName, type Extension, type ExtensionKind, type DiscoveredProject } from "@/lib/types";
import { toast } from "@/stores/toast-store";

async function openDirectoryPicker(title: string): Promise<string | null> {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ directory: true, title });
    if (typeof selected === "string") return selected;
    return null;
  } catch (e) {
    console.error("Dialog plugin not available:", e);
    return null;
  }
}

const kindLabels: Record<ExtensionKind, string> = {
  skill: "Skills",
  mcp: "MCP Servers",
  hook: "Hooks",
  plugin: "Plugins",
};

const kindOrder: ExtensionKind[] = ["skill", "mcp", "hook", "plugin"];

function groupByKind(extensions: Extension[]): Record<string, Extension[]> {
  const groups: Record<string, Extension[]> = {};
  for (const ext of extensions) {
    if (!groups[ext.kind]) groups[ext.kind] = [];
    groups[ext.kind].push(ext);
  }
  return groups;
}

const THEME_OPTIONS: { value: ThemeName; label: string; colors: [string, string, string] }[] = [
  { value: "tiesen", label: "Tiesen", colors: ["oklch(0.5144 0.1605 267.4400)", "oklch(0.9851 0 0)", "oklch(0 0 0)"] },
{ value: "claude", label: "Claude", colors: ["oklch(0.6171 0.1375 39.0427)", "oklch(0.9665 0.0067 97.3521)", "oklch(0.2679 0.0036 106.6427)"] },
];

export default function SettingsPage() {
  const { themeName, mode, setThemeName, setMode } = useUIStore();
  const {
    projects,
    selectedProject,
    projectExtensions,
    loading,
    extensionsLoading,
    loadProjects,
    addProject,
    removeProject,
    selectProject,
  } = useProjectStore();

  const { agents, fetch: fetchAgents, updatePath, setEnabled } = useAgentStore();
  const [searchParams, setSearchParams] = useSearchParams();

  const [adding, setAdding] = useState(false);
  const [discoveredProjects, setDiscoveredProjects] = useState<DiscoveredProject[] | null>(null);
  const [discoveredSelected, setDiscoveredSelected] = useState<Set<string>>(new Set());
  const [confirmingRemoveId, setConfirmingRemoveId] = useState<string | null>(null);
  const confirmRemoveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Auto-cancel project removal confirmation after 5 seconds
  useEffect(() => {
    if (confirmingRemoveId) {
      confirmRemoveTimerRef.current = setTimeout(() => setConfirmingRemoveId(null), 5000);
      return () => { if (confirmRemoveTimerRef.current) clearTimeout(confirmRemoveTimerRef.current); };
    }
  }, [confirmingRemoveId]);

  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  useEffect(() => {
    const scrollTo = searchParams.get("scrollTo");
    if (scrollTo) {
      const el = document.getElementById(scrollTo);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "start" });
        searchParams.delete("scrollTo");
        setSearchParams(searchParams, { replace: true });
      }
    }
  }, [searchParams, setSearchParams]);

  const agentOrder = useAgentStore((s) => s.agentOrder);
  const agentNames = agentOrder;
  const agentMap = new Map(agents.map((a) => [a.name.toLowerCase(), a]));

  const existingPaths = new Set(projects.map((p) => p.path));

  const handleAdd = async () => {
    const path = await openDirectoryPicker("Select Project Directory");
    if (!path) return;

    setAdding(true);
    try {
      // Try adding directly first (it's a project itself)
      await addProject(path);
      setDiscoveredProjects(null);
      toast.success("Project added");
    } catch {
      // Not a valid project — try discovering projects inside it
      try {
        const results = await api.discoverProjects(path);
        if (results.length > 0) {
          setDiscoveredProjects(results);
          setDiscoveredSelected(new Set());
        } else {
          toast.error("No projects found in directory");
        }
      } catch (e) {
        console.error("Failed to discover projects:", e);
        toast.error("Failed to discover projects");
      }
    } finally {
      setAdding(false);
    }
  };

  const handleAddDiscovered = async () => {
    setAdding(true);
    let added = 0;
    try {
      for (const path of discoveredSelected) {
        try { await addProject(path); added++; } catch {}
      }
      if (added > 0) toast.success(`${added} project${added > 1 ? "s" : ""} added`);
    } finally {
      setAdding(false);
      setDiscoveredProjects(null);
      setDiscoveredSelected(new Set());
    }
  };

  const toggleDiscovered = (path: string) => {
    setDiscoveredSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const grouped = useMemo(() => groupByKind(projectExtensions), [projectExtensions]);

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      <div className="shrink-0 pb-4">
        <h2 className="text-2xl font-bold tracking-tight select-none">Settings</h2>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto"><div className="max-w-2xl mx-auto space-y-8 pb-6">

      {/* Appearance */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-muted-foreground">Appearance</h3>

        <div className="flex flex-col gap-3 rounded-lg border border-border bg-card px-4 py-3 shadow-sm">
          {/* Theme */}
          <div className="flex items-center justify-between">
            <span className="text-sm">Theme</span>
            <div className="flex rounded-lg border border-border">
              {THEME_OPTIONS.map((t, i) => (
                <button
                  key={t.value}
                  onClick={() => { setThemeName(t.value); toast.success(`Theme: ${t.label}`); }}
                  aria-pressed={themeName === t.value}
                  className={clsx(
                    "flex items-center gap-1.5 px-3 py-1 text-xs font-medium transition-colors duration-200",
                    i === 0 && "rounded-l-lg",
                    i === THEME_OPTIONS.length - 1 && "rounded-r-lg",
                    themeName === t.value
                      ? "bg-primary text-primary-foreground shadow-sm"
                      : "text-muted-foreground hover:bg-accent"
                  )}
                >
                  <span className="h-2.5 w-2.5 rounded-full border border-primary-foreground/20" style={{ backgroundColor: themeName === t.value ? "oklch(1 0 0 / 0.9)" : t.colors[0] }} />
                  {t.label}
                </button>
              ))}
            </div>
          </div>

          <div className="border-t border-border" />

          {/* Mode */}
          <div className="flex items-center justify-between">
            <span className="text-sm">Mode</span>
            <div className="flex rounded-lg border border-border">
              {(["system", "light", "dark"] as const).map((m, i) => (
                <button
                  key={m}
                  onClick={() => { setMode(m); toast.success(`Mode: ${m === "system" ? "System" : m === "light" ? "Light" : "Dark"}`); }}
                  aria-pressed={mode === m}
                  className={clsx(
                    "px-3 py-1 text-xs font-medium transition-colors duration-200",
                    i === 0 && "rounded-l-lg",
                    i === 2 && "rounded-r-lg",
                  mode === m
                    ? "bg-primary text-primary-foreground shadow-sm"
                    : "text-muted-foreground hover:bg-accent"
                )}
              >
                {m === "system" ? "System" : m === "light" ? "Light" : "Dark"}
              </button>
            ))}
          </div>
          </div>
        </div>
      </section>

      {/* Agent Paths */}
      <section className="space-y-4 border-t border-border pt-8">
        <h3 className="text-sm font-medium text-muted-foreground">Agent Paths</h3>
        <p className="text-xs text-muted-foreground">
          Auto-detected paths shown below. Click the edit button to choose a custom path.
        </p>
        {agentNames.map((agent) => {
          const info = agentMap.get(agent);
          const isEnabled = info?.enabled ?? true;
          return (
            <div key={agent} className={clsx(
              "flex items-center gap-3 rounded-lg border border-border bg-card px-4 py-3 shadow-sm transition-opacity",
              !isEnabled && "opacity-50"
            )}>
              <button
                type="button"
                onClick={() => setEnabled(agent, !isEnabled)}
                className={clsx(
                  "shrink-0 w-16 text-center rounded-md px-2 py-0.5 text-xs font-medium transition-colors",
                  isEnabled
                    ? "bg-primary/10 text-primary hover:bg-primary/20"
                    : "bg-muted text-muted-foreground hover:bg-muted/80"
                )}
              >
                {isEnabled ? "Enabled" : "Disabled"}
              </button>
              <span className="shrink-0 w-28 text-sm font-medium text-foreground">{agentDisplayName(agent)}</span>
              <input
                type="text"
                readOnly
                value={info?.path ?? ""}
                placeholder="Not detected"
                aria-label={`${agent} config path`}
                className="flex-1 rounded-md border border-border bg-muted px-3 py-1 text-sm text-foreground placeholder:text-muted-foreground cursor-default truncate"
              />
              <button
                type="button"
                disabled={!isEnabled}
                aria-label={`Change ${agent} path`}
                className="shrink-0 rounded-md border border-border p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:pointer-events-none disabled:opacity-40"
                onClick={async () => {
                  const path = await openDirectoryPicker(`Select ${agent} directory`);
                  if (path) updatePath(agent, path);
                }}
              >
                <Pencil size={14} />
              </button>
            </div>
          );
        })}
      </section>

      {/* Project Paths */}
      <section id="project-paths" className="space-y-4 border-t border-border pt-8">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-muted-foreground">Project Paths</h3>
            <p className="text-xs text-muted-foreground mt-1">
              Add project directories to scan their local extensions (.claude/skills, .mcp.json, hooks).
            </p>
          </div>
          <button
            onClick={handleAdd}
            disabled={adding}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground shadow-sm transition-[color,background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md disabled:opacity-50"
          >
            {adding ? <Loader2 size={12} className="animate-spin" /> : <Plus size={12} />}
            Add
          </button>
        </div>

        {/* Discovered projects (shown when user selected a non-project root dir) */}
        {discoveredProjects !== null && (
          <div className="rounded-lg border border-border bg-card p-4 space-y-3 shadow-sm">
            <p className="text-xs text-muted-foreground">
              The selected directory is not a project. Found {discoveredProjects.length} project(s) inside:
            </p>
            {discoveredProjects.length === 0 ? (
              <p className="text-xs text-muted-foreground italic">No projects found.</p>
            ) : (
              <>
                <div className="space-y-1 max-h-48 overflow-y-auto" onWheel={(e) => e.stopPropagation()}>
                  {discoveredProjects.map((dp) => {
                    const already = existingPaths.has(dp.path);
                    return (
                      <label
                        key={dp.path}
                        className={clsx(
                          "flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm cursor-pointer transition-colors",
                          already ? "opacity-50 cursor-not-allowed" : "hover:bg-muted"
                        )}
                      >
                        <input
                          type="checkbox"
                          disabled={already}
                          checked={discoveredSelected.has(dp.path)}
                          onChange={() => toggleDiscovered(dp.path)}
                          className="rounded border-border"
                        />
                        <div className="min-w-0 flex-1">
                          <span className="font-medium text-foreground">{dp.name}</span>
                          <span className="ml-2 text-xs text-muted-foreground truncate">{dp.path}</span>
                        </div>
                        {already && <span className="text-xs text-muted-foreground">Added</span>}
                      </label>
                    );
                  })}
                </div>
                <div className="flex justify-end gap-2">
                  <button
                    onClick={() => setDiscoveredProjects(null)}
                    className="rounded-lg border border-border px-3 py-1 text-xs text-muted-foreground hover:bg-muted"
                  >
                    Cancel
                  </button>
                  <button
                    onClick={handleAddDiscovered}
                    disabled={discoveredSelected.size === 0 || adding}
                    className="rounded-lg bg-primary px-3 py-1 text-xs text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
                  >
                    Add Selected ({discoveredSelected.size})
                  </button>
                </div>
              </>
            )}
          </div>
        )}

        {/* Project list */}
        {loading ? (
          <p className="text-xs text-muted-foreground">Loading...</p>
        ) : projects.length === 0 ? (
          <div className="rounded-lg border-2 border-dashed border-border bg-muted/20 p-6">
            <h4 className="text-sm font-medium text-foreground">No projects yet</h4>
            <p className="mt-1 text-xs text-muted-foreground">Add a project directory to scan for local extensions.</p>
          </div>
        ) : (
          <div className="space-y-1">
            {projects.map((project) => {
              const isSelected = selectedProject?.id === project.id;
              const isConfirmingRemove = confirmingRemoveId === project.id;
              return (
                <div key={project.id}>
                  {isConfirmingRemove ? (
                    <div className="animate-fade-in flex items-center gap-3 rounded-lg px-4 py-2.5 text-sm border border-border bg-card shadow-sm">
                      <span className="text-sm text-muted-foreground">Remove {project.name}?</span>
                      <div className="ml-auto flex items-center gap-2">
                        <button
                          onClick={() => { removeProject(project.id); setConfirmingRemoveId(null); toast.success("Project removed"); }}
                          className="rounded-lg bg-destructive px-3 py-1 text-xs text-destructive-foreground hover:bg-destructive/90"
                        >
                          Remove
                        </button>
                        <button
                          onClick={() => setConfirmingRemoveId(null)}
                          className="rounded-lg px-3 py-1 text-xs text-muted-foreground hover:text-foreground"
                        >
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                  <button
                    type="button"
                    onClick={() => selectProject(isSelected ? null : project)}
                    className={clsx(
                      "group flex w-full text-left items-center gap-3 rounded-lg px-4 py-2.5 text-sm cursor-pointer border shadow-sm transition-[color,background-color,border-color,box-shadow,transform] duration-200",
                      isSelected
                        ? "border-ring bg-accent"
                        : "border-border bg-card hover:bg-muted hover:shadow-md"
                    )}
                  >
                    {isSelected ? <ChevronDown size={14} className="shrink-0 text-muted-foreground" /> : <ChevronRight size={14} className="shrink-0 text-muted-foreground" />}
                    <FolderOpen size={14} className="shrink-0 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <span className="font-medium text-foreground">{project.name}</span>
                      <span className="ml-2 text-xs text-muted-foreground truncate">{project.path}</span>
                    </div>
                    <span
                      role="button"
                      tabIndex={0}
                      onClick={(e) => { e.stopPropagation(); setConfirmingRemoveId(project.id); }}
                      onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); e.stopPropagation(); setConfirmingRemoveId(project.id); } }}
                      className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-destructive transition-opacity cursor-pointer"
                      aria-label={`Remove ${project.name}`}
                    >
                      <Trash2 size={14} />
                    </span>
                  </button>
                  )}

                  {/* Expanded: show project extensions */}
                  {isSelected && (
                    <div className="animate-fade-in ml-8 mt-1 mb-2 space-y-2">
                      {extensionsLoading ? (
                        <p className="text-xs text-muted-foreground py-2">Scanning...</p>
                      ) : projectExtensions.length === 0 ? (
                        <p className="text-xs text-muted-foreground italic py-2">No project-level extensions found.</p>
                      ) : (
                        kindOrder.map((kind) => {
                          const items = grouped[kind];
                          if (!items || items.length === 0) return null;
                          return (
                            <div key={kind}>
                              <p className="text-xs font-medium text-muted-foreground mb-1">
                                {kindLabels[kind]} ({items.length})
                              </p>
                              {items.map((ext) => (
                                <div key={ext.id} className="flex items-center gap-2 rounded-lg border border-border bg-card px-3 py-2 mb-1">
                                  <span className="text-sm text-foreground">{ext.name}</span>
                                  <KindBadge kind={ext.kind} />
                                  {ext.description && (
                                    <span className="text-xs text-muted-foreground truncate ml-auto">{ext.description}</span>
                                  )}
                                </div>
                              ))}
                            </div>
                          );
                        })
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div></div></div>
  );
}
