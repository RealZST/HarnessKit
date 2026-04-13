import { Download, Loader2, Package } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useFocusTrap } from "@/hooks/use-focus-trap";
import type { NewRepoSkill } from "@/lib/types";
import { agentDisplayName, sortAgents } from "@/lib/types";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";

interface NewSkillsDialogProps {
  skills: NewRepoSkill[];
  onInstall: (url: string, skillIds: string[], targetAgents: string[]) => Promise<void>;
  onDismiss: () => void;
  onClose: () => void;
}

export function NewSkillsDialog({ skills, onInstall, onDismiss, onClose }: NewSkillsDialogProps) {
  const dlgRef = useRef<HTMLDivElement>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set(skills.map((s) => `${s.repo_url}::${s.skill_id}`)));
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(new Set());
  const [installing, setInstalling] = useState(false);

  const agents = useAgentStore((s) => s.agents);
  const agentOrder = useAgentStore((s) => s.agentOrder);
  const detectedAgents = sortAgents(
    agents.filter((a) => a.detected),
    agentOrder,
  );

  // If only one agent detected, auto-select it
  useEffect(() => {
    if (detectedAgents.length === 1) {
      setSelectedAgents(new Set([detectedAgents[0].name]));
    }
  }, [detectedAgents.length]);

  // Group skills by repo
  const grouped = new Map<string, { pack: string | null; skills: NewRepoSkill[] }>();
  for (const skill of skills) {
    const key = skill.repo_url;
    if (!grouped.has(key)) {
      grouped.set(key, { pack: skill.pack, skills: [] });
    }
    grouped.get(key)!.skills.push(skill);
  }

  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose]);

  useFocusTrap(dlgRef, true);

  const toggleSkill = (key: string) => {
    const next = new Set(selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    setSelected(next);
  };

  const toggleAgent = (name: string) => {
    const next = new Set(selectedAgents);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    setSelectedAgents(next);
  };

  const allAgentsSelected =
    detectedAgents.length > 0 &&
    detectedAgents.every((a) => selectedAgents.has(a.name));

  const toggleAllAgents = () => {
    if (allAgentsSelected) {
      setSelectedAgents(new Set());
    } else {
      setSelectedAgents(new Set(detectedAgents.map((a) => a.name)));
    }
  };

  const handleInstall = async () => {
    setInstalling(true);
    try {
      const targetAgents = [...selectedAgents];
      for (const [url, group] of grouped) {
        const selectedSkills = group.skills.filter((s) => selected.has(`${s.repo_url}::${s.skill_id}`));
        if (selectedSkills.length === 0) continue;
        const skillIds = selectedSkills.map((s) => s.skill_id);
        await onInstall(url, skillIds, targetAgents);
      }
      onClose();
    } catch (e: any) {
      toast.error(`Failed to install: ${e?.message ?? e}`);
    } finally {
      setInstalling(false);
    }
  };

  const selectedCount = selected.size;
  const canInstall = selectedCount > 0 && selectedAgents.size > 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="absolute inset-0 bg-background/80 backdrop-blur-[2px]" />

      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-label="New skills available"
        tabIndex={-1}
        className="relative z-10 w-[calc(100%-2rem)] max-w-md rounded-xl border border-border bg-card p-5 shadow-xl animate-fade-in outline-none max-h-[80vh] overflow-y-auto"
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-4">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <Package size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-foreground">
              More skills available from your installed repos
            </h3>
          </div>
        </div>

        {/* Skill list grouped by repo */}
        <div className="space-y-4">
          {[...grouped].map(([url, group]) => (
            <div key={url}>
              <p className="text-xs font-medium text-muted-foreground mb-2">
                {group.pack ?? url}
              </p>
              <div className="space-y-1.5">
                {group.skills.map((skill) => {
                  const key = `${skill.repo_url}::${skill.skill_id}`;
                  return (
                    <label
                      key={key}
                      className="flex items-start gap-2 rounded-lg border border-border px-3 py-2 cursor-pointer hover:bg-muted/50 transition-colors"
                    >
                      <input
                        type="checkbox"
                        checked={selected.has(key)}
                        onChange={() => toggleSkill(key)}
                        className="mt-0.5 shrink-0"
                      />
                      <div className="min-w-0">
                        <span className="text-sm font-medium text-foreground">
                          {skill.name}
                        </span>
                        {skill.description && (
                          <p className="text-xs text-muted-foreground line-clamp-2">
                            {skill.description}
                          </p>
                        )}
                      </div>
                    </label>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        {/* Agent selection */}
        {detectedAgents.length > 0 && (
          <div className="mt-4">
            <span className="text-xs text-muted-foreground">Install to</span>
            <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
              <label className="flex items-center gap-1.5 text-xs font-medium text-foreground">
                <input
                  type="checkbox"
                  checked={allAgentsSelected}
                  onChange={toggleAllAgents}
                  className="rounded border-border accent-primary"
                />
                All Agents
              </label>
              <span className="text-border">|</span>
              {detectedAgents.map((a) => (
                <label
                  key={a.name}
                  className="flex items-center gap-1.5 text-xs text-foreground"
                >
                  <input
                    type="checkbox"
                    checked={selectedAgents.has(a.name)}
                    onChange={() => toggleAgent(a.name)}
                    className="rounded border-border accent-primary"
                  />
                  {agentDisplayName(a.name)}
                </label>
              ))}
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="mt-4">
          <button
            onClick={handleInstall}
            disabled={!canInstall || installing}
            className="w-full flex items-center justify-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-xs font-medium text-primary-foreground disabled:opacity-50"
          >
            {installing ? (
              <Loader2 size={12} className="animate-spin" />
            ) : (
              <Download size={12} />
            )}
            {installing
              ? "Installing..."
              : `Install Selected (${selectedCount})`}
          </button>
          <button
            onClick={onDismiss}
            className="mt-2 w-full rounded-lg border border-border px-3 py-2 text-xs text-muted-foreground hover:bg-muted/50 transition-colors"
          >
            Not Now
          </button>
          <button
            onClick={onClose}
            className="mt-2 w-full rounded-lg px-3 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
