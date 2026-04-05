import { AlertTriangle, FolderOpen, Link, Loader2, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import type {
  Extension,
  ExtensionContent as ExtContent,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName } from "@/lib/types";

type DeleteItem = {
  key: string;
  agents: string[];
  paths: string[];
  mcps: string[];
  shared: boolean;
  symlink?: string;
  description?: string;
};

/**
 * Build path-based delete items from skill locations (for CLI and Skill).
 * Each item = one physical path, with agent names as the primary label.
 */
function buildPathItems(
  locations: [string, string, string | null][],
  childMcps?: Extension[],
  instanceData?: Map<string, ExtContent>,
): DeleteItem[] {
  // Group by physical path → list of agents
  const pathMap = new Map<string, string[]>();
  for (const [agent, path] of locations) {
    const list = pathMap.get(path) ?? [];
    if (!list.includes(agent)) list.push(agent);
    pathMap.set(path, list);
  }

  const items: DeleteItem[] = [];
  for (const [path, agents] of pathMap) {
    items.push({
      key: `path:${path}`,
      agents,
      paths: [path],
      mcps: [],
      shared: agents.length > 1,
    });
  }

  // Attach MCPs as separate items
  if (childMcps) {
    for (const m of childMcps) {
      const mcpData = instanceData?.get(m.id);
      items.push({
        key: `mcp:${m.id}`,
        agents: [...m.agents],
        paths: mcpData?.path ? [mcpData.path] : [],
        mcps: [m.name],
        shared: false,
        description: `Remove MCP server "${m.name}" from configuration`,
      });
    }
  }

  return items;
}

/**
 * Build agent-based delete items from instances (for MCP, Hook, Plugin).
 * For these types, the "path" is the config file they live in (e.g. settings.json),
 * NOT a file being deleted — so we show a description instead.
 */
function buildAgentItems(
  instances: GroupedExtension["instances"],
  instanceData: Map<string, ExtContent>,
  kind: string,
  name: string,
): DeleteItem[] {
  return instances.map((inst) => {
    const data = instanceData.get(inst.id);
    const configPath = data?.path ?? null;
    const isConfigBased = kind === "mcp" || kind === "hook";
    const desc = isConfigBased
      ? kind === "mcp"
        ? `Remove MCP server "${name}" from configuration`
        : `Remove hook from configuration`
      : null;
    return {
      key: `agent:${inst.agents[0]}`,
      agents: [...inst.agents],
      paths: configPath ? [configPath] : [],
      mcps: [],
      shared: false,
      description: desc ?? undefined,
    };
  });
}

export function DeleteDialog({
  group,
  instanceData,
  deleting,
  deleteAgents,
  setDeleteAgents,
  onDelete,
  onClose,
  childExtensions,
  skillLocations,
}: {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  deleting: boolean;
  deleteAgents: Set<string>;
  setDeleteAgents: (s: Set<string>) => void;
  onDelete: (agents: string[]) => void;
  onClose: () => void;
  childExtensions?: Extension[];
  skillLocations?: [string, string, string | null][];
}) {
  const dlgRef = useRef<HTMLDivElement>(null);

  // Escape to close
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  // Focus trap
  useFocusTrap(dlgRef, true);

  // Reset selection when dialog opens
  useEffect(() => {
    setDeleteAgents(new Set());
  }, [setDeleteAgents]);

  // Friendly display name (hooks use internal format like "AfterAgent:*:command")
  const displayName = group.kind === "hook"
    ? (() => {
        const parts = group.name.split(":");
        if (parts.length >= 3) {
          const cmd = parts.slice(2).join(":");
          return cmd.split(" ").map((t) => t.split("/").pop() || t).join(" ");
        }
        return group.name;
      })()
    : group.name;

  // Build items based on extension kind
  const isCli = group.kind === "cli";
  const isSkill = group.kind === "skill";
  const usePathBased = (isCli || isSkill) && skillLocations && skillLocations.length > 0;

  const items: DeleteItem[] = usePathBased
    ? buildPathItems(
        skillLocations!,
        isCli ? (childExtensions ?? []).filter((e) => e.kind === "mcp") : undefined,
        instanceData,
      )
    : buildAgentItems(group.instances, instanceData, group.kind, group.name);

  const selectedKeys = deleteAgents;
  const allSelected = items.length > 0 && items.every((i) => selectedKeys.has(i.key));
  const isSingle = items.length === 1;
  const binaryPath = isCli ? group.instances[0]?.cli_meta?.binary_path : null;

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label="Delete extension"
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <Trash2 size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Delete "{displayName}"
            </h3>
            <p className="text-xs text-muted-foreground">
              This action cannot be undone.
            </p>
          </div>
        </div>

        <div className="space-y-3">
          <p className="text-xs text-muted-foreground">
            {isSingle
              ? "This will permanently delete:"
              : "Select items to remove:"}
          </p>

          <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
            {/* All Items toggle */}
            {!isSingle && (
              <label className="flex items-start gap-2 text-xs cursor-pointer pb-1.5 mb-1.5 border-b border-border/50">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={() => {
                    setDeleteAgents(
                      allSelected
                        ? new Set()
                        : new Set(items.map((i) => i.key)),
                    );
                  }}
                  className="mt-0.5 rounded border-border accent-destructive"
                />
                <span className="font-medium text-foreground">All Items</span>
              </label>
            )}

            {/* Each deletable item */}
            {items.map((item) => (
              <label
                key={item.key}
                className={`flex items-start gap-2 text-xs ${isSingle ? "" : "cursor-pointer"}`}
              >
                {!isSingle && (
                  <input
                    type="checkbox"
                    checked={selectedKeys.has(item.key)}
                    onChange={() => {
                      const next = new Set(selectedKeys);
                      if (next.has(item.key)) next.delete(item.key);
                      else next.add(item.key);
                      setDeleteAgents(next);
                    }}
                    className="mt-0.5 rounded border-border accent-destructive"
                  />
                )}
                <div className="min-w-0">
                  {/* Agent names as primary label */}
                  <span className="font-medium text-foreground">
                    {item.agents.map(agentDisplayName).join(", ")}
                  </span>
                  {item.shared && (
                    <span className="ml-1.5 text-[10px] text-chart-5 font-medium">
                      shared
                    </span>
                  )}
                  {/* Description (for config-based types like MCP/Hook) */}
                  {item.description && (
                    <p className="text-muted-foreground mt-0.5">
                      {item.description}
                    </p>
                  )}
                  {/* Paths as secondary info */}
                  {item.paths.map((p) => (
                    <p
                      key={p}
                      className="text-muted-foreground flex items-start gap-1 mt-0.5"
                    >
                      <FolderOpen size={10} className="mt-0.5 shrink-0" />
                      <span className="break-all">{p}</span>
                    </p>
                  ))}
                  {/* MCPs (only if no description already mentions it) */}
                  {!item.description && item.mcps.map((name) => (
                    <p key={name} className="text-muted-foreground mt-0.5">
                      MCP: {name}
                    </p>
                  ))}
                  {/* Symlink */}
                  {item.symlink && (
                    <p className="flex items-center gap-1 text-chart-5 mt-0.5">
                      <Link size={10} className="shrink-0" />
                      <span className="break-all">{item.symlink}</span>
                    </p>
                  )}
                </div>
              </label>
            ))}
          </div>

          {/* Binary removal warning for CLI */}
          {isCli && allSelected && binaryPath && (
            <div className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5">
              <AlertTriangle size={12} className="mt-0.5 shrink-0" />
              <span>
                All items selected — the binary{" "}
                <span className="font-mono">{binaryPath}</span> will also be
                removed.
              </span>
            </div>
          )}

          {/* Delete button */}
          {isSingle ? (
            <button
              disabled={deleting}
              onClick={() => onDelete(items[0].agents)}
              className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
            >
              {deleting ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Trash2 size={12} />
              )}
              Delete from{" "}
              {items[0].agents.map(agentDisplayName).join(", ")}
            </button>
          ) : (
            <button
              disabled={deleting || selectedKeys.size === 0}
              onClick={() => {
                const agents = new Set<string>();
                for (const item of items) {
                  if (selectedKeys.has(item.key)) {
                    for (const a of item.agents) agents.add(a);
                  }
                }
                onDelete(Array.from(agents));
              }}
              className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
            >
              {deleting ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Trash2 size={12} />
              )}
              {isCli && allSelected
                ? `Uninstall ${displayName}`
                : `Remove ${selectedKeys.size} item${selectedKeys.size !== 1 ? "s" : ""}`}
            </button>
          )}
        </div>

        {/* Cancel */}
        <button
          onClick={onClose}
          disabled={deleting}
          className="mt-4 w-full rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground hover:bg-muted disabled:opacity-50"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
