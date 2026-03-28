import { useState, useEffect } from "react";
import { api } from "@/lib/invoke";
import { humanizeError } from "@/lib/errors";
import { useExtensionStore } from "@/stores/extension-store";
import { useAgentStore } from "@/stores/agent-store";

interface InstallDialogProps {
  open: boolean;
  onClose: () => void;
}

export function InstallDialog({ open, onClose }: InstallDialogProps) {
  const [url, setUrl] = useState("");
  const [skillId, setSkillId] = useState("");
  const [targetAgent, setTargetAgent] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fetch = useExtensionStore((s) => s.fetch);
  const { agents, fetch: fetchAgents } = useAgentStore();

  useEffect(() => { fetchAgents(); }, [fetchAgents]);

  const detectedAgents = agents.filter((a) => a.detected);

  // Default to first detected agent
  useEffect(() => {
    if (!targetAgent && detectedAgents.length > 0) {
      setTargetAgent(detectedAgents[0].name);
    }
  }, [detectedAgents, targetAgent]);

  // Reset form when closing
  useEffect(() => {
    if (!open) {
      setUrl("");
      setSkillId("");
      setError(null);
    }
  }, [open]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && open) onClose();
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [onClose, open]);

  const handleInstall = async () => {
    if (!url.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await api.installFromGit(url.trim(), targetAgent || undefined, skillId.trim() || undefined);
      await fetch();
      onClose();
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
        <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
          <h3 className="text-sm font-semibold">Install from Git</h3>
          <p className="mt-1 text-xs text-muted-foreground">Enter a Git repository URL containing a skill to install.</p>
          <div className="mt-3 flex flex-col gap-2 sm:flex-row sm:gap-3">
            <input
              type="text"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !loading && handleInstall()}
              placeholder="https://github.com/user/skill-repo.git"
              aria-label="Git repository URL"
              className="flex-1 rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
              autoFocus={open}
              disabled={loading}
            />
            <input
              type="text"
              value={skillId}
              onChange={(e) => setSkillId(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && !loading && handleInstall()}
              placeholder="Skill ID (optional)"
              aria-label="Skill ID"
              className="sm:w-48 rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
              disabled={loading}
            />
          </div>
          {detectedAgents.length > 1 && (
            <div className="mt-2">
              <label htmlFor="install-agent-select" className="text-xs text-muted-foreground">Install to agent</label>
              <select
                id="install-agent-select"
                value={targetAgent}
                onChange={(e) => setTargetAgent(e.target.value)}
                disabled={loading}
                className="mt-1 w-full sm:w-auto rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
              >
                {detectedAgents.map((a) => (
                  <option key={a.name} value={a.name}>{a.name}</option>
                ))}
              </select>
            </div>
          )}
          {error && (
            <div className="mt-2 rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive">
              {humanizeError(error)}
            </div>
          )}
          <div className="mt-3 flex items-center gap-2">
            <button
              onClick={handleInstall}
              disabled={loading || !url.trim()}
              className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
            >
              {loading ? "Installing..." : "Install"}
            </button>
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
