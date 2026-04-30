import {
  act,
  fireEvent,
  render,
  renderHook,
  screen,
} from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
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

  it("setScope writes scope to URL via replace", () => {
    // Render with a starting URL so we can observe what changes
    let currentSearch = "";
    const Probe = () => {
      const location = useLocation();
      currentSearch = location.search;
      return null;
    };
    const Harness = () => {
      const { setScope } = useScope();
      return (
        <>
          <Probe />
          <button onClick={() => setScope({ type: "all" })}>set-all</button>
          <button onClick={() => setScope({ type: "global" })}>set-global</button>
        </>
      );
    };
    render(
      <MemoryRouter initialEntries={["/extensions?scope=global"]}>
        <Harness />
      </MemoryRouter>,
    );
    fireEvent.click(screen.getByText("set-all"));
    expect(currentSearch).toBe("?scope=all");
    fireEvent.click(screen.getByText("set-global"));
    expect(currentSearch).toBe(""); // global → param removed
  });
});
