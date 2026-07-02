import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useCollapsibleState } from "../use-collapsible-state";

beforeEach(() => localStorage.clear());

describe("useCollapsibleState", () => {
  it("defaults to expanded and toggles + persists", () => {
    const { result } = renderHook(() => useCollapsibleState("k"));
    expect(result.current.collapsed).toBe(false);
    act(() => result.current.toggle());
    expect(result.current.collapsed).toBe(true);
    expect(localStorage.getItem("k")).toBe("1");
    act(() => result.current.setCollapsed(false));
    expect(localStorage.getItem("k")).toBeNull();
  });

  it("rehydrates collapsed=true from storage", () => {
    localStorage.setItem("k2", "1");
    const { result } = renderHook(() => useCollapsibleState("k2"));
    expect(result.current.collapsed).toBe(true);
  });

  it("null key is session-only (no throw, no persistence)", () => {
    const { result } = renderHook(() => useCollapsibleState(null));
    act(() => result.current.toggle());
    expect(result.current.collapsed).toBe(true);
    expect(localStorage.length).toBe(0);
  });
});
