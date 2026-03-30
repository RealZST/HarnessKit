import { useState, useEffect, useRef } from "react";
import { api } from "@/lib/invoke";
import { sortAgents, agentDisplayName } from "@/lib/types";
import type { DiscoveredSkill } from "@/lib/types";
import { humanizeError } from "@/lib/errors";
import { useExtensionStore } from "@/stores/extension-store";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";
import { ChevronLeft } from "lucide-react";

interface InstallDialogProps {
  open: boolean;
  onClose: () => void;
}

type Phase = "input" | "select-skills";

export function InstallDialog({ open, onClose }: InstallDialogProps) {
  const [url, setUrl] = useState("");
  const [selectedAgents, setSelectedAgents] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("input");
  const [discoveredSkills, setDiscoveredSkills] = useState<DiscoveredSkill[]>([]);
  const [selectedSkills, setSelectedSkills] = useState<Set<string>>(new Set());
  const [cloneId, setCloneId] = useState<string | null>(null);
  const fetch = useExtensionStore((s) => s.fetch);
  const { agents, fetch: fetchAgents, agentOrder } = useAgentStore();
  const triggerRef = useRef<HTMLElement | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => { fetchAgents(); }, [fetchAgents]);

  const detectedAgents = sortAgents(agents.filter((a) => a.detected), agentOrder);

  // If only one agent detected, auto-select it
  useEffect(() => {
    if (detectedAgents.length === 1) {
      setSelectedAgents(new Set([detectedAgents[0].name]));
    }
  }, [detectedAgents]);

  // Store trigger element on open, restore focus on close
  useEffect(() => {
    if (open) {
      triggerRef.current = document.activeElement as HTMLElement;
      requestAnimationFrame(() => {
        dialogRef.current?.querySelector<HTMLElement>("input:not([disabled])")?.focus();
      });
    } else if (triggerRef.current) {
      triggerRef.current.focus();
      triggerRef.current = null;
    }
  }, [open]);

  // Reset form when closing
  useEffect(() => {
    if (!open) {
      setUrl("");
      setError(null);
      setPhase("input");
      setDiscoveredSkills([]);
      setSelectedSkills(new Set());
      setCloneId(null);
    }
  }, [open]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && open) onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose, open]);

  // Focus trap
  useEffect(() => {
    if (!open || !dialogRef.current) return;
    const dialog = dialogRef.current;
    const handleTab = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const focusable = dialog.querySelectorAll<HTMLElement>(
        "input:not([disabled]), select:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex=\"-1\"])"
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleTab);
    return () => document.removeEventListener("keydown", handleTab);
  }, [open]);

  const toggleAgent = (name: string) => {
    setSelectedAgents((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name); else next.add(name);
      return next;
    });
  };

  const allAgentsSelected = detectedAgents.length > 0 && detectedAgents.every((a) => selectedAgents.has(a.name));
  const toggleAllAgents = () => {
    if (allAgentsSelected) {
      setSelectedAgents(new Set());
    } else {
      setSelectedAgents(new Set(detectedAgents.map((a) => a.name)));
    }
  };

  const toggleSkill = (skillId: string) => {
    setSelectedSkills((prev) => {
      const next = new Set(prev);
      if (next.has(skillId)) next.delete(skillId); else next.add(skillId);
      return next;
    });
  };

  const allSkillsSelected = discoveredSkills.length > 0 && discoveredSkills.every((s) => selectedSkills.has(s.skill_id));
  const toggleAllSkills = () => {
    if (allSkillsSelected) {
      setSelectedSkills(new Set());
    } else {
      setSelectedSkills(new Set(discoveredSkills.map((s) => s.skill_id)));
    }
  };

  const handleScan = async () => {
    if (!url.trim() || selectedAgents.size === 0) return;
    setLoading(true);
    setError(null);
    try {
      const result = await api.scanGitRepo(url.trim(), [...selectedAgents]);
      if (result.type === "Installed") {
        await fetch();
        onClose();
        toast.success(`${result.result.name} installed`);
      } else if (result.type === "MultipleSkills") {
        setDiscoveredSkills(result.skills);
        setSelectedSkills(new Set(result.skills.map((s) => s.skill_id)));
        setCloneId(result.clone_id);
        setPhase("select-skills");
      } else {
        setError("No skills found in repository");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleInstallSelected = async () => {
    if (!cloneId || selectedSkills.size === 0) return;
    setLoading(true);
    setError(null);
    try {
      const results = await api.installScannedSkills(cloneId, [...selectedSkills], [...selectedAgents]);
      await fetch();
      onClose();
      const names = results.map((r) => r.name);
      toast.success(names.length === 1
        ? `${names[0]} installed`
        : `${names.length} skills installed`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="grid transition-[grid-template-rows] duration-[250ms]"
      style={{ gridTemplateRows: open ? '1fr' : '0fr' }}
    >
      <div className="overflow-hidden">
        <div ref={dialogRef} role="dialog" aria-modal="true" className="rounded-xl border border-border bg-card p-4 shadow-sm">
          {phase === "input" ? (
            <>
              <h3 className="text-sm font-semibold">Install from Git</h3>
              <p className="mt-1 text-xs text-muted-foreground">Enter a Git repository URL containing a skill to install.</p>
              <div className="mt-3">
                <input
                  type="text"
                  value={url}
                  onChange={(e) => setUrl(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && !loading && handleScan()}
                  placeholder="https://github.com/user/skill-repo.git"
                  aria-label="Git repository URL"
                  aria-required="true"
                  aria-describedby={error ? "install-error" : undefined}
                  className="w-full rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring focus:ring-2 focus:ring-ring/50"
                  autoFocus={open}
                  disabled={loading}
                />
              </div>
              {detectedAgents.length > 1 && (
                <div className="mt-3">
                  <span className="text-xs text-muted-foreground">Install to</span>
                  <div className="mt-1.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
                    <label className="flex items-center gap-1.5 text-xs font-medium text-foreground">
                      <input
                        type="checkbox"
                        checked={allAgentsSelected}
                        onChange={toggleAllAgents}
                        disabled={loading}
                        className="rounded border-border accent-primary"
                      />
                      All Agents
                    </label>
                    <span className="text-border">|</span>
                    {detectedAgents.map((a) => (
                      <label key={a.name} className="flex items-center gap-1.5 text-xs text-foreground">
                        <input
                          type="checkbox"
                          checked={selectedAgents.has(a.name)}
                          onChange={() => toggleAgent(a.name)}
                          disabled={loading}
                          className="rounded border-border accent-primary"
                        />
                        {agentDisplayName(a.name)}
                      </label>
                    ))}
                  </div>
                </div>
              )}
            </>
          ) : (
            <>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => { setPhase("input"); setError(null); }}
                  disabled={loading}
                  className="shrink-0 rounded-lg p-1 text-muted-foreground hover:text-foreground"
                  aria-label="Back"
                >
                  <ChevronLeft size={16} />
                </button>
                <div>
                  <h3 className="text-sm font-semibold">Select Skills to Install</h3>
                  <p className="text-xs text-muted-foreground">{discoveredSkills.length} skills found in repository</p>
                </div>
              </div>
              <div className="mt-3 space-y-1.5">
                <label className="flex items-center gap-2 rounded-lg px-3 py-2 text-sm font-medium text-foreground hover:bg-muted/30 transition-colors">
                  <input
                    type="checkbox"
                    checked={allSkillsSelected}
                    onChange={toggleAllSkills}
                    disabled={loading}
                    className="rounded border-border accent-primary"
                  />
                  All Skills
                </label>
                <div className="border-t border-border/50" />
                {discoveredSkills.map((skill) => (
                  <label
                    key={skill.skill_id}
                    className="flex items-start gap-2 rounded-lg px-3 py-2 text-sm hover:bg-muted/30 transition-colors"
                  >
                    <input
                      type="checkbox"
                      checked={selectedSkills.has(skill.skill_id)}
                      onChange={() => toggleSkill(skill.skill_id)}
                      disabled={loading}
                      className="mt-0.5 rounded border-border accent-primary"
                    />
                    <div className="min-w-0">
                      <span className="font-medium text-foreground">{skill.name}</span>
                      {skill.description && (
                        <p className="text-xs text-muted-foreground line-clamp-1">{skill.description}</p>
                      )}
                    </div>
                  </label>
                ))}
              </div>
            </>
          )}

          {error && (
            <div id="install-error" className="mt-2 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
              {humanizeError(error)}
            </div>
          )}
          <div className="mt-3 flex items-center gap-2">
            {phase === "input" ? (
              <button
                onClick={handleScan}
                disabled={loading || !url.trim() || selectedAgents.size === 0}
                className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                {loading ? "Scanning..." : "Install"}
              </button>
            ) : (
              <button
                onClick={handleInstallSelected}
                disabled={loading || selectedSkills.size === 0}
                className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
              >
                {loading ? "Installing..." : `Install${selectedSkills.size > 0 ? ` (${selectedSkills.size})` : ""}`}
              </button>
            )}
            <button
              onClick={onClose}
              disabled={loading}
              className="rounded-lg px-4 py-2 text-sm text-muted-foreground hover:text-foreground"
            >
              Cancel
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
