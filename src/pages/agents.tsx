import { useEffect } from "react";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { AgentList } from "@/components/agents/agent-list";
import { AgentDetail } from "@/components/agents/agent-detail";

export default function AgentsPage() {
  const fetch = useAgentConfigStore((s) => s.fetch);
  const loading = useAgentConfigStore((s) => s.loading);

  useEffect(() => { fetch(); }, [fetch]);

  return (
    <div className="flex h-full">
      <div className="w-[160px] shrink-0 border-r border-border overflow-y-auto overscroll-contain">
        <AgentList />
      </div>
      {loading ? (
        <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">Loading...</div>
      ) : (
        <AgentDetail />
      )}
    </div>
  );
}
