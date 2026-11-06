// @vitest-environment node
import { describe, expect, it } from "vitest";
import { AGENTS, EXTENSION_KINDS } from "../../../data/agents";
import { MOCK_AUDIT, MOCK_EXTENSIONS, MOCK_GATHER, MOCK_GATHER_AGENTS } from "./mock-data";

// The App's Trust Score: 100 minus each rule's first finding (crates/hk-core/src/auditor/mod.rs, Severity::deduction
// in crates/hk-core/src/models.rs): Critical 25, High 15, Medium 8, Low 3.
const DEDUCTIONS = [25, 15, 8, 3];
// Whether some few findings, of any severities, take 100 down to `score`.
function reachable(score: number, findings = 4): boolean {
  if (score === 100) return true;
  return findings > 0 && DEDUCTIONS.some((d) => score + d <= 100 && reachable(score + d, findings - 1));
}

describe("Problem block sample (Gather.astro)", () => {
  it("piles only agents the site knows, so no pile falls back to a placeholder mascot", () => {
    for (const agent of MOCK_GATHER_AGENTS) expect(AGENTS.map((a) => a.id)).toContain(agent);
  });

  it.each(MOCK_GATHER)("puts $name only on piled agents that support its kind", (item) => {
    for (const agent of item.agents) {
      expect(MOCK_GATHER_AGENTS).toContain(agent);
      expect(AGENTS.find((a) => a.id === agent)?.supports, agent).toContain(item.kind);
    }
  });

  // The picture's story: every agent has a pile, and the same item sits in several piles.
  it("gives every agent a pile and repeats an item across piles", () => {
    for (const agent of MOCK_GATHER_AGENTS) expect(MOCK_GATHER.some((item) => item.agents.includes(agent)), agent).toBe(true);
    expect(MOCK_GATHER.some((item) => item.agents.length > 1)).toBe(true);
  });

  // The list shows what HarnessKit gathers: every kind of extension the App knows, once each, in its order.
  it("has one extension of each kind, in the App's order", () => {
    expect(MOCK_GATHER.map((item) => item.kind)).toEqual([...EXTENSION_KINDS]);
  });

  // The body says it in words: the same extension ends up in several places.
  it("has PDF tools, a skill, in three piles", () => {
    const pdf = MOCK_GATHER.find((item) => item.name === "PDF tools");
    expect(pdf?.kind).toBe("skill");
    expect(pdf?.agents).toHaveLength(3);
  });

  it.each(MOCK_GATHER)("gives $name a score the App's formula can reach", (item) => {
    expect(reachable(item.score), String(item.score)).toBe(true);
  });

  // The windows below show some of the same extensions: one name, one kind, one set of agents (before the Extensions
  // window adds one), one score.
  it("agrees with the Extensions and Security Audit windows on every extension they share", () => {
    const shared = MOCK_GATHER.filter((item) => MOCK_EXTENSIONS.some((e) => e.name === item.name));
    expect(shared.length).toBeGreaterThan(0);
    for (const item of shared) {
      const ext = MOCK_EXTENSIONS.find((e) => e.name === item.name)!;
      expect({ kind: item.kind, agents: [...item.agents].sort(), score: item.score }, item.name)
        .toEqual({ kind: ext.kind, agents: [...ext.agents].sort(), score: ext.score });
    }
    for (const item of MOCK_GATHER.filter((g) => MOCK_AUDIT.some((a) => a.name === g.name))) {
      expect(item.score, item.name).toBe(MOCK_AUDIT.find((a) => a.name === item.name)!.score);
    }
  });
});
