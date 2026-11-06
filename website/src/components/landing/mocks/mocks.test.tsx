import { readFileSync } from "node:fs";
import { act, fireEvent, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AGENTS, type ExtensionKind } from "../../../data/agents";
import { en, type CopyShape } from "../../../data/copy/en";
import { zh } from "../../../data/copy/zh";
import AuditMock, { PERIOD as AUDIT_PERIOD } from "./AuditMock";
import ExtensionsMock, { PERIOD as EXT_PERIOD } from "./ExtensionsMock";
import { MOCK_AUDIT, MOCK_EXTENSIONS, type MockAuditItem, type MockRule, type MockSeverity } from "./mock-data";

// The kinds each rule checks and the severities it reports for them, by the rule's key in the App's audit locale
// (src/lib/i18n/locales/en/audit.json).
// Permission Combination Risk: any kind, High, needs network plus shell or env (crates/hk-core/src/auditor/rules/permissions.rs).
// Dangerous Commands: High for hooks, Medium for skills and plugins (rules/content.rs).
const RULES: Record<MockRule, Partial<Record<ExtensionKind, readonly MockSeverity[]>>> = {
  permissionComboRisk: { skill: ["High"], mcp: ["High"], plugin: ["High"], hook: ["High"], cli: ["High"] },
  dangerousCommands: { skill: ["Medium"], plugin: ["Medium"], hook: ["High"] },
};

// The App's Trust Score: 100 minus the first finding of each rule (crates/hk-core/src/auditor/mod.rs,
// Severity::deduction in crates/hk-core/src/models.rs).
const DEDUCTION = { High: 15, Medium: 8 } as const;
function appScore(findings: MockAuditItem["findings"]): number {
  const firstPerRule = new Map(findings.map((f) => [f.rule, f.severity]));
  return 100 - [...firstPerRule.values()].reduce((sum, severity) => sum + DEDUCTION[severity], 0);
}

function preferReducedMotion() {
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
}

const advance = (ms: number) => act(() => { vi.advanceTimersByTime(ms); });
// A file of the App's, from the repo root. Through a variable: Vite would turn `new URL(…, import.meta.url)` into an
// asset import of the whole App.
const here = import.meta.url;
const appSource = (path: string) => readFileSync(new URL(`../../../../../${path}`, here), "utf8");

// Replaces IntersectionObserver; `see(ratio)` reports that share of the mock window on-screen (0 = off-screen).
function stubIntersection() {
  let report: (entries: Pick<IntersectionObserverEntry, "isIntersecting" | "intersectionRatio">[]) => void = () => {};
  vi.stubGlobal("IntersectionObserver", class {
    constructor(callback: typeof report) { report = callback; }
    observe() {}
    disconnect() {}
  });
  return (ratio: number) => act(() => report([{ isIntersecting: ratio > 0, intersectionRatio: ratio }]));
}

const addedTo = MOCK_EXTENSIONS.find((e) => e.addTo)!;
const addedName = AGENTS.find((a) => a.id === addedTo.addTo)!.name;
const PCR = en.mockLabels.audit.rules.permissionComboRisk;
type Strings = CopyShape["mockLabels"];
const extensions = (t: Strings["extensions"] = en.mockLabels.extensions) => render(<ExtensionsMock t={t} />);
const audit = (t: Strings["audit"] = en.mockLabels.audit) => render(<AuditMock t={t} />);
const liveList = (container: HTMLElement) => within(container.querySelector<HTMLElement>(".mock-audit-live")!);

