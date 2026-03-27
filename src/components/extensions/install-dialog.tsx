import { useState, useEffect } from "react";
import { api } from "@/lib/invoke";
import { useExtensionStore } from "@/stores/extension-store";
import { useAgentStore } from "@/stores/agent-store";
import { X } from "lucide-react";

export function InstallDialog({ onClose }: { onClose: () => void }) {
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-full max-w-md rounded-xl border border-border bg-card p-6 shadow-lg">
        <div className="flex items-center justify-between">
          <h3 className="text-lg font-semibold">Install from Git</h3>
          <button onClick={onClose} className="rounded-lg p-1 text-muted-foreground hover:text-foreground">
            <X size={18} />
          </button>
        </div>
        <p className="mt-2 text-sm text-muted-foreground">Enter a Git repository URL containing a skill to install.</p>
        <input
          type="text"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !loading && handleInstall()}
          placeholder="https://github.com/user/skill-repo.git"
          className="mt-3 w-full rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
          autoFocus
          disabled={loading}
        />
        <input
          type="text"
          value={skillId}
          onChange={(e) => setSkillId(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !loading && handleInstall()}
          placeholder="Skill ID (optional, for multi-skill repos)"
          className="mt-2 w-full rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
          disabled={loading}
        />
        {detectedAgents.length > 1 && (
          <div className="mt-3">
            <label className="text-xs text-muted-foreground">Install to agent</label>
            <select
              value={targetAgent}
              onChange={(e) => setTargetAgent(e.target.value)}
              disabled={loading}
              className="mt-1 w-full rounded-lg border border-border bg-muted px-3 py-2 text-sm outline-none focus:border-ring"
            >
              {detectedAgents.map((a) => (
                <option key={a.name} value={a.name}>{a.name}</option>
              ))}
            </select>
          </div>
        )}
        {error && (
          <p className="mt-2 text-sm text-red-500">{error}</p>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onClose}
            disabled={loading}
            className="rounded-lg px-4 py-2 text-sm text-muted-foreground hover:text-foreground"
          >
            Cancel
          </button>
          <button
            onClick={handleInstall}
            disabled={loading || !url.trim()}
            className="rounded-lg bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
          >
            {loading ? "Installing..." : "Install"}
          </button>
        </div>
      </div>
    </div>
  );
}
