import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PLAY_RATIO, useMockCycle, type MockStep } from "./use-mock-cycle";

const PERIOD = 1000;

// Replaces IntersectionObserver; `see(ratio)` reports that share of the window on-screen (0 = off-screen).
function stubIntersection() {
  let report: (entries: Pick<IntersectionObserverEntry, "isIntersecting" | "intersectionRatio">[]) => void = () => {};
  vi.stubGlobal("IntersectionObserver", class {
    constructor(callback: typeof report) { report = callback; }
    observe() {}
    disconnect() {}
  });
  return (ratio: number) => act(() => report([{ isIntersecting: ratio > 0, intersectionRatio: ratio }]));
}

function setTabHidden(hidden: boolean) {
  vi.spyOn(document, "hidden", "get").mockReturnValue(hidden);
  act(() => { document.dispatchEvent(new Event("visibilitychange")); });
}

const fire = (el: HTMLElement, type: string, init?: FocusEventInit) =>
  act(() => { el.dispatchEvent(type.startsWith("focus") ? new FocusEvent(type, { bubbles: true, ...init }) : new Event(type)); });

// The hook on a fresh window element, on-screen or not yet.
function mount(steps: readonly MockStep[]) {
  const el = document.createElement("div");
  const target = { current: el };
  const hook = renderHook(() => useMockCycle(steps, target, PERIOD));
  return { el, ...hook };
}

describe("useMockCycle", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("runs each step at its offset once the window is on-screen", () => {
    const see = stubIntersection();
    const first = vi.fn();
    const second = vi.fn();
    mount([[100, first], [300, second]]);
    see(1);

    vi.advanceTimersByTime(99);
    expect(first).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).not.toHaveBeenCalled();
    vi.advanceTimersByTime(200);
    expect(second).toHaveBeenCalledTimes(1);
  });

  it("starts over every period, from its first frame, for as long as the window is on-screen", () => {
    const see = stubIntersection();
    const order: string[] = [];
    mount([[0, () => order.push("reset")], [100, () => order.push("a")], [900, () => order.push("b")]]);
    see(1);

    vi.advanceTimersByTime(PERIOD * 3);
    expect(order).toEqual(["reset", "a", "b", "reset", "a", "b", "reset", "a", "b"]);
  });

  it("waits until enough of the window is on-screen", () => {
    const see = stubIntersection();
    const step = vi.fn();
    mount([[100, step]]);

    see(PLAY_RATIO / 2);
    vi.advanceTimersByTime(PERIOD);
    expect(step).not.toHaveBeenCalled();
    see(PLAY_RATIO);
    vi.advanceTimersByTime(100);
    expect(step).toHaveBeenCalledTimes(1);
  });

  // WCAG 2.2.2: a reader can stop the loop by pointing at the window.
  it("holds where it is while the pointer is over the window, and carries on from there after", () => {
    const see = stubIntersection();
    const step = vi.fn();
    const { el } = mount([[300, step]]);
    see(1);
    vi.advanceTimersByTime(200);

    fire(el, "pointerenter");
    expect(el).toHaveAttribute("data-held");
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(PERIOD * 5);
    expect(step).not.toHaveBeenCalled();

    fire(el, "pointerleave");
    expect(el).not.toHaveAttribute("data-held");
    vi.advanceTimersByTime(99);
    expect(step).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(step).toHaveBeenCalledTimes(1);
  });

  it("holds while focus is inside the window, for keyboard readers", () => {
    const see = stubIntersection();
    const step = vi.fn();
    const { el } = mount([[300, step]]);
    see(1);

    fire(el, "focusin");
    vi.advanceTimersByTime(PERIOD);
    expect(step).not.toHaveBeenCalled();
    fire(el, "focusout");
    vi.advanceTimersByTime(300);
    expect(step).toHaveBeenCalledTimes(1);
  });

  it("doesn't start while the pointer is already over the window", () => {
    const see = stubIntersection();
    const step = vi.fn();
    const { el } = mount([[100, step]]);
    fire(el, "pointerenter");
    see(1);

    vi.advanceTimersByTime(PERIOD);
    expect(step).not.toHaveBeenCalled();
    fire(el, "pointerleave");
    vi.advanceTimersByTime(100);
    expect(step).toHaveBeenCalledTimes(1);
  });

  it("holds while the tab is hidden, and doesn't start while it is", () => {
    const see = stubIntersection();
    const step = vi.fn();
    mount([[0, () => {}], [500, step]]);
    setTabHidden(true);
    see(1);
    vi.advanceTimersByTime(PERIOD);
    expect(step).not.toHaveBeenCalled();

    setTabHidden(false);
    vi.advanceTimersByTime(400);
    setTabHidden(true);
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(PERIOD);
    expect(step).not.toHaveBeenCalled();
    setTabHidden(false);
    vi.advanceTimersByTime(100);
    expect(step).toHaveBeenCalledTimes(1);
  });

  it("stops and rewinds to the first frame once the window is fully off-screen, and starts afresh when it is back", () => {
    const see = stubIntersection();
    const order: string[] = [];
    mount([[0, () => order.push("reset")], [100, () => order.push("a")], [300, () => order.push("b")]]);
    see(1);
    vi.advanceTimersByTime(150);

    see(PLAY_RATIO / 2);
    expect(order).toEqual(["reset", "a"]);
    see(0);
    expect(order).toEqual(["reset", "a", "reset"]);
    expect(vi.getTimerCount()).toBe(0);

    see(1);
    vi.advanceTimersByTime(100);
    expect(order).toEqual(["reset", "a", "reset", "reset", "a"]);
  });

  it("runs nothing after unmount and leaves no timers behind", () => {
    const see = stubIntersection();
    const step = vi.fn();
    const { unmount } = mount([[100, step]]);
    see(1);

    unmount();
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(5000);
    expect(step).not.toHaveBeenCalled();
  });

  it("schedules nothing and reports reduced motion when the visitor asks for it", () => {
    vi.spyOn(window, "matchMedia").mockImplementation((query: string) => ({
      matches: query === "(prefers-reduced-motion: reduce)",
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false,
    }));
    const see = stubIntersection();
    const step = vi.fn();
    const { result } = mount([[100, step]]);
    see(1);

    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(5000);
    expect(step).not.toHaveBeenCalled();
    expect(result.current).toBe(true);
  });
});
