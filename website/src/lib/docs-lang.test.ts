import { describe, expect, it } from "vitest";
import { docsLang, otherLangPath } from "./docs-lang";

describe("a docs page's language", () => {
  it("is Chinese under /zh/ and English elsewhere", () => {
    expect(docsLang("/zh/docs/faq/")).toBe("zh");
    expect(docsLang("/docs/faq/")).toBe("en");
    expect(docsLang("/zhihu/")).toBe("en");
  });

  it("switches to the same page in the other language", () => {
    expect(otherLangPath("/docs/guides/kits/")).toBe("/zh/docs/guides/kits/");
    expect(otherLangPath("/zh/docs/guides/kits/")).toBe("/docs/guides/kits/");
  });

  it("goes from a not-found page to the other language's docs overview", () => {
    expect(otherLangPath("/404")).toBe("/zh/docs/");
    expect(otherLangPath("/404.html")).toBe("/zh/docs/");
    expect(otherLangPath("/zh/404/")).toBe("/docs/");
  });
});
