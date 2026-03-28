import { useEffect, useState } from "react";
import { StatCard } from "@/components/shared/stat-card";
import { Package, Server, Puzzle, Webhook } from "lucide-react";
import type { DashboardStats } from "@/lib/types";
import { api } from "@/lib/invoke";

export default function OverviewPage() {
  const [stats, setStats] = useState<DashboardStats | null>(null);

  useEffect(() => {
    api.getDashboardStats().then(setStats);
  }, []);

  if (!stats) {
    return (
      <div className="space-y-6" aria-live="polite">
        <h2 className="text-2xl font-bold tracking-tight">Overview</h2>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="animate-shimmer h-24 rounded-xl bg-muted" />
          ))}
        </div>
        <div className="animate-shimmer h-32 rounded-xl bg-muted" />
      </div>
    );
  }

  return (
    <div className="animate-fade-in space-y-6">
      <h2 className="text-2xl font-bold tracking-tight">Overview</h2>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <div title="Skills installed across all agents">
          <StatCard label="Skills" value={stats.skill_count} icon={<Package size={18} />} className="h-full" />
        </div>
        <div title="MCP server configurations detected">
          <StatCard label="MCP Servers" value={stats.mcp_count} icon={<Server size={18} />} className="h-full" />
        </div>
        <div title="Plugin extensions installed">
          <StatCard label="Plugins" value={stats.plugin_count} icon={<Puzzle size={18} />} className="h-full" />
        </div>
        <div title="Hook configurations active">
          <StatCard label="Hooks" value={stats.hook_count} icon={<Webhook size={18} />} className="h-full" />
        </div>
      </div>

      <div className="rounded-xl border border-border bg-gradient-to-br from-card to-muted/30 p-6 shadow-sm">
        <h3 className="text-sm font-medium text-muted-foreground">Total Extensions</h3>
        <p className="mt-1 font-serif text-4xl font-bold tracking-tight">{stats.total_extensions}</p>
        <p className="mt-2 text-sm tracking-wide text-muted-foreground">
          Across all detected agents — {stats.skill_count} skills · {stats.mcp_count} MCP · {stats.plugin_count} plugins · {stats.hook_count} hooks
        </p>
      </div>
    </div>
  );
}
