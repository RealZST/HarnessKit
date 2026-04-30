import { act, renderHook } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it } from "vitest";
import { useScope } from "@/hooks/use-scope";
import { useScopeStore } from "@/stores/scope-store";

beforeEach(() => {
  localStorage.clear();
  useScopeStore.setState({ current: { type: "global" }, hydrated: true });
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <MemoryRouter>{children}</MemoryRouter>
);

describe("useScope", () => {
  it("returns current scope from store", () => {
    const { result } = renderHook(() => useScope(), { wrapper });
    expect(result.current.scope).toEqual({ type: "global" });
    expect(result.current.scopeId).toBe("global");
    expect(result.current.isAll).toBe(false);
  });

  it("setScope updates store and writes localStorage", () => {
    const { result } = renderHook(() => useScope(), { wrapper });
    act(() => {
      result.current.setScope({
        type: "project",
        name: "alpha",
        path: "/p/alpha",
      });
    });
    expect(useScopeStore.getState().current).toEqual({
      type: "project",
      name: "alpha",
      path: "/p/alpha",
    });
    expect(localStorage.getItem("HK_SCOPE_LAST_USED")).toBe(
      JSON.stringify({ type: "project", name: "alpha", path: "/p/alpha" }),
    );
  });

  it("scopeId returns 'all' for all scope", () => {
    useScopeStore.setState({ current: { type: "all" }, hydrated: true });
    const { result } = renderHook(() => useScope(), { wrapper });
    expect(result.current.scopeId).toBe("all");
    expect(result.current.isAll).toBe(true);
  });

  it("scopeId returns project path for project scope", () => {
    useScopeStore.setState({
      current: { type: "project", name: "x", path: "/p/x" },
      hydrated: true,
    });
    const { result } = renderHook(() => useScope(), { wrapper });
    expect(result.current.scopeId).toBe("/p/x");
  });
});
