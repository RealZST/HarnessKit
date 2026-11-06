import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Testing Library only auto-unmounts when vitest globals are on; they're off here.
afterEach(() => cleanup());

// jsdom has no IntersectionObserver; islands like OverviewLive construct one on mount.
if (typeof globalThis.IntersectionObserver === "undefined") {
  class IntersectionObserverStub {
    readonly root = null;
    readonly rootMargin = "0px";
    readonly thresholds: readonly number[] = [];
    observe() {}
    unobserve() {}
    disconnect() {}
    takeRecords(): IntersectionObserverEntry[] { return []; }
  }
  globalThis.IntersectionObserver = IntersectionObserverStub as unknown as typeof IntersectionObserver;
}

// jsdom has no matchMedia; islands read prefers-reduced-motion / prefers-color-scheme on mount.
if (typeof window !== "undefined" && typeof window.matchMedia === "undefined") {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener() {},
    removeEventListener() {},
    addListener() {},
    removeListener() {},
    dispatchEvent() { return false; },
  })) as unknown as typeof window.matchMedia;
}
