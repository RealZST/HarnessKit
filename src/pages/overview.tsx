import { useEffect, useState } from "react";
import { StatCard } from "@/components/shared/stat-card";
import { Package, Server, Puzzle, Webhook, AlertTriangle } from "lucide-react";
import type { DashboardStats } from "@/lib/types";
import { api } from "@/lib/invoke";

export default function OverviewPage() {
  const [stats, setStats] = useState<DashboardStats | null>(null);

  useEffect(() => {
    api.getDashboardStats().then(setStats);
  }, []);

  if (!stats) {
    return <div className="text-zinc-500">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <h2 className="text-xl font-semibold">Overview</h2>

      <div className="grid grid-cols-5 gap-4">
        <StatCard label="Skills" value={stats.skill_count} icon={<Package size={18} />} />
        <StatCard label="MCP Servers" value={stats.mcp_count} icon={<Server size={18} />} />
        <StatCard label="Plugins" value={stats.plugin_count} icon={<Puzzle size={18} />} />
        <StatCard label="Hooks" value={stats.hook_count} icon={<Webhook size={18} />} />
        <StatCard
          label="Issues"
          value={stats.critical_issues + stats.high_issues}
          icon={<AlertTriangle size={18} />}
          className={stats.critical_issues > 0 ? "border-red-900/50" : undefined}
        />
      </div>

      <div className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
        <h3 className="text-sm font-medium text-zinc-400">Total Extensions</h3>
        <p className="mt-1 text-4xl font-bold">{stats.total_extensions}</p>
        <p className="mt-1 text-sm text-zinc-500">
          {stats.skill_count} skills · {stats.mcp_count} mcp · {stats.plugin_count} plugins · {stats.hook_count} hooks
        </p>
      </div>
    </div>
  );
}
