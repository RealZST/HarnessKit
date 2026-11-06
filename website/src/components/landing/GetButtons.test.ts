// @vitest-environment node
import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { getContainerRenderer as reactRenderer } from "@astrojs/react";
import { loadRenderers } from "astro:container";
import { JSDOM } from "jsdom";
import { describe, expect, it } from "vitest";
import { en, type CopyShape } from "../../data/copy/en";
import { zh } from "../../data/copy/zh";
import { DOWNLOADS } from "../../data/downloads";
import GetButtons from "./GetButtons.astro";

/** The two buttons as a page at `url` renders them. */
async function render(t: CopyShape["hero"], url: string) {
  const container = await AstroContainer.create({ renderers: await loadRenderers([reactRenderer()]) });
  const html = await container.renderToString(GetButtons, { props: { t }, request: new Request(new URL(url, "https://harness-kit.dev")) });
  return new JSDOM(html).window.document;
}
const href = (doc: Document, selector: string) => [...doc.querySelectorAll<HTMLAnchorElement>(selector)].map((a) => a.getAttribute("href"));

describe("GetButtons", () => {
  it("offers each platform's way in and the download page with every way", async () => {
    const doc = await render(en.hero, "/");
    expect(href(doc, '[data-get="mac"]')).toEqual([DOWNLOADS.macArm64.href, DOWNLOADS.macX64.href]);
    expect(href(doc, '[data-get="windows"]')).toEqual([DOWNLOADS.windowsX64.href]);
    // Linux has no desktop app: its way in is the download page's CLI section.
    expect(href(doc, '[data-get="cli"]')).toEqual(["/download/#cli"]);
    expect(doc.querySelector(".other")).toHaveProperty("textContent", `${en.hero.other} →`);
    expect(href(doc, ".other")).toEqual(["/download/"]);
  });

  it("keeps a Chinese page on the Chinese download page", async () => {
    const doc = await render(zh.hero, "/zh/");
    expect(href(doc, '[data-get="cli"]')).toEqual(["/zh/download/#cli"]);
    expect(href(doc, ".other")).toEqual(["/zh/download/"]);
  });

  it("says what to do with a downloaded installer, Windows' first-run warning included", async () => {
    const doc = await render(en.hero, "/");
    const notice = (way: string) => doc.querySelector<HTMLElement>(`[data-get="${way}"]`)?.dataset.notice;
    if (DOWNLOADS.macArm64.direct) expect(notice("mac")).toBe(en.hero.mac.downloading);
    if (DOWNLOADS.windowsX64.direct) expect(notice("windows")).toBe(`${en.hero.windows.downloading} ${en.hero.windows.note}`);
    // A link to the download page leaves nothing to say.
    expect(notice("cli")).toBeUndefined();
  });

  it("says under the buttons that it's free and every system it runs on, with no version to go stale", async () => {
    const doc = await render(en.hero, "/");
    expect(doc.querySelector(".fineprint .facts")?.textContent).toBe(en.hero.available);
    for (const system of ["macOS", "Windows", "Linux", "browser"]) expect(en.hero.available).toContain(system);
    expect(doc.querySelector(".fineprint [role='status']")).not.toBeNull();
  });
});
