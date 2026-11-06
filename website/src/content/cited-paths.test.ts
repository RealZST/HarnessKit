// @vitest-environment node
import { existsSync, globSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// "How it works" pages cite source files under "## Under the hood" (their Chinese pages: "## 相关源码"). A cited
// path that no longer exists means the page has drifted from the code, so the test suite says so. Reference and
// Contributing pages may have the section too; when they do, the same rules apply.
const REPO_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const CONTENT_DIR = fileURLToPath(new URL("./docs/", import.meta.url));
/** The English pages, then the Chinese ones (astro.config.mjs locales). */
const LANG_DIRS = ["docs", "zh/docs"];
const DESIGN_DIR = "how-it-works";
const OTHER_DIRS = ["reference", "contributing"];
const HUB_PAGE = "how-it-works/index.mdx";
const MAX_BULLETS = 5;

function pagesIn(langDir: string, dir: string) {
  const folder = join(CONTENT_DIR, langDir, dir);
  if (!existsSync(folder)) return [];
  return readdirSync(folder)
    .filter((name) => /\.mdx?$/.test(name))
    .sort()
    .map((name) => ({ name: `${langDir}/${dir}/${name}`, section: underTheHoodSection(readFileSync(join(folder, name), "utf8")) }));
}

const designPages = LANG_DIRS.flatMap((langDir) => pagesIn(langDir, DESIGN_DIR));
const designNotes = designPages.filter((page) => !page.name.endsWith(`/${HUB_PAGE}`));
const otherPages = LANG_DIRS.flatMap((langDir) => OTHER_DIRS.flatMap((dir) => pagesIn(langDir, dir)));
const pages = [...designPages, ...otherPages.filter((page) => page.section !== null)];

/** The text between "## Under the hood" (or "## 相关源码") and the next H2, or null when the page has no such section. */
function underTheHoodSection(markdown: string): string | null {
  const heading = /^## (?:Under the hood|相关源码)[ \t]*$/m.exec(markdown);
  if (!heading) return null;
  const rest = markdown.slice(heading.index + heading[0].length);
  const nextHeading = /^## /m.exec(rest);
  return nextHeading ? rest.slice(0, nextHeading.index) : rest;
}

/** A backticked span is a path when it has a "/" or ends in a file extension; `HARNESSKIT_HOME` is not. */
function isPathLike(span: string): boolean {
  return span.includes("/") || /\.[A-Za-z0-9]+$/.test(span);
}

/** Backticked paths in a section. Entries with ":" name another repo, so they are skipped. */
function citedPaths(section: string): string[] {
  return [...section.matchAll(/`([^`\n]+)`/g)]
    .map((match) => match[1])
    .filter((span) => isPathLike(span) && !span.includes(":"));
}

function bulletCount(section: string): number {
  return section.split("\n").filter((line) => /^[-*] /.test(line)).length;
}

function existsInRepo(path: string): boolean {
  return path.includes("*") ? globSync(path, { cwd: REPO_ROOT }).length > 0 : existsSync(join(REPO_ROOT, path));
}

const citations = pages.flatMap(({ name, section }) => citedPaths(section ?? "").map((path) => ({ page: name, path })));

describe("Docs source citations", () => {
  it("finds the design-note pages", () => {
    expect(designNotes.length).toBeGreaterThan(0);
  });

  it.each(designNotes)("$name has an Under the hood section that cites a path", ({ section }) => {
    expect(section, "missing ## Under the hood").not.toBeNull();
    expect(citedPaths(section ?? "").length).toBeGreaterThanOrEqual(1);
  });

  it.each(pages)("$name links paths, not line numbers", ({ section }) => {
    expect(section ?? "").not.toMatch(/\.\w+:\d+/);
  });

  it.each(pages)("$name has at most five Under the hood bullets", ({ section }) => {
    expect(bulletCount(section ?? "")).toBeLessThanOrEqual(MAX_BULLETS);
  });

  it.each(citations)("$page cites $path, which exists in the repo", ({ path }) => {
    expect(existsInRepo(path), `${path} is not in the repo`).toBe(true);
  });
});
