import { clsx } from "clsx";
import {
  Check,
  Download,
  FolderOpen,
  FolderSearch,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { openDirectoryPicker } from "@/lib/dialog";
import { api } from "@/lib/invoke";
import { isDesktop } from "@/lib/transport";
import { agentDisplayName, type DiscoveredProject } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { useProjectStore } from "@/stores/project-store";
import { toast } from "@/stores/toast-store";
import type { AppIcon, ThemeName } from "@/stores/ui-store";
import { useUIStore } from "@/stores/ui-store";
import { useUpdateStore } from "@/stores/update-store";
import { useWebUpdateStore } from "@/stores/web-update-store";

const THEME_OPTIONS: {
  value: ThemeName;
  label: string;
  colors: [string, string, string];
}[] = [
  {
    value: "tiesen",
    label: "Tiesen",
    colors: [
      "oklch(0.5144 0.1605 267.4400)",
      "oklch(0.9851 0 0)",
      "oklch(0 0 0)",
    ],
  },
  {
    value: "claude",
    label: "Claude",
    colors: [
      "oklch(0.6171 0.1375 39.0427)",
      "oklch(0.9665 0.0067 97.3521)",
      "oklch(0.2679 0.0036 106.6427)",
    ],
  },
];

const ICON_OPTIONS: { value: AppIcon; label: string; src: string }[] = [
  { value: "icon-1", label: "Tiesen", src: "/icons/app-icon-1.png" },
  { value: "icon-2", label: "Claude", src: "/icons/app-icon-2.png" },
];

function UpdateSection() {
  const available = useUpdateStore((s) => s.available);
  const checking = useUpdateStore((s) => s.checking);
  const installing = useUpdateStore((s) => s.installing);
  const checkForUpdate = useUpdateStore((s) => s.checkForUpdate);
  const promptUpdate = useUpdateStore((s) => s.promptUpdate);

  const handleCheck = async () => {
    await checkForUpdate();
    // Show toast if no update found (checked becomes true, available stays null)
    if (!useUpdateStore.getState().available) {
      toast.success("You're up to date");
    }
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-muted-foreground">v{__APP_VERSION__}</span>
      {available ? (
        <button
          onClick={promptUpdate}
          disabled={installing}
          className="flex items-center gap-1.5 rounded-lg bg-primary px-2.5 py-1 text-xs text-primary-foreground shadow-sm hover:bg-primary/90 disabled:opacity-50 transition-colors"
        >
          {installing ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <Download size={12} />
          )}
          {installing ? "Updating..." : `Update to v${available.version}`}
        </button>
      ) : (
        <button
          onClick={handleCheck}
          disabled={checking}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-muted disabled:opacity-50 transition-colors"
        >
          <RefreshCw
            size={12}
            className={checking ? "origin-center animate-spin" : ""}
          />
          {checking ? "Checking..." : "Check for Updates"}
        </button>
      )}
    </div>
  );
}

function WebUpdateSection() {
  const available = useWebUpdateStore((s) => s.available);
  const checking = useWebUpdateStore((s) => s.checking);
  const checkForUpdate = useWebUpdateStore((s) => s.checkForUpdate);
  const promptUpdate = useWebUpdateStore((s) => s.promptUpdate);

  const handleCheck = async () => {
    await checkForUpdate(true);
    if (!useWebUpdateStore.getState().available) {
      toast.success("You're up to date");
    }
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-muted-foreground">v{__APP_VERSION__}</span>
      {available ? (
        <button
          onClick={promptUpdate}
          className="flex items-center gap-1.5 rounded-lg bg-primary px-2.5 py-1 text-xs text-primary-foreground shadow-sm hover:bg-primary/90 transition-colors"
        >
          <Download size={12} />
          Update to v{available.version}
        </button>
      ) : (
        <button
          onClick={handleCheck}
          disabled={checking}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-muted disabled:opacity-50 transition-colors"
        >
          <RefreshCw
            size={12}
            className={checking ? "origin-center animate-spin" : ""}
          />
          {checking ? "Checking..." : "Check for Updates"}
        </button>
      )}
    </div>
  );
}

