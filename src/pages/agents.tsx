import { useEffect, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { AgentDetail } from "@/components/agents/agent-detail";
import { AgentList } from "@/components/agents/agent-list";
import { useScope } from "@/hooks/use-scope";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { useProjectStore } from "@/stores/project-store";
import {
  resolveDeepLinkScope,
  scopesEqual,
  useScopeStore,
} from "@/stores/scope-store";

export default function AgentsPage() {
  const hydrated = useScopeStore((s) => s.hydrated);
  const fetch = useAgentConfigStore((s) => s.fetch);
  const loading = useAgentConfigStore((s) => s.loading);
  const selectAgent = useAgentConfigStore((s) => s.selectAgent);
  const expandFile = useAgentConfigStore((s) => s.expandFile);
  const setPendingFocusFile = useAgentConfigStore((s) => s.setPendingFocusFile);
  const { scope, setScope } = useScope();
  const projects = useProjectStore((s) => s.projects);
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    if (!hydrated) return;
    fetch();
  }, [fetch, hydrated]);

  // When the user switches scope (e.g., via the Sidebar ScopeSwitcher), collapse
  // all expanded file previews and drop any pending focus signal — the file
  // visible just before the switch may not exist (or differ) in the new scope.
  const prevScopeRef = useRef(scope);
  useEffect(() => {
    if (prevScopeRef.current !== scope) {
      useAgentConfigStore.setState({
        expandedFiles: new Set(),
        pendingFocusFile: null,
      });
      prevScopeRef.current = scope;
    }
  }, [scope]);

  // Collapse expansions when leaving the page so revisiting starts clean.
  // expandedFiles lives in zustand (persists across remounts) — without this,
  // navigating to Extensions and back would keep an old preview pane open.
  useEffect(() => {
    return () => {
      useAgentConfigStore.setState({
        expandedFiles: new Set(),
        pendingFocusFile: null,
      });
    };
  }, []);

  // Deep-link handler: applies ?scope= and selects the target agent + file.
  // Pre-syncs prevScopeRef so the scope-change cleanup above doesn't wipe
  // the focus signal we're about to set.
  useEffect(() => {
    const agent = searchParams.get("agent");
    if (loading || !agent) return;
    const file = searchParams.get("file");
    const targetScope = resolveDeepLinkScope(searchParams.get("scope"), projects);
    if (!scopesEqual(targetScope, scope)) {
      setScope(targetScope);
      prevScopeRef.current = targetScope;
    }
    selectAgent(agent);
    if (file) {
      expandFile(file);
      setPendingFocusFile(file);
    }
    setSearchParams({}, { replace: true });
  }, [
    loading,
    searchParams,
    scope,
    setScope,
    projects,
    selectAgent,
    expandFile,
    setPendingFocusFile,
    setSearchParams,
  ]);

  if (!hydrated) {
    return <div className="p-4 text-sm text-muted-foreground">Loading...</div>;
  }

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
