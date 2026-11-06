// @vitest-environment node
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { en } from "./en";
import { zh } from "./zh";

// The docs FAQ opens with the landing questions (a .md page can't import copy): each one's first paragraph is the
// landing's plain answer, and the docs-only detail follows it. So a landing edit must land there too, in each
// language: en.ts with docs/faq.md, zh.ts with zh/docs/faq.md.
const FAQS = [
  { lang: "en", copy: en, page: "../../content/docs/docs/faq.md" },
  { lang: "zh", copy: zh, page: "../../content/docs/zh/docs/faq.md" },
];
// The landing sets ’; the docs write ' and smartypants curls it, so both render the same.
const straight = (text: string) => text.replaceAll("’", "'");

describe.each(FAQS)("docs FAQ ($lang)", ({ copy, page }) => {
  const docsFaq = readFileSync(new URL(page, import.meta.url), "utf8");

  it("opens with the landing questions, in the landing order", () => {
    const headings = [...docsFaq.matchAll(/^## (.+)$/gm)].map((m) => m[1]);
    expect(headings.slice(0, copy.faq.items.length)).toEqual(copy.faq.items.map((item) => straight(item.q)));
  });

  it.each(copy.faq.items.map((item) => [item.q, item]))("opens %s with the landing answer", (_q, item) => {
    expect(docsFaq).toContain(`## ${straight(item.q)}\n\n${straight(item.a)}\n`);
  });
});
