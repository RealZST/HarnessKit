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

const COMMENT_BLOCK = `## What's new
English line

<!-- lang:zh
## 更新内容
中文行
-->

## What's Changed
* some PR`;

describe("localizeChangelog comment blocks", () => {
  it("extracts the block for zh and drops the plain text", () => {
    const zh = localizeChangelog(COMMENT_BLOCK, "zh");
    expect(zh).toContain("中文行");
    expect(zh).not.toContain("English line");
  });

  it("treats the un-commented plain text as the English section", () => {
    const en = localizeChangelog(COMMENT_BLOCK, "en");
    expect(en).toContain("English line");
    expect(en).toContain("What's Changed");
    expect(en).not.toContain("中文行");
    expect(en).not.toContain("<!--");
  });

  it("falls back to the plain text for unsupported languages", () => {
    expect(localizeChangelog(COMMENT_BLOCK, "fr")).toContain("English line");
  });

  it("normalizes regional codes for blocks", () => {
    expect(localizeChangelog(COMMENT_BLOCK, "zh-CN")).toContain("中文行");
  });

  it("does not mistake legacy fences for comment blocks", () => {
    const zh = localizeChangelog(BILINGUAL, "zh");
    expect(zh).toContain("中文行");
    expect(zh).not.toContain("English line");
  });
});
