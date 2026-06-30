import { describe, expect, it } from "vitest";
import { localizeChangelog } from "../changelog";

const BILINGUAL = `<!-- lang:en -->
## What's new
English line
<!-- lang:zh -->
## 更新内容
中文行`;

describe("localizeChangelog", () => {
  it("returns the section matching the language", () => {
    const zh = localizeChangelog(BILINGUAL, "zh");
    expect(zh).toContain("中文行");
    expect(zh).not.toContain("English line");
    expect(localizeChangelog(BILINGUAL, "en")).toContain("English line");
  });

  it("normalizes regional codes like zh-CN to the section language", () => {
    expect(localizeChangelog(BILINGUAL, "zh-CN")).toContain("中文行");
  });

  it("falls back to English when the requested section is missing", () => {
    expect(localizeChangelog("<!-- lang:en -->\nonly english", "zh")).toBe(
      "only english",
    );
  });

  it("returns the whole body unchanged when there are no fences", () => {
    expect(localizeChangelog("plain single-language notes", "zh")).toBe(
      "plain single-language notes",
    );
  });
});
