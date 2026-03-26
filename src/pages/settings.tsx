import { useUIStore } from "@/stores/ui-store";

export default function SettingsPage() {
  const { theme, setTheme } = useUIStore();

  return (
    <div className="max-w-2xl space-y-8">
      <h2 className="text-xl font-semibold">Settings</h2>

      <section className="space-y-4">
        <h3 className="text-sm font-medium text-zinc-400">Appearance</h3>
        <div className="flex items-center justify-between rounded-lg border border-zinc-800 bg-zinc-900/50 px-4 py-3">
          <span className="text-sm">Theme</span>
          <select
            value={theme}
            onChange={(e) => setTheme(e.target.value as "dark" | "light")}
            className="rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1 text-sm text-zinc-200"
          >
            <option value="dark">Dark</option>
            <option value="light">Light</option>
          </select>
        </div>
      </section>

      <section className="space-y-4">
        <h3 className="text-sm font-medium text-zinc-400">Agent Paths</h3>
        <p className="text-xs text-zinc-500">
          HarnessKit auto-detects agent directories. Override paths here if needed.
        </p>
        {["claude", "cursor", "codex", "gemini", "antigravity", "copilot"].map((agent) => (
          <div key={agent} className="flex items-center gap-4 rounded-lg border border-zinc-800 bg-zinc-900/50 px-4 py-3">
            <span className="w-28 text-sm text-zinc-300">{agent}</span>
            <input
              type="text"
              placeholder="Auto-detected"
              className="flex-1 rounded-md border border-zinc-700 bg-zinc-800 px-3 py-1 text-sm text-zinc-200 placeholder-zinc-600"
            />
          </div>
        ))}
      </section>
    </div>
  );
}
