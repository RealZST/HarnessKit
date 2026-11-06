// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { AGENT_COUNT, AGENTS, EXTENSION_KINDS } from "./agents";

function appAgentOrder(): string[] {
  const source = readFileSync(new URL("../../../src/lib/types.ts", import.meta.url), "utf8");
  const block = source.match(/export const AGENT_ORDER = \[([\s\S]*?)\]/);
  if (!block) throw new Error("AGENT_ORDER not found in src/lib/types.ts");
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

describe("agents data", () => {
  it("lists exactly the App's agents in AGENT_ORDER order", () => {
    expect(AGENTS.map((a) => a.id)).toEqual(appAgentOrder());
  });
  it("every agent supports skills", () => {
    for (const a of AGENTS) expect(a.supports).toContain("skill");
  });
  it("has the agent count the landing's drawings are set up for", () => {
    expect(AGENT_COUNT, "the Overview window needs the new agent's sample count (MOCK_EXT_COUNTS) and a SCATTER start (OverviewLive.tsx)").toBe(17);
  });
  it("supports only uses known kinds", () => {
    for (const a of AGENTS) for (const k of a.supports) expect(EXTENSION_KINDS).toContain(k);
  });
});
