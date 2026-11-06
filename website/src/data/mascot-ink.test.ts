import { describe, expect, it } from "vitest";
import { AGENTS } from "./agents";
import { MASCOT_INK_OFFSET, centerInk } from "./mascot-ink";

describe("mascot ink offsets", () => {
  it("has a measured offset for every agent, and none for an agent that's gone", () => {
    expect(Object.keys(MASCOT_INK_OFFSET).sort()).toEqual(AGENTS.map((a) => a.id).sort());
  });

  it("moves a mascot drawn high down by its offset times its size", () => {
    expect(centerInk("claude", 18)).toBe("0 1.8px");
    expect(centerInk("codex", 18)).toBe("0 0.6px");
  });
});