export default function SettingsPage() {
  const {
    themeName,
    mode,
    appIcon,
    setThemeName,
    setMode,
    setAppIcon: setAppIconState,
  } = useUIStore();
  const { projects, loading, loadProjects, addProject, removeProject } =
    useProjectStore();

  const {
    agents,
    fetch: fetchAgents,
    updatePath,
    setEnabled,
  } = useAgentStore();
  const [searchParams, setSearchParams] = useSearchParams();

  const [editingAgent, setEditingAgent] = useState<string | null>(null);
  const [editingPath, setEditingPath] = useState("");
  const [adding, setAdding] = useState(false);
  const [projectPathInput, setProjectPathInput] = useState("");
  const [discoveredProjects, setDiscoveredProjects] = useState<
    DiscoveredProject[] | null
  >(null);
  const [discoveredSelected, setDiscoveredSelected] = useState<Set<string>>(
    new Set(),
  );

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

  const handleAddPath = async (path: string) => {
    if (!path) return;
    setAdding(true);
    try {
      await addProject(path);
      setDiscoveredProjects(null);
      setProjectPathInput("");
      toast.success("Project added");
    } catch {
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

  const handleBrowseProject = async () => {
    const path = await openDirectoryPicker({
      title: "Select Project Directory",
    });
    if (path) handleAddPath(path);
  };

  const handleAddDiscovered = async () => {
    setAdding(true);
    let added = 0;
    const failed: string[] = [];
    try {
      for (const path of discoveredSelected) {
        try {
          await addProject(path);
          added++;
        } catch {
          failed.push(path);
        }
      }
      if (added > 0)
        toast.success(`${added} project${added > 1 ? "s" : ""} added`);
      if (failed.length > 0)
        toast.error(
          `Failed to add ${failed.length} project${failed.length > 1 ? "s" : ""}: ${failed.join(", ")}`,
        );
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

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      <div className="shrink-0 pb-4">
        <div className="flex items-center justify-between">
          <h2 className="text-2xl font-bold tracking-tight select-none">
            Settings
          </h2>
          {isDesktop() ? <UpdateSection /> : <WebUpdateSection />}
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="max-w-2xl mx-auto space-y-8 pb-6">
          {/* Agent Paths */}
          <section className="space-y-4">
            <div>
              <h3 className="text-sm font-medium text-muted-foreground">
                Agent Paths
              </h3>
              <p className="text-xs text-muted-foreground mt-1">
                Auto-detected paths shown below. Click the edit button to choose
                a custom path.
              </p>
            </div>
            <div className="flex flex-col rounded-lg border border-border bg-card shadow-sm divide-y divide-border">
              {agentNames.map((agent) => {
                const info = agentMap.get(agent);
                const isEnabled = info?.enabled ?? true;
                return (
                  <div
                    key={agent}
                    className={clsx(
                      "flex items-center gap-3 px-4 py-2.5 transition-opacity",
                      !isEnabled && "opacity-50",
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => setEnabled(agent, !isEnabled)}
                      className={clsx(
                        "shrink-0 w-16 text-center rounded-md px-2 py-0.5 text-xs font-medium transition-colors",
                        isEnabled
                          ? "bg-primary/10 text-primary hover:bg-primary/20"
                          : "bg-muted text-muted-foreground hover:bg-muted/80",
                      )}
                    >
                      {isEnabled ? "Enabled" : "Disabled"}
                    </button>
                    <span className="shrink-0 w-28 text-sm font-medium text-foreground">
                      {agentDisplayName(agent)}
                    </span>
                    <input
                      type="text"
                      readOnly={editingAgent !== agent}
                      disabled={!isEnabled}
                      value={
                        editingAgent === agent
                          ? editingPath
                          : (info?.path ?? "")
                      }
                      placeholder="Not detected"
                      aria-label={`${agent} config path`}
                      onChange={(e) => setEditingPath(e.target.value)}
                      onKeyDown={(e) => {
                        if (
                          e.key === "Enter" &&
                          !e.nativeEvent.isComposing &&
                          e.keyCode !== 229 &&
                          editingPath.trim()
                        ) {
                          updatePath(agent, editingPath.trim());
                          setEditingAgent(null);
                        }
                        if (e.key === "Escape") setEditingAgent(null);
                      }}
                      className={clsx(
                        "flex-1 rounded-md border border-border px-3 py-1 text-sm text-foreground placeholder:text-muted-foreground truncate disabled:opacity-40",
                        editingAgent === agent
                          ? "bg-card ring-1 ring-ring"
                          : "bg-muted cursor-default",
                      )}
                    />
                    {editingAgent === agent ? (
                      <>
                        {isDesktop() && (
                          <button
                            type="button"
                            aria-label={`Browse ${agent} path`}
                            className="shrink-0 rounded-md border border-border p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
                            onClick={async () => {
                              const path = await openDirectoryPicker({
                                title: `Select ${agent} directory`,
                              });
                              if (path) {
                                updatePath(agent, path);
                                setEditingAgent(null);
                              }
                            }}
                          >
                            <FolderSearch size={14} />
                          </button>
                        )}
                        <button
                          type="button"
                          aria-label="Cancel"
                          className="shrink-0 rounded-md border border-border bg-background p-1.5 text-muted-foreground hover:text-foreground transition-colors"
                          onClick={() => setEditingAgent(null)}
                        >
                          <X size={14} />
                        </button>
                        <button
                          type="button"
                          aria-label="Save"
                          disabled={!editingPath.trim()}
                          className="shrink-0 rounded-md bg-primary p-1.5 text-primary-foreground hover:bg-primary/90 disabled:opacity-40 transition-colors"
                          onClick={() => {
                            updatePath(agent, editingPath.trim());
                            setEditingAgent(null);
                          }}
                        >
                          <Check size={14} />
                        </button>
                      </>
                    ) : (
                      <button
                        type="button"
                        disabled={!isEnabled}
                        aria-label={`Edit ${agent} path`}
                        className="shrink-0 rounded-md border border-border p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors disabled:pointer-events-none disabled:opacity-40"
                        onClick={() => {
                          setEditingAgent(agent);
                          setEditingPath(info?.path ?? "");
                        }}
                      >
                        <Pencil size={14} />
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </section>

          {/* Project Paths */}
          <section
            id="project-paths"
            className="space-y-4 border-t border-border pt-8"
          >
            <div>
              <h3 className="text-sm font-medium text-muted-foreground">
                Project Paths
              </h3>
              <p className="text-xs text-muted-foreground mt-1">
                Add project directories to scan their local extensions
                (.claude/skills, .mcp.json, hooks).
              </p>
            </div>
            <div className="flex items-center gap-1.5">
              <input
                type="text"
                placeholder={
                  isDesktop()
                    ? "Paste a project path or browse..."
                    : "Paste a project path..."
                }
                value={projectPathInput}
                onChange={(e) => setProjectPathInput(e.target.value)}
                onKeyDown={(e) => {
                  if (
                    e.key === "Enter" &&
                    !e.nativeEvent.isComposing &&
                    e.keyCode !== 229 &&
                    projectPathInput.trim()
                  )
                    handleAddPath(projectPathInput.trim());
                }}
                className="flex-1 rounded-md border border-border bg-card px-3 py-1.5 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
              />
              {isDesktop() && (
                <button
                  type="button"
                  disabled={adding}
                  onClick={handleBrowseProject}
                  className="shrink-0 rounded-md border border-border bg-card p-1.5 text-muted-foreground hover:text-foreground hover:bg-accent transition-colors disabled:opacity-40"
                  title="Browse..."
                >
                  <FolderSearch size={16} />
                </button>
              )}
              <button
                onClick={() => handleAddPath(projectPathInput.trim())}
                disabled={adding || !projectPathInput.trim()}
                className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs text-primary-foreground shadow-sm transition-[color,background-color,box-shadow] duration-200 hover:bg-primary/90 hover:shadow-md disabled:opacity-50"
              >
                {adding ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <Plus size={12} />
                )}
                Add
              </button>
            </div>

            {/* Discovered projects (shown when user selected a non-project root dir) */}
            {discoveredProjects !== null && (
              <div className="rounded-lg border border-border bg-card p-4 space-y-3 shadow-sm">
                <p className="text-xs text-muted-foreground">
                  The selected directory is not a project. Found{" "}
                  {discoveredProjects.length} project(s) inside:
                </p>
                {discoveredProjects.length === 0 ? (
                  <p className="text-xs text-muted-foreground italic">
                    No projects found.
                  </p>
                ) : (
                  <>
                    <div className="space-y-1 max-h-48 overflow-y-auto overscroll-contain">
                      {discoveredProjects.map((dp) => {
                        const already = existingPaths.has(dp.path);
                        return (
                          <label
                            key={dp.path}
                            className={clsx(
                              "flex items-center gap-2 rounded-lg px-2 py-1.5 text-sm cursor-pointer transition-colors",
                              already
                                ? "opacity-50 cursor-not-allowed"
                                : "hover:bg-muted",
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
                              <span className="font-medium text-foreground">
                                {dp.name}
                              </span>
                              <span className="ml-2 text-xs text-muted-foreground truncate">
                                {dp.path}
                              </span>
                            </div>
                            {already && (
                              <span className="text-xs text-muted-foreground">
                                Added
                              </span>
                            )}
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
                <h4 className="text-sm font-medium text-foreground">
                  No projects yet
                </h4>
                <p className="mt-1 text-xs text-muted-foreground">
                  Add a project directory to scan for local extensions.
                </p>
              </div>
            ) : (
              <div className="space-y-1">
                {projects.map((project) => (
                  <div
                    key={project.id}
                    className={clsx(
                      "flex w-full items-center gap-3 rounded-lg px-4 py-2.5 text-sm border bg-card shadow-sm",
                      project.exists ? "border-border" : "border-border",
                    )}
                  >
                    <FolderOpen
                      size={14}
                      className={clsx(
                        "shrink-0",
                        project.exists
                          ? "text-muted-foreground"
                          : "text-muted-foreground/50",
                      )}
                    />
                    <div className="min-w-0 flex-1">
                      <span
                        className={clsx(
                          "font-medium",
                          project.exists
                            ? "text-foreground"
                            : "text-muted-foreground line-through",
                        )}
                      >
                        {project.name}
                      </span>
                      {!project.exists && (
                        <span className="ml-2 text-[10px] px-1.5 py-0.5 rounded-full bg-muted text-muted-foreground inline-flex items-center gap-1">
                          <TriangleAlert size={10} /> Missing
                        </span>
                      )}
                      <span className="ml-2 text-xs text-muted-foreground truncate">
                        {project.path}
                      </span>
                    </div>
                    <button
                      type="button"
                      onClick={() => {
                        removeProject(project.id);
                        toast.success("Project removed");
                      }}
                      className="text-muted-foreground hover:text-destructive transition-colors cursor-pointer focus:outline-none"
                      aria-label={`Remove ${project.name}`}
                    >
                      <Trash2 size={14} />
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* Appearance */}
          <section className="space-y-4 border-t border-border pt-8">
            <h3 className="text-sm font-medium text-muted-foreground">
              Appearance
            </h3>

            <div className="flex flex-col gap-2 rounded-lg border border-border bg-card px-4 py-2.5 shadow-sm">
              {/* Theme */}
              <div className="flex items-center justify-between">
                <span className="text-sm">Theme</span>
                <div className="flex rounded-lg border border-border">
                  {THEME_OPTIONS.map((t, i) => (
                    <button
                      key={t.value}
                      onClick={() => {
                        setThemeName(t.value);
                        toast.success(`Theme: ${t.label}`);
                      }}
                      aria-pressed={themeName === t.value}
                      className={clsx(
                        "flex items-center gap-1.5 px-3 py-1 text-xs font-medium transition-colors duration-200",
                        i === 0 && "rounded-l-lg",
                        i === THEME_OPTIONS.length - 1 && "rounded-r-lg",
                        themeName === t.value
                          ? "bg-primary text-primary-foreground shadow-sm"
                          : "text-muted-foreground hover:bg-accent",
                      )}
                    >
                      <span
                        className="h-2.5 w-2.5 rounded-full border border-primary-foreground/20"
                        style={{
                          backgroundColor:
                            themeName === t.value
                              ? "oklch(1 0 0 / 0.9)"
                              : t.colors[0],
                        }}
                      />
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
                      onClick={() => {
                        setMode(m);
                        toast.success(
                          `Mode: ${m === "system" ? "System" : m === "light" ? "Light" : "Dark"}`,
                        );
                      }}
                      aria-pressed={mode === m}
                      className={clsx(
                        "px-3 py-1 text-xs font-medium transition-colors duration-200",
                        i === 0 && "rounded-l-lg",
                        i === 2 && "rounded-r-lg",
                        mode === m
                          ? "bg-primary text-primary-foreground shadow-sm"
                          : "text-muted-foreground hover:bg-accent",
                      )}
                    >
                      {m === "system"
                        ? "System"
                        : m === "light"
                          ? "Light"
                          : "Dark"}
                    </button>
                  ))}
                </div>
              </div>

              {isDesktop() && (
                <>
                  <div className="border-t border-border" />

                  {/* App Icon — desktop only */}
                  <div className="flex items-center justify-between">
                    <span className="text-sm">App Icon</span>
                    <div className="flex gap-2">
                      {ICON_OPTIONS.map((icon) => (
                        <button
                          key={icon.value}
                          onClick={() => {
                            setAppIconState(icon.value);
                            api
                              .setAppIcon(icon.value)
                              .then(() => {
                                toast.success(`Icon: ${icon.label}`);
                              })
                              .catch(() => {
                                toast.error("Failed to set icon");
                              });
                          }}
                          aria-pressed={appIcon === icon.value}
                          className={clsx(
                            "rounded-lg p-0.5 transition-all duration-200",
                            appIcon === icon.value
                              ? "ring-2 ring-primary ring-offset-2 ring-offset-card"
                              : "ring-1 ring-border hover:ring-primary/50",
                          )}
                        >
                          <img
                            src={icon.src}
                            alt={icon.label}
                            className="h-10 w-10 rounded-md"
                          />
                        </button>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </div>
          </section>

          {/* Footer */}
          <footer className="border-t border-border pt-6 pb-2 flex items-center justify-center gap-1.5 text-xs text-muted-foreground/50">
            <span>HarnessKit</span>
            <span>&middot;</span>
            <span>One home for every agent</span>
            <span>&middot;</span>
            <a
              href="https://github.com/RealZST/HarnessKit"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-muted-foreground transition-colors"
            >
              GitHub
            </a>
          </footer>
        </div>
      </div>
    </div>
  );
}
