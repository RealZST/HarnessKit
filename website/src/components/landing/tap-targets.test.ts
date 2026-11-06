// @vitest-environment node
import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Every link the landing sets on its own (not inside a sentence: those come from Inline.astro) is a 44px touch
// target: .tap-target gives a small one the room (global.css), and data-hit="own" marks one that is big enough by its
// own layout (a tile, a release, a padded footer row). A new link with neither fails here. This reads the templates;
// scripts/check-targets.mjs measures the built page.
const DIR = new URL("./", import.meta.url);
const FILES = readdirSync(DIR).filter((name) => /\.(astro|tsx)$/.test(name) && !name.includes(".test."));

/** The opening `<a …>` tags in a template, read up to the `>` outside any `{…}` expression. */
function openingLinks(source: string): string[] {
  const tags: string[] = [];
  for (const m of source.matchAll(/<a[\s>]/g)) {
    let depth = 0;
    let i = m.index;
    for (; i < source.length; i++) {
      if (source[i] === "{") depth++;
      else if (source[i] === "}") depth--;
      else if (source[i] === ">" && depth === 0) break;
    }
    tags.push(source.slice(m.index, i + 1));
  }
  return tags;
}

/** True when the tag's class attribute (class, className, class:list) names tap-target as a whole class. */
const hasTapTarget = (tag: string) =>
  [...tag.matchAll(/\b(?:class|className|class:list)=(?:"([^"]*)"|\{([^}]*)\})/g)].some((m) => /(?<![\w-])tap-target(?![\w-])/.test(m[1] ?? m[2]));
const ownsHit = (tag: string) => /\sdata-hit="own"/.test(tag);

describe("landing links", () => {
  it("finds the links it checks", () => {
    expect(FILES).toContain("Nav.astro");
    expect(openingLinks(readFileSync(new URL("Nav.astro", DIR), "utf8")).length).toBeGreaterThan(3);
  });

  it("reads the class attribute only, and tap-target only as a whole class", () => {
    expect(hasTapTarget('<a class="script tap-target">')).toBe(true);
    expect(hasTapTarget('<a class:list={["x", "tap-target"]}>')).toBe(true);
    expect(hasTapTarget('<a class="tap-targets">')).toBe(false);
    expect(hasTapTarget('<a class="no-tap-target">')).toBe(false);
    expect(hasTapTarget('<a href="#tap-target">')).toBe(false);
    expect(ownsHit('<a data-hit="own" href="#">')).toBe(true);
    expect(ownsHit('<a data-hit="owner">')).toBe(false);
  });

  it.each(FILES)("%s: each link is a tap target or sized as one", (name) => {
    const bare = openingLinks(readFileSync(new URL(name, DIR), "utf8")).filter((tag) => !hasTapTarget(tag) && !ownsHit(tag));
    expect(bare).toEqual([]);
  });
});
