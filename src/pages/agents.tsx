import { useEffect, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { AgentDetail } from "@/components/agents/agent-detail";
import { AgentList } from "@/components/agents/agent-list";
import { useScope } from "@/hooks/use-scope";
import { useAgentConfigStore } from "@/stores/agent-config-store";
import { useProjectStore } from "@/stores/project-store";
import { type ScopeValue, useScopeStore } from "@/stores/scope-store";

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

  useEffect(() => {
    const agent = searchParams.get("agent");
    const file = searchParams.get("file");
    if (!loading && agent) {
      // Apply incoming scope FIRST so the file (which may belong to a
      // different scope than the user's current one — e.g. clicking a
      // global file from Overview while in a project scope) actually
      // renders in the list. Only does work for *deep links* (presence of
      // `agent` query param); a plain visit to /agents preserves the
      // user's current scope selection.
      const urlScope = searchParams.get("scope");
      const targetScope: ScopeValue =
        urlScope == null
          ? { type: "global" }
          : urlScope === "all"
            ? { type: "all" }
            : ((): ScopeValue => {
                const proj = projects.find((p) => p.path === urlScope);
                return proj
                  ? { type: "project", name: proj.name, path: proj.path }
                  : { type: "global" };
              })();
      const sameScope =
        targetScope.type === scope.type &&
        (targetScope.type !== "project" ||
          (scope.type === "project" && targetScope.path === scope.path));
      if (!sameScope) {
        setScope(targetScope);
        // Sync prevScopeRef immediately so the scope-change cleanup effect
        // (line 28) does NOT clear the expandedFiles + pendingFocusFile we
        // are about to set below — without this, the deep-link's focus
        // signal is wiped on the next render.
        prevScopeRef.current = targetScope;
      }
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
