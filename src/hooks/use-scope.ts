import { useScopeStore, type ScopeValue } from "@/stores/scope-store";

function computeScopeId(scope: ScopeValue): string {
  if (scope.type === "all") return "all";
  if (scope.type === "global") return "global";
  return scope.path;
}

/** Read + write the current scope. setScope only mutates the store; URL
 *  sync is handled by AppShell Effect 3 (store → URL).
 *
 *  Why no inline navigate(): a previous version of this hook called
 *  navigate({ search: ... }) inside setScope to mirror the URL eagerly,
 *  but that fought any *follow-up* navigate() in the same tick — e.g.
 *  Overview's "click an agent file" handler does `setScope(file.scope);
 *  navigate('/agents?...')`, and React Router would batch the two and
 *  drop the second navigate. Letting the AppShell effect handle URL
 *  sync asynchronously sidesteps the conflict and gives us a single
 *  authoritative direction (store → URL). */
export function useScope() {
  const scope = useScopeStore((s) => s.current);
  const setScope = useScopeStore((s) => s.setScope);

  return {
    scope,
    scopeId: computeScopeId(scope),
    isAll: scope.type === "all",
    setScope,
  };
}
