import { useEffect } from "react";
import { useSearchParams } from "react-router-dom";
import { AgentDetail } from "@/components/agents/agent-detail";
import { AgentList } from "@/components/agents/agent-list";
import { useAgentConfigStore } from "@/stores/agent-config-store";

export default function AgentsPage() {
  const fetch = useAgentConfigStore((s) => s.fetch);
  const loading = useAgentConfigStore((s) => s.loading);
  const selectAgent = useAgentConfigStore((s) => s.selectAgent);
  const expandFile = useAgentConfigStore((s) => s.expandFile);
  const setPendingFocusFile = useAgentConfigStore(
    (s) => s.setPendingFocusFile,
  );
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    fetch();
  }, [fetch]);

  useEffect(() => {
    const agent = searchParams.get("agent");
    const file = searchParams.get("file");
    if (!loading && agent) {
      selectAgent(agent);
      if (file) {
        // expandFile opens the file's preview pane; pendingFocusFile is what
        // the detail page uses to force-open the (possibly collapsed) parent
        // section and scroll/highlight the row.
        expandFile(file);
        setPendingFocusFile(file);
      }
      setSearchParams({}, { replace: true });
    }
  }, [
    loading,
    searchParams,
    selectAgent,
    expandFile,
    setPendingFocusFile,
    setSearchParams,
  ]);

  return (
    <div className="flex h-full">
      <div className="w-[160px] shrink-0 border-r border-border overflow-y-auto overscroll-contain">
        <AgentList />
      </div>
      {loading ? (
        <div className="flex flex-1 items-center justify-center text-muted-foreground text-sm">
          Loading...
        </div>
      ) : (
        <AgentDetail />
      )}
    </div>
  );
}
