import { useEffect, useState } from "react";
import { useUIStore } from "@/stores/ui-store";
import type { ThemeName } from "@/stores/ui-store";
import { useProjectStore } from "@/stores/project-store";
import { KindBadge } from "@/components/shared/kind-badge";
import { FolderOpen, Plus, Trash2, Loader2, ChevronDown, ChevronRight } from "lucide-react";
import { clsx } from "clsx";
import { api } from "@/lib/invoke";
import type { Extension, ExtensionKind, DiscoveredProject } from "@/lib/types";

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
  { value: "astrovista", label: "AstroVista", colors: ["oklch(0.6420 0.1691 38.5815)", "oklch(0.4138 0.0846 259.8759)", "oklch(0.2178 0 0)"] },
  { value: "claude", label: "Claude", colors: ["oklch(0.6171 0.1375 39.0427)", "oklch(0.9665 0.0067 97.3521)", "oklch(0.2679 0.0036 106.6427)"] },
  { value: "lightgreen", label: "Light Green", colors: ["oklch(0.8871 0.2122 128.5041)", "oklch(0.3717 0.0392 257.2870)", "oklch(0.1288 0.0406 264.6952)"] },
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

  const [adding, setAdding] = useState(false);
  const [discoveredProjects, setDiscoveredProjects] = useState<DiscoveredProject[] | null>(null);
  const [discoveredSelected, setDiscoveredSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  const existingPaths = new Set(projects.map((p) => p.path));

  const handleAdd = async () => {
    const path = await openDirectoryPicker("Select Project Directory");
    if (!path) return;

    setAdding(true);
    try {
      // Try adding directly first (it's a project itself)
      await addProject(path);
      setDiscoveredProjects(null);
    } catch {
      // Not a valid project — try discovering projects inside it
      try {
        const results = await api.discoverProjects(path);
        if (results.length > 0) {
          setDiscoveredProjects(results);
          setDiscoveredSelected(new Set());
        }
      } catch (e) {
        console.error("Failed to discover projects:", e);
      }
    } finally {
      setAdding(false);
    }
  };

  const handleAddDiscovered = async () => {
    setAdding(true);
    try {
      for (const path of discoveredSelected) {
        try { await addProject(path); } catch {}
      }
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

  const grouped = groupByKind(projectExtensions);

  return (
    <div className="max-w-2xl space-y-8">
      <h2 className="text-xl font-semibold">Settings</h2>

      {/* Appearance */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-muted-foreground">Appearance</h3>

        {/* Theme selector */}
        <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
          <span className="text-sm font-medium">Theme</span>
          <div className="mt-3 grid grid-cols-4 gap-3">
            {THEME_OPTIONS.map((t) => (
              <button
                key={t.value}
                onClick={() => setThemeName(t.value)}
                className={clsx(
                  "flex flex-col items-center gap-2 rounded-lg border-2 p-3 transition-colors",
                  themeName === t.value
                    ? "border-primary bg-accent"
                    : "border-border hover:border-ring/50 hover:bg-muted"
                )}
              >
                <div className="flex gap-1">
                  {t.colors.map((color, i) => (
                    <span
                      key={i}
                      className="h-4 w-4 rounded-full border border-border"
                      style={{ backgroundColor: color }}
                    />
                  ))}
                </div>
                <span className="text-xs font-medium">{t.label}</span>
              </button>
            ))}
          </div>
        </div>

        {/* Mode selector */}
        <div className="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3 shadow-sm">
          <span className="text-sm">Mode</span>
          <div className="flex rounded-lg border border-border">
            <button
              onClick={() => setMode("light")}
              className={clsx(
                "rounded-l-lg px-3 py-1 text-xs font-medium transition-colors",
                mode === "light"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent"
              )}
            >
              Light
            </button>
            <button
              onClick={() => setMode("dark")}
              className={clsx(
                "rounded-r-lg px-3 py-1 text-xs font-medium transition-colors",
                mode === "dark"
                  ? "bg-primary text-primary-foreground"
                  : "text-muted-foreground hover:bg-accent"
              )}
            >
              Dark
            </button>
          </div>
        </div>
      </section>

      {/* Agent Paths */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-muted-foreground">Agent Paths</h3>
        <p className="text-xs text-muted-foreground">
          HarnessKit auto-detects agent directories. Override paths here if needed.
        </p>
        {["claude", "cursor", "codex", "gemini", "antigravity", "copilot"].map((agent) => (
          <div key={agent} className="flex items-center gap-4 rounded-lg border border-border bg-card px-4 py-3 shadow-sm">
            <span className="w-28 text-sm text-foreground">{agent}</span>
            <input
              type="text"
              placeholder="Auto-detected"
              className="flex-1 rounded-md border border-border bg-muted px-3 py-1 text-sm placeholder:text-muted-foreground focus:border-ring focus:outline-none"
            />
          </div>
        ))}
      </section>

      {/* Projects */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-medium text-muted-foreground">Projects</h3>
            <p className="text-xs text-muted-foreground mt-1">
              Add project directories to scan their local extensions (.claude/skills, .mcp.json, hooks).
            </p>
          </div>
          <button
            onClick={handleAdd}
            disabled={adding}
            className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
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
          <div className="rounded-lg border border-dashed border-border p-6 text-center">
            <FolderOpen size={24} className="mx-auto mb-2 text-muted-foreground" />
            <p className="text-xs text-muted-foreground">No projects added yet.</p>
          </div>
        ) : (
          <div className="space-y-1">
            {projects.map((project) => {
              const isSelected = selectedProject?.id === project.id;
              return (
                <div key={project.id}>
                  <div
                    onClick={() => selectProject(isSelected ? null : project)}
                    className={clsx(
                      "group flex items-center gap-3 rounded-lg px-4 py-2.5 text-sm transition-colors cursor-pointer border shadow-sm",
                      isSelected
                        ? "border-ring bg-accent"
                        : "border-border bg-card hover:bg-muted"
                    )}
                  >
                    {isSelected ? <ChevronDown size={14} className="shrink-0 text-muted-foreground" /> : <ChevronRight size={14} className="shrink-0 text-muted-foreground" />}
                    <FolderOpen size={14} className="shrink-0 text-muted-foreground" />
                    <div className="min-w-0 flex-1">
                      <span className="font-medium text-foreground">{project.name}</span>
                      <span className="ml-2 text-xs text-muted-foreground truncate">{project.path}</span>
                    </div>
                    <button
                      onClick={(e) => { e.stopPropagation(); removeProject(project.id); }}
                      className="opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-red-500 dark:hover:text-red-400 transition-opacity"
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>

                  {/* Expanded: show project extensions */}
                  {isSelected && (
                    <div className="ml-8 mt-1 mb-2 space-y-2">
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
    </div>
  );
}
