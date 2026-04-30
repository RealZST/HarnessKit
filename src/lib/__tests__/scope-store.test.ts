import { beforeEach, describe, expect, it } from "vitest";
import type { Project } from "@/lib/types";
import { useScopeStore } from "@/stores/scope-store";

const makeProject = (name: string, path: string): Project => ({
  id: name,
  name,
  path,
  created_at: "2026-01-01T00:00:00Z",
  exists: true,
});

beforeEach(() => {
  localStorage.clear();
  useScopeStore.setState({ current: { type: "global" }, hydrated: false });
});

describe("scope-store hydrate", () => {
  it("uses URL scope when valid project path", () => {
    const projects = [makeProject("alpha", "/Users/me/alpha")];
    useScopeStore.getState().hydrate("/Users/me/alpha", projects);
    expect(useScopeStore.getState().current).toEqual({
      type: "project",
      name: "alpha",
      path: "/Users/me/alpha",
    });
  });

  it("uses URL 'global' value", () => {
    useScopeStore.getState().hydrate("global", []);
    expect(useScopeStore.getState().current).toEqual({ type: "global" });
  });

  it("uses URL 'all' when projects exist", () => {
    useScopeStore.getState().hydrate("all", [makeProject("a", "/p")]);
    expect(useScopeStore.getState().current).toEqual({ type: "all" });
  });

  it("coerces URL 'all' to global when no projects", () => {
    useScopeStore.getState().hydrate("all", []);
    expect(useScopeStore.getState().current).toEqual({ type: "global" });
  });

  it("falls back to localStorage when URL is null", () => {
    localStorage.setItem(
      "HK_SCOPE_LAST_USED",
      JSON.stringify({ type: "project", name: "beta", path: "/b" }),
    );
    useScopeStore.getState().hydrate(null, [makeProject("beta", "/b")]);
    expect(useScopeStore.getState().current).toEqual({
      type: "project",
      name: "beta",
      path: "/b",
    });
  });

  it("falls back to global when localStorage is invalid", () => {
    localStorage.setItem("HK_SCOPE_LAST_USED", "not-json{{");
    useScopeStore.getState().hydrate(null, []);
    expect(useScopeStore.getState().current).toEqual({ type: "global" });
  });

  it("sets hydrated true after hydrate", () => {
    useScopeStore.getState().hydrate(null, []);
    expect(useScopeStore.getState().hydrated).toBe(true);
  });

  it("writes the resolved value back to localStorage", () => {
    useScopeStore.getState().hydrate("all", []); // coerces to global
    expect(localStorage.getItem("HK_SCOPE_LAST_USED")).toBe(
      JSON.stringify({ type: "global" }),
    );
  });

  it("URL with unknown project path falls through to global when project list is empty", () => {
    useScopeStore.getState().hydrate("/Users/me/gone", []);
    expect(useScopeStore.getState().current).toEqual({ type: "global" });
  });

  it("localStorage with deleted project falls back to global", () => {
    localStorage.setItem(
      "HK_SCOPE_LAST_USED",
      JSON.stringify({ type: "project", name: "old", path: "/p/old" }),
    );
    useScopeStore.getState().hydrate(null, []);
    expect(useScopeStore.getState().current).toEqual({ type: "global" });
  });
});
