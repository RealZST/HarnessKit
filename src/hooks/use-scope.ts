import { useCallback } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { type ScopeValue, useScopeStore } from "@/stores/scope-store";

function scopeToUrlValue(scope: ScopeValue): string | null {
  if (scope.type === "global") return null; // default → no param
  if (scope.type === "all") return "all";
  return scope.path;
}

function computeScopeId(scope: ScopeValue): string {
  if (scope.type === "all") return "all";
  if (scope.type === "global") return "global";
  return scope.path;
}

export function useScope() {
  const scope = useScopeStore((s) => s.current);
  const setScopeStore = useScopeStore((s) => s.setScope);
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();

  const setScope = useCallback(
    (next: ScopeValue) => {
      setScopeStore(next);
      // Mirror to URL via replace (don't pollute browser history with scope changes)
      const params = new URLSearchParams(searchParams);
      const urlValue = scopeToUrlValue(next);
      if (urlValue == null) params.delete("scope");
      else params.set("scope", urlValue);
      const search = params.toString();
      navigate({ search: search ? `?${search}` : "" }, { replace: true });
    },
    [setScopeStore, searchParams, navigate],
  );

  return {
    scope,
    scopeId: computeScopeId(scope),
    isAll: scope.type === "all",
    setScope,
  };
}
