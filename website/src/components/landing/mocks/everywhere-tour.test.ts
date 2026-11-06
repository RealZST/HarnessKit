import { readFileSync } from "node:fs";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { en } from "../../../data/copy/en";
import { zh } from "../../../data/copy/zh";
import { CLI_STEP_MS } from "./everywhere-terminal";
import { MODES, STEP_MS, TOUR_RATIO, tour } from "./everywhere-tour";

// The picture's root as EverywherePanel renders it once the inline script ran: the radios, the first one checked.
function picture(): HTMLElement {
  const root = document.createElement("div");
  root.dataset.tour = "waiting";
  root.innerHTML = MODES.map((mode) => `<label><input type="radio" name="mode" value="${mode}"><span>${mode}</span></label>`).join("");
  root.querySelector<HTMLInputElement>(`input[value="${MODES[0]}"]`)!.checked = true;
  document.body.append(root);
  return root;
}
const shown = (root: HTMLElement) => root.querySelector<HTMLInputElement>("input:checked")?.value;

// Replaces IntersectionObserver; `see(ratio)` reports that share of the picture on-screen.
function stubIntersection() {
  let report: (entries: Pick<IntersectionObserverEntry, "intersectionRatio">[]) => void = () => {};
  vi.stubGlobal("IntersectionObserver", class {
    constructor(callback: typeof report) { report = callback; }
    observe() {}
    disconnect() {}
  });
  return (ratio: number) => report([{ intersectionRatio: ratio }]);
}

// A mouse moving onto or off the picture.
const mouse = (root: HTMLElement, type: "pointerenter" | "pointerleave") => root.dispatchEvent(Object.assign(new Event(type), { pointerType: "mouse" }));

function setTabHidden(hidden: boolean) {
  vi.spyOn(document, "hidden", "get").mockReturnValue(hidden);
  document.dispatchEvent(new Event("visibilitychange"));
}

describe("the everywhere tour", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("shows each way in turn once the picture is on-screen, and after the last, the first again", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);

    expect(shown(root)).toBe("mac");
    expect(root.dataset.tour).toBe("playing");
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("win");
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("web");
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("cli");
    vi.advanceTimersByTime(CLI_STEP_MS);
    expect(shown(root)).toBe("mac");
    expect(root.dataset.tour).toBe("playing");
  });

  it("stays on the terminal longer, while its session types itself out", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    vi.advanceTimersByTime(STEP_MS * 3);
    expect(shown(root)).toBe("cli");
    vi.advanceTimersByTime(CLI_STEP_MS - 1);
    expect(shown(root)).toBe("cli");
    vi.advanceTimersByTime(1);
    expect(shown(root)).toBe("mac");
  });

  it("pauses while a mouse rests on the picture, and goes on with the time the step had left", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    vi.advanceTimersByTime(STEP_MS / 2);

    mouse(root, "pointerenter");
    expect(root.dataset.tour).toBe("paused");
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("mac");

    mouse(root, "pointerleave");
    expect(root.dataset.tour).toBe("playing");
    vi.advanceTimersByTime(STEP_MS / 2 - 1);
    expect(shown(root)).toBe("mac");
    vi.advanceTimersByTime(1);
    expect(shown(root)).toBe("win");
  });

  it("ignores a finger's touch passing over: only a mouse pauses it", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    root.dispatchEvent(Object.assign(new Event("pointerenter"), { pointerType: "touch" }));
    expect(root.dataset.tour).toBe("playing");
  });

  it("pauses while the picture is off-screen", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    vi.advanceTimersByTime(STEP_MS);
    see(0);
    expect(root.dataset.tour).toBe("paused");
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("win");
    see(1);
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("web");
  });

  it("waits until enough of the picture is on-screen", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(TOUR_RATIO / 2);
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("mac");
    expect(root.dataset.tour).toBe("waiting");

    see(TOUR_RATIO);
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("win");
  });

  it.each(["click", "keydown", "focusin", "change"])("stops where it is on the visitor's first %s", (type) => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    vi.advanceTimersByTime(STEP_MS);

    root.querySelector("input")!.dispatchEvent(new Event(type, { bubbles: true }));
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("win");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("never plays once the visitor has chosen a way", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    root.querySelector("input")!.dispatchEvent(new Event("change", { bubbles: true }));
    see(1);
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("mac");
  });

  it("pauses while the tab is hidden, where it was", () => {
    const see = stubIntersection();
    const root = picture();
    tour(root);
    see(1);
    setTabHidden(true);
    expect(root.dataset.tour).toBe("paused");
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("mac");
    setTabHidden(false);
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("win");
  });

  it("waits for a hidden tab to come back before it starts", () => {
    const see = stubIntersection();
    const root = picture();
    setTabHidden(true);
    tour(root);
    see(1);
    vi.advanceTimersByTime((STEP_MS + CLI_STEP_MS) * MODES.length);
    expect(shown(root)).toBe("mac");

    setTabHidden(false);
    vi.advanceTimersByTime(STEP_MS);
    expect(shown(root)).toBe("win");
  });
});

// The picture's sidebar is the App's: its items in sidebar.tsx's order (main items, then Settings), labelled from the
// App's navigation locale.
// Through a variable: Vite would turn `new URL(…, import.meta.url)` into an asset import of the whole App.
const here = import.meta.url;
const appSource = (path: string) => readFileSync(new URL(`../../../../../${path}`, here), "utf8");

describe("the picture's sidebar", () => {
  const sidebar = appSource("src/components/layout/sidebar.tsx");
  const keys = [...sidebar.matchAll(/labelKey: "(\w+)"/g)].map((m) => m[1]);

  it.each([["en", en], ["zh", zh]] as const)("%s: has the App's items and labels", (lang, copy) => {
    const labels: Record<string, string> = JSON.parse(appSource(`src/lib/i18n/locales/${lang}/navigation.json`));
    expect(keys.length).toBeGreaterThan(3);
    expect(copy.mockLabels.everywhere.nav).toEqual(keys.map((key) => labels[key]));
  });
});

// The Overview page's words are the App's (its overview locale), plural where the App counts more than one.
describe("the picture's Overview page", () => {
  it.each([["en", en], ["zh", zh]] as const)("%s: has the App's words", (lang, copy) => {
    const o = JSON.parse(appSource(`src/lib/i18n/locales/${lang}/overview.json`));
    const t = copy.mockLabels.everywhere.overview;
    expect([t.agents, t.extensions]).toEqual([o.terminal.agentLabel_other, o.terminal.extensionLabel_other]);
    expect(t.kinds).toEqual({ skill: o.stats.skill_other, mcp: o.stats.mcpServer_other, plugin: o.stats.plugin_other, hook: o.stats.hook_other, cli: o.stats.cli_other });
    expect([t.tip, t.activity, t.installed]).toEqual([o.tipOfDay.title, o.activity.agentActivity, o.activity.recentlyInstalled]);
  });
});
