// @vitest-environment node
import { globSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { AGENT_COUNT, AUDIT_RULE_COUNT } from "../data/agents";

// Counts go stale the day an agent or a rule is added, so docs pages render them from constants
// (contributing/writing-docs.md). This test fails on a count typed into the prose. Fenced code blocks
// are skipped: sample command output keeps the numbers it printed.
// English pages under docs/, Chinese ones under zh/docs/ (astro.config.mjs locales).
const DOCS_DIR = fileURLToPath(new URL("./docs/", import.meta.url));
const pages = globSync("{docs,zh/docs}/**/*.{md,mdx}", { cwd: DOCS_DIR }).sort();

const COUNTS = [
  { constant: "AGENT_COUNT", value: AGENT_COUNT, noun: "agents", nounZh: "agent" },
  { constant: "AUDIT_RULE_COUNT", value: AUDIT_RULE_COUNT, noun: "rules", nounZh: "(?:规则|检查)" },
];

/**
 * "17 agents", "17 supported agents", "20 static analysis rules", or "all 17"; in Chinese "17 个 agent",
 * "17 个受支持的 agent", "20 条规则", "20 项安全检查", or "全部 17" / "所有 17".
 */
const literalCount = (value: number, noun: string, nounZh: string) =>
  new RegExp(String.raw`\b${value}(?: [\w-]+){0,3} ${noun}\b|\ball ${value}\b|${value}\s*[个条项种][^\n，。、]{0,6}?${nounZh}|(?:全部|所有)\s*${value}`, "i");

/** The page with every fenced code block blanked out. */
function prose(markdown: string): string {
  return markdown.replace(/^(```|~~~)[^\n]*\n[\s\S]*?^\1[ \t]*$/gm, "");
}

describe("Docs counts", () => {
  it("finds the docs pages", () => {
    expect(pages.length).toBeGreaterThan(0);
  });

  it.each(pages)("%s takes agent and rule counts from constants", (page) => {
    const text = prose(readFileSync(DOCS_DIR + page, "utf8"));
    for (const { constant, value, noun, nounZh } of COUNTS) {
      const typed = text.match(literalCount(value, noun, nounZh));
      expect(typed?.[0], `write {${constant}} in place of the typed count`).toBeUndefined();
    }
  });
});
