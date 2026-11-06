// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { AGENTS } from "./agents";
import {
  CARD_MASCOT_SIZE, CARD_PADDING, CARD_WIDTH, CLICK_DURATIONS, CLIPPED_CARDS, DEFAULT_CLICK_MS, MASCOT_CLIPS, cardName,
} from "./agent-card";

const appSource = (path: string) => readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");
const card = appSource("src/components/shared/agent-card.tsx");
// Tailwind's spacing scale: one step is 4px (px-3 = 12px, -mt-2.5 = 10px).
const px = (step: string) => Number(step) * 4;

function match(source: string, pattern: RegExp, what: string): RegExpMatchArray {
  const m = source.match(pattern);
  if (!m) throw new Error(`${what} not found in the App source; update agent-card.test.ts to its new shape`);
  return m;
}

describe("the App's agent card", () => {
  it("has the same size, padding and mascot size", () => {
    expect(Number(match(card, /\bw-\[(\d+)px\]/, "card width")[1])).toBe(CARD_WIDTH);
    expect(Number(match(card, /size=\{(\d+)\}/, "mascot size")[1])).toBe(CARD_MASCOT_SIZE);
    const [, x, y] = match(card, /\bpx-([\d.]+) py-([\d.]+)\b/, "card padding");
    expect({ x: px(x), y: px(y) }).toEqual(CARD_PADDING);
  });

  it("has the same click durations and default", () => {
    const block = match(card, /const CLICK_DURATIONS[^=]*=\s*\{([\s\S]*?)\n\};/, "CLICK_DURATIONS")[1];
    const durations = Object.fromEntries([...block.matchAll(/^\s*"?([\w-]+)"?:\s*(\d+),/gm)].map((m) => [m[1], Number(m[2])]));
    expect(CLICK_DURATIONS).toEqual(durations);
    expect(Number(match(card, /CLICK_DURATIONS\[agent\.name\] \?\? (\d+)/, "default click duration")[1])).toBe(DEFAULT_CLICK_MS);
  });

  it("clips the same cards", () => {
    const condition = match(card, /((?:agent\.name === "[\w-]+"(?: \|\| )?)+) \? "overflow-hidden"/, "the card's overflow-hidden condition")[1];
    expect(new Set([...condition.matchAll(/"([\w-]+)"/g)].map((m) => m[1]))).toEqual(CLIPPED_CARDS);
  });

  it("clips the same mascots, out to the card's sides", () => {
    const wraps = [...card.matchAll(/agent\.name === "([\w-]+)"\s*\?\s*"-mx-([\d.]+) -mt-([\d.]+) overflow-hidden px-([\d.]+) pt-([\d.]+)"/g)];
    expect(Object.fromEntries(wraps.map(([, id, , mt]) => [id, { top: px(mt) }]))).toEqual(MASCOT_CLIPS);
    for (const [, id, mx, mt, padX, pt] of wraps) {
      expect(px(mx), `${id}: the clip reaches the card's sides`).toBe(CARD_PADDING.x);
      expect([px(padX), px(pt)], `${id}: the padding undoes the negative margin`).toEqual([px(mx), px(mt)]);
    }
  });

  it("labels each card with the App's display name", () => {
    const block = match(appSource("src/lib/types.ts"), /const AGENT_DISPLAY_NAMES[^=]*=\s*\{([\s\S]*?)\n\};/, "AGENT_DISPLAY_NAMES")[1];
    const names = Object.fromEntries([...block.matchAll(/"?([\w-]+)"?:\s*"([^"]+)"/g)].map((m) => [m[1], m[2]]));
    expect(Object.fromEntries(AGENTS.map((a) => [a.id, cardName(a)]))).toEqual(names);
  });
});
