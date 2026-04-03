import { AlertTriangle, Link, Loader2, Trash2 } from "lucide-react";
import { useEffect, useRef } from "react";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import type {
  ExtensionContent as ExtContent,
  GroupedExtension,
} from "@/lib/types";
import { agentDisplayName } from "@/lib/types";

export function DeleteDialog({
  group,
  instanceData,
  deleting,
  deleteAgents,
  setDeleteAgents,
  onDelete,
  onClose,
}: {
  group: GroupedExtension;
  instanceData: Map<string, ExtContent>;
  deleting: boolean;
  deleteAgents: Set<string>;
  setDeleteAgents: (s: Set<string>) => void;
  onDelete: (agents: string[]) => void;
  onClose: () => void;
}) {
  const dlgRef = useRef<HTMLDivElement>(null);

  // Categorize instances
  const ownInstances: typeof group.instances = [];
  const sharedAgents: string[] = [];
  for (const inst of group.instances) {
    const data = instanceData.get(inst.id);
    if (data?.path?.includes("/.agents/skills")) {
      sharedAgents.push(...inst.agents);
    } else {
      ownInstances.push(inst);
    }
  }
  const hasShared = sharedAgents.length > 0;
  const hasOwn = ownInstances.length > 0;

  // Escape to close
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  // Focus trap: keep Tab cycling within the dialog
  useFocusTrap(dlgRef, true);

  // Reset selection when dialog opens
  useEffect(() => {
    setDeleteAgents(new Set());
  }, [setDeleteAgents]);

  return (
    <div
      className="absolute inset-0 z-50 flex items-center justify-center rounded-xl overflow-hidden"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* Backdrop — contained within the detail panel */}
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      {/* Dialog */}
      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label="Delete extension"
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-sm rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none"
      >
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/10 text-destructive">
            <Trash2 size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              Delete "{group.name}"
            </h3>
            <p className="text-xs text-muted-foreground">
              This action cannot be undone.
            </p>
          </div>
        </div>

        <div className="space-y-3">
          {/* Own-directory instances: per-agent deletion */}
          {hasOwn && (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                {ownInstances.length === 1
                  ? "This will permanently delete the skill file:"
                  : "Select agents to permanently delete from:"}
              </p>
              <div className="space-y-1.5 rounded-lg border border-border bg-muted/30 p-2.5">
                {ownInstances.length > 1 && (() => {
                  const allAgents = ownInstances.map((inst) => inst.agents[0]);
                  const allSelected = allAgents.every((a) => deleteAgents.has(a));
                  return (
                    <label className="flex items-start gap-2 text-xs cursor-pointer pb-1.5 mb-1.5 border-b border-border/50">
                      <input
                        type="checkbox"
                        checked={allSelected}
                        onChange={() => {
                          if (allSelected) {
                            setDeleteAgents(new Set());
                          } else {
                            setDeleteAgents(new Set(allAgents));
                          }
                        }}
                        className="mt-0.5 rounded border-border accent-destructive"
                      />
                      <span className="font-medium text-foreground">All Agents</span>
                    </label>
                  );
                })()}
                {ownInstances.map((inst) => {
                  const agent = inst.agents[0];
                  const data = instanceData.get(inst.id);
                  const sym = data?.symlink_target;
                  const isSingle = ownInstances.length === 1;
                  return (
                    <label
                      key={inst.id}
                      className="flex items-start gap-2 text-xs cursor-pointer"
                    >
                      {!isSingle && (
                        <input
                          type="checkbox"
                          checked={deleteAgents.has(agent)}
                          onChange={() => {
                            const next = new Set(deleteAgents);
                            if (next.has(agent)) next.delete(agent);
                            else next.add(agent);
                            setDeleteAgents(next);
                          }}
                          className="mt-0.5 rounded border-border accent-destructive"
                        />
                      )}
                      <div className="min-w-0">
                        <span className="font-medium text-foreground">
                          {agentDisplayName(agent)}
                        </span>
                        {data?.path && (
                          <p className="text-muted-foreground truncate">
                            {data.path}
                          </p>
                        )}
                        {sym && (
                          <p className="flex items-center gap-1 text-chart-5">
                            <Link size={10} className="shrink-0" />
                            <span className="truncate">{sym}</span>
                          </p>
                        )}
                      </div>
                    </label>
                  );
                })}
              </div>
              {ownInstances.length === 1 ? (
                <button
                  disabled={deleting}
                  onClick={() => onDelete(ownInstances[0].agents)}
                  className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
                >
                  {deleting ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Trash2 size={12} />
                  )}
                  Delete from {agentDisplayName(ownInstances[0].agents[0])}
                </button>
              ) : (
                <button
                  disabled={deleting || deleteAgents.size === 0}
                  onClick={() => onDelete(Array.from(deleteAgents))}
                  className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
                >
                  {deleting ? (
                    <Loader2 size={12} className="animate-spin" />
                  ) : (
                    <Trash2 size={12} />
                  )}
                  Delete selected ({deleteAgents.size})
                </button>
              )}
            </div>
          )}

          {/* Separator */}
          {hasOwn && hasShared && <hr className="border-border" />}

          {/* Shared directory: all-or-nothing */}
          {hasShared && (
            <div className="space-y-2">
              <div className="flex items-start gap-1.5 rounded-lg border border-chart-5/30 bg-chart-5/5 p-2.5 text-xs text-chart-5">
                <AlertTriangle size={12} className="mt-0.5 shrink-0" />
                <span>
                  This skill is in the shared directory{" "}
                  <span className="font-mono">~/.agents/skills/</span>. Deleting
                  it will remove access for{" "}
                  {sharedAgents.map(agentDisplayName).join(", ")}.
                </span>
              </div>
              <button
                disabled={deleting}
                onClick={() => onDelete(sharedAgents)}
                className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-destructive px-3 py-2 text-xs font-medium text-destructive-foreground hover:bg-destructive/90 disabled:opacity-50"
              >
                {deleting ? (
                  <Loader2 size={12} className="animate-spin" />
                ) : (
                  <Trash2 size={12} />
                )}
                Delete from shared directory
              </button>
            </div>
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