describe("mock windows", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("uses only agent ids the site knows, so no row falls back to a placeholder mascot", () => {
    const known = new Set(AGENTS.map((a) => a.id));
    const ids = MOCK_EXTENSIONS.flatMap((e) => [...e.agents, ...(e.addTo ? [e.addTo] : [])]);
    expect(ids.filter((id) => !known.has(id))).toEqual([]);
  });

  it("puts each sample extension only on agents that support its kind", () => {
    const supports = new Map(AGENTS.map((a) => [a.id, a.supports]));
    const misplaced = MOCK_EXTENSIONS.flatMap((e) => e.agents.filter((id) => !supports.get(id)?.includes(e.kind)).map((id) => `${e.name} on ${id}`));
    expect(misplaced).toEqual([]);
  });

  // The App's cross-agent install takes skills, MCP servers, hooks and CLIs (src/lib/agent-capabilities.ts
  // INSTALLABLE_KINDS), and only onto an agent that supports the kind.
  it("adds exactly one extension to one agent that can take it and doesn't have it yet", () => {
    expect(MOCK_EXTENSIONS.filter((e) => e.addTo)).toHaveLength(1);
    expect(["skill", "mcp", "hook", "cli"]).toContain(addedTo.kind);
    expect(AGENTS.find((a) => a.id === addedTo.addTo)?.supports).toContain(addedTo.kind);
    expect(addedTo.agents).not.toContain(addedTo.addTo);
  });

  it("reports only findings the App's rules can produce for each extension's kind", () => {
    const byName = new Map(MOCK_EXTENSIONS.map((e) => [e.name, e]));
    const impossible = MOCK_AUDIT.flatMap((item) => {
      const ext = byName.get(item.name)!;
      return item.findings
        .filter((f) => !RULES[f.rule][ext.kind]?.includes(f.severity))
        .map((f) => `${item.name} (${ext.kind}): ${f.rule} ${f.severity}`);
    });
    expect(impossible).toEqual([]);
  });

  // The rule fires whenever network and shell are both among the permissions, and only then (no sample has env).
  it("reports Permission Combination Risk exactly for the extensions that have network and shell", () => {
    const risky = MOCK_EXTENSIONS.filter((e) => e.perms.includes("network") && e.perms.includes("shell")).map((e) => e.name);
    const flagged = MOCK_AUDIT.filter((item) => item.findings.some((f) => f.rule === "permissionComboRisk")).map((item) => item.name);
    expect(flagged.sort()).toEqual(risky.sort());
  });

  // The note by the open row says the skill reads files, goes online and runs commands.
  it("writes the audit note about an extension that has filesystem, network and shell", () => {
    const opened = MOCK_EXTENSIONS.find((e) => e.name === MOCK_AUDIT[0].name)!;
    expect(opened.perms).toEqual(expect.arrayContaining(["filesystem", "network", "shell"]));
  });

  it("scores the audit rows as the App would, lists them lowest first, and covers every extension", () => {
    expect(MOCK_AUDIT.map((item) => item.score)).toEqual(MOCK_AUDIT.map((item) => appScore(item.findings)));
    const order = [...MOCK_AUDIT].sort((a, b) => a.score - b.score || a.name.localeCompare(b.name));
    expect(MOCK_AUDIT.map((item) => item.name)).toEqual(order.map((item) => item.name));
    const extensionScores = Object.fromEntries(MOCK_EXTENSIONS.map((e) => [e.name, e.score]));
    expect(Object.fromEntries(MOCK_AUDIT.map((item) => [item.name, item.score]))).toEqual(extensionScores);
  });

  it("names every extension row", () => {
    const { container } = extensions();
    const names = [...container.querySelectorAll(".mock-ext-name")].map((el) => el.textContent);
    expect(names).toEqual(MOCK_EXTENSIONS.map((e) => e.name));
  });

  it.each([["en", en], ["zh", zh]] as const)("labels the Extensions window as the App's %s UI does: kind filter, headers, permissions, detail panel, toast", (lang, copy) => {
    preferReducedMotion();
    const t = copy.mockLabels.extensions;
    const { container } = extensions(t);
    const kinds = JSON.parse(appSource(`src/lib/i18n/locales/${lang}/common.json`)).kinds;
    const table = JSON.parse(appSource(`src/lib/i18n/locales/${lang}/extensions.json`));

    const chips = [...container.querySelectorAll(".mock-chip")].map((el) => el.textContent);
    expect(chips).toEqual([kinds.all, kinds.skill, kinds.mcp, kinds.plugin, kinds.hook, kinds.cli]);
    expect([...container.querySelectorAll(".mock-chip.on")].map((el) => el.textContent)).toEqual([kinds.all]);
    // Score stands in for the App's "Audit" header, as the App's onboarding has it.
    const { name, kind, agent, permissions, status } = table.table.headers;
    expect([...container.querySelectorAll("th")].map((el) => el.textContent)).toEqual([name, kind, agent, permissions, t.score, status]);
    expect(t.permNames).toEqual({ filesystem: table.permissions.filesystem, network: table.permissions.network, shell: table.permissions.shell, database: table.permissions.database });
    // The detail panel's, and the toast: the App's first sentence, its {{agent}} as {agent}.
    const scope = JSON.parse(appSource(`src/lib/i18n/locales/${lang}/common.json`)).scope;
    expect([t.on, t.agentsHeading, t.installTo, t.scope, t.global]).toEqual([table.detail.enabled, table.detail.agents, table.detail.installToAgent, scope.installToScopeColon, scope.global]);
    expect(table.detail.installToSuccess.replace("{{agent}}", "{agent}").startsWith(t.installed)).toBe(true);
  });

  it("shows each row's kind, permissions and score, in columns a narrow window can drop whole", () => {
    preferReducedMotion();
    const { container } = extensions();
    const rows = [...container.querySelectorAll("tbody tr")];
    expect(rows.map((row) => row.querySelector(".mock-col-kind")?.textContent)).toEqual(MOCK_EXTENSIONS.map((e) => (e.kind === "mcp" ? "MCP" : e.kind)));
    expect(rows.map((row) => [...row.querySelectorAll(".mock-perms [title]")].map((el) => el.getAttribute("title")).sort()))
      .toEqual(MOCK_EXTENSIONS.map((e) => e.perms.map((p) => en.mockLabels.extensions.permNames[p]).sort()));
    expect(rows.map((row) => row.querySelector(".mock-col-score")?.textContent)).toEqual(MOCK_EXTENSIONS.map((e) => String(e.score)));
    // mocks.css hides these by the window's width: each must be on the header and every row, or the table would shear.
    for (const col of ["mock-col-kind", "mock-col-perms", "mock-col-score"]) {
      expect(container.querySelectorAll(`th.${col}`)).toHaveLength(1);
      expect(container.querySelectorAll(`td.${col}`)).toHaveLength(MOCK_EXTENSIONS.length);
    }
  });

  // The App's install to another agent: select the row, its detail panel opens, click the agent's tile under Install to
  // Agent; it spins, then shows the check and flashes, with the toast, and the agent joins the panel's Agents and the row.
  it("opens PDF tools' detail panel once on-screen and installs it to Codex from its tile, with the App's toast", () => {
    const see = stubIntersection();
    const { container } = extensions();
    const $ = (selector: string) => container.querySelector(selector);
    const tile = () => [...container.querySelectorAll(".mock-tile")].find((el) => el.textContent === addedName)!;
    const chips = () => [...container.querySelectorAll(".mock-agent-chips > :not([aria-hidden]) > *")].map((el) => el.textContent);

    advance(5000);
    expect($(".mock-detail")).toHaveClass("is-closed");
    expect($("tr.is-selected")).toBeNull();
    see(1);
    advance(1500);
    expect($("tr.is-selected")?.textContent).toContain(addedTo.name);
    expect($(".mock-detail")).toHaveClass("is-open");
    expect(tile()).toHaveClass("is-can");
    expect(chips()).not.toContain(addedName);
    advance(1400);
    expect(tile()).toHaveClass("is-busy");
    expect($(".mock-toast")).toBeNull();
    advance(700);
    expect(tile()).toHaveClass("is-flash");
    expect($(".mock-toast")?.textContent).toBe(`Installed to ${addedName}`);
    expect(chips()).toContain(addedName);
    expect($("tr.is-selected .mock-new")).toHaveAttribute("title", addedName);
    // The panel slides out and leaves the row with its new agent.
    advance(3100);
    expect($(".mock-detail")).toHaveClass("is-closing");
    expect($("tr.is-selected")).toBeNull();
    expect($(".mock-new")).not.toBeNull();
  });

  // The App shows a tile for each agent it found, in its own order; one already holding the extension is checked.
  it("draws a tile per agent in the list, in the App's order, checked where PDF tools already is", () => {
    preferReducedMotion();
    const { container } = extensions();
    const tiles = [...container.querySelectorAll(".mock-tile")];
    const order = AGENTS.map((a) => a.id);
    const ids = [...new Set(MOCK_EXTENSIONS.flatMap((e) => [...e.agents, ...(e.addTo ? [e.addTo] : [])]))].sort((a, b) => order.indexOf(a) - order.indexOf(b));
    expect(tiles.map((el) => el.textContent)).toEqual(ids.map((id) => AGENTS.find((a) => a.id === id)!.name));
    expect(ids.every((id) => AGENTS.find((a) => a.id === id)!.supports.includes(addedTo.kind))).toBe(true);
    const checked = ids.filter((id) => addedTo.agents.includes(id) || id === addedTo.addTo);
    expect(tiles.filter((el) => el.classList.contains("is-has")).map((el) => el.textContent)).toEqual(checked.map((id) => AGENTS.find((a) => a.id === id)!.name));
  });

  it("runs a first audit: Ready, then running, then the scores, then the lowest opens once with its note", () => {
    const see = stubIntersection();
    const { container } = audit();
    const live = liveList(container);
    const a = en.mockLabels.audit;

    expect(live.queryByText(a.readyTitle)).not.toBeNull();
    expect(live.queryByText(MOCK_AUDIT[0].name)).toBeNull();
    see(1);
    advance(1100);
    expect(live.queryByText(a.running)).not.toBeNull();
    expect(container.querySelector(".mock-run")).toHaveClass("is-running");
    advance(1000);
    expect(live.getAllByText(/^(PDF tools|GitHub|Code review|Notify when done)$/).map((el) => el.textContent)).toEqual(MOCK_AUDIT.map((item) => item.name));
    expect(live.queryByText(PCR)).toBeNull();
    // Opens once and stays open: no close and reopen.
    const opened: boolean[] = [];
    for (let ms = 0; ms < 3000; ms += 100) {
      advance(100);
      opened.push(live.queryByText(PCR) !== null);
    }
    expect(opened.indexOf(true)).toBeGreaterThan(0);
    expect(opened.slice(opened.indexOf(true)).every(Boolean)).toBe(true);
    expect(live.queryByText(a.note)).not.toBeNull();
    expect(container.querySelector(".mockcursor")).not.toBeNull();
  });

  // As the App's onboarding windows: the run, a rest on where it ends, the changes fading out, then the run again.
  it.each([
    // The window, where each run ends, where it starts; the Audit window's hidden copy is always finished.
    ["Extensions", () => <ExtensionsMock t={en.mockLabels.extensions} />, EXT_PERIOD, ".mockwin-body", ".mock-new", ".mock-detail.is-closed"],
    ["Audit", () => <AuditMock t={en.mockLabels.audit} />, AUDIT_PERIOD, ".mock-audit-live", ".mock-findings", ".mock-audit-state"],
  ])("loops the %s window: plays it, rests on its finished state, fades it out and starts over", (_, mock, period, scope, end, start) => {
    const see = stubIntersection();
    const { container } = render(mock());
    const win = container.querySelector(".mockwin")!;
    const live = () => container.querySelector(scope)!;
    see(1);

    advance(5000);
    expect(container.querySelector(".mockcursor")).toHaveStyle({ opacity: "0" });
    expect(live().querySelector(end)).not.toBeNull();
    expect(win).not.toHaveClass("is-leaving");
    // At least two seconds at rest before the fade.
    advance(period - 400 - 5000 - 1);
    expect(win).not.toHaveClass("is-leaving");
    advance(1);
    expect(win).toHaveClass("is-leaving");
    advance(401);
    expect(win).not.toHaveClass("is-leaving");
    expect(live().querySelector(end)).toBeNull();
    expect(live().querySelector(start)).not.toBeNull();
    advance(5000);
    expect(live().querySelector(end)).not.toBeNull();
  });

  // WCAG 2.2.2: the loop lasts more than five seconds, so a reader can stop it by pointing at the window or tabbing to it.
  it.each([
    ["Extensions", () => <ExtensionsMock t={en.mockLabels.extensions} />],
    ["Audit", () => <AuditMock t={en.mockLabels.audit} />],
  ])("holds the %s window still while the pointer is over it or it has focus", (_, mock) => {
    const see = stubIntersection();
    const { container } = render(mock());
    const win = container.querySelector<HTMLElement>(".mockwin")!;
    const body = () => container.querySelector(".mockwin-body")!.innerHTML;
    expect(win).toHaveAttribute("tabindex", "0");
    see(1);
    advance(1200);

    fireEvent.pointerEnter(win);
    const held = body();
    advance(60_000);
    expect(body()).toBe(held);
    fireEvent.pointerLeave(win);

    act(() => win.focus());
    const focused = body();
    advance(60_000);
    expect(body()).toBe(focused);
    act(() => win.blur());
    advance(10_000);
    expect(body()).not.toBe(focused);
  });

  it("rewinds the Extensions window once it is off-screen, so the next visit plays the install again", () => {
    const see = stubIntersection();
    const { container } = extensions();
    see(1);
    advance(5000);
    expect(container.querySelector(".mock-new")).not.toBeNull();

    see(0);
    expect(container.querySelector(".mock-detail")).toHaveClass("is-closed");
    expect(container.querySelector(".mock-new")).toBeNull();
    expect(container.querySelector(".mock-toast")).toBeNull();
    see(1);
    advance(3600);
    expect(container.querySelector(".mock-new")).not.toBeNull();
  });

  it("rewinds the Audit window to Ready once it is off-screen, so the next visit runs the audit again", () => {
    const see = stubIntersection();
    const { container } = audit();
    const live = liveList(container);
    see(1);
    advance(1100);
    expect(live.queryByText(en.mockLabels.audit.running)).not.toBeNull();

    see(0);
    expect(vi.getTimerCount()).toBe(0);
    expect(live.queryByText(en.mockLabels.audit.readyTitle)).not.toBeNull();
    see(1);
    advance(5000);
    expect(live.queryByText(PCR)).not.toBeNull();
  });

  it("keeps a hidden copy of the list with the first row open, so the window never changes height", () => {
    const see = stubIntersection();
    const { container } = audit();
    const reserve = container.querySelector<HTMLElement>(".mock-audit-reserve")!;

    expect(reserve).toHaveAttribute("aria-hidden", "true");
    expect(within(reserve).queryByText(PCR)).not.toBeNull();
    expect(within(reserve).queryByText(en.mockLabels.audit.note)).not.toBeNull();
    expect(reserve.children).toHaveLength(MOCK_AUDIT.length);
    see(1);
    advance(1100);
    expect(liveList(container).queryByText(PCR)).toBeNull();
    expect(within(reserve).queryByText(PCR)).not.toBeNull();
  });

  it("shows where each run ends, with no cursor, under reduced motion", () => {
    preferReducedMotion();
    const ext = extensions();
    expect(ext.container.querySelector(".mock-detail")).toHaveClass("is-open");
    expect(ext.container.querySelector(".mock-new")).not.toBeNull();
    expect(ext.container.querySelector(".mock-toast")).not.toBeNull();
    expect(ext.container.querySelector(".mockcursor")).toBeNull();

    const { container } = audit();
    expect(liveList(container).queryByText(PCR)).not.toBeNull();
    expect(liveList(container).queryByText(en.mockLabels.audit.rules.dangerousCommands)).not.toBeNull();
    expect(container.querySelector(".mockcursor")).toBeNull();
  });

  // The App lists an extension's rules in AUDIT_RULES order, its rule ids camel-cased into locale keys (ruleI18nKey).
  it("opens the lowest row as the App does: each rule in the App's order, with its severity and where it was found", () => {
    preferReducedMotion();
    const order = [...appSource("src/pages/audit-utils.ts").matchAll(/^ {4}id: "([\w-]+)"/gm)].map((m) => m[1].replace(/-(\w)/g, (_, c: string) => c.toUpperCase()));
    for (const item of MOCK_AUDIT) {
      const rules = item.findings.map((f) => order.indexOf(f.rule));
      expect(rules.every((i) => i >= 0)).toBe(true);
      expect(rules).toEqual([...rules].sort((a, b) => a - b));
    }
    const { container } = audit();
    const opened = [...liveList(container).getAllByText(MOCK_AUDIT[0].name)[0].closest(".mock-audit-item")!.querySelectorAll(".mock-rule")];
    expect(opened.map((rule) => rule.querySelector(".mock-rule-head > span")?.textContent)).toEqual(MOCK_AUDIT[0].findings.map((f) => en.mockLabels.audit.rules[f.rule]));
    expect(opened.map((rule) => rule.querySelector(".mock-loc")?.textContent)).toEqual(MOCK_AUDIT[0].findings.map((f) => f.location));
  });

  // A skill's finding points at a line of its SKILL.md; an MCP server's at the server, by its name (crates/hk-core
  // service.rs: no file to point into).
  it("gives each finding a location of the kind the App reports for its extension", () => {
    const kinds = new Map(MOCK_EXTENSIONS.map((e) => [e.name, e.kind]));
    const wrong = MOCK_AUDIT.flatMap((item) => item.findings
      .filter((f) => !(kinds.get(item.name) === "skill" ? /^SKILL\.md:\d+$/ : /^[a-z0-9-]+$/).test(f.location))
      .map((f) => `${item.name}: ${f.location}`));
    expect(wrong).toEqual([]);
  });

  // The pictures show the App's zh UI on /zh/, not its English one.
  it("shows the App's Chinese labels when given the zh strings", () => {
    preferReducedMotion();
    const ext = extensions(zh.mockLabels.extensions);
    expect(ext.container.querySelector(".mockwin-title")?.textContent).toBe("扩展");
    expect(ext.container.querySelector(".mock-toast")?.textContent).toBe(`已安装到 ${addedName}`);
    expect(within(ext.container).getAllByText("已启用")).not.toHaveLength(0);

    const { container } = audit(zh.mockLabels.audit);
    expect(container.querySelector(".mockwin-title")?.textContent).toBe("安全审计");
    expect(liveList(container).queryByText("权限组合风险")).not.toBeNull();
    expect(container.querySelector(".mock-audit-head .mock-meta")?.textContent).toBe(`已扫描 ${MOCK_AUDIT.length} 个扩展 · 上次运行 刚刚`);
    expect(container.textContent).not.toMatch(/Security Audit|Run Audit|enabled|finding/);
  });
});
