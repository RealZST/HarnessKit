// @vitest-environment node
import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { getContainerRenderer as reactRenderer } from "@astrojs/react";
import { loadRenderers } from "astro:container";
import { JSDOM } from "jsdom";
import { describe, expect, it } from "vitest";
import { en } from "../../data/copy/en";
import { zh } from "../../data/copy/zh";
import { INSTALL_SH } from "../../data/agents";
import { DOWNLOADS } from "../../data/downloads";
import DownloadPage from "./DownloadPage.astro";

// astro:assets records each image a page uses on this global, which the dev server and the build set up; the
// container doesn't.
(globalThis as { astroAsset?: unknown }).astroAsset ??= { referencedImages: new Set() };

/** The download page in `lang`, as its URL serves it. */
async function render(lang: "en" | "zh") {
  const container = await AstroContainer.create({ renderers: await loadRenderers([reactRenderer()]) });
  const path = lang === "zh" ? "/zh/download/" : "/download/";
  const html = await container.renderToString(DownloadPage, { props: { lang }, request: new Request(new URL(path, "https://harness-kit.dev")) });
  return new JSDOM(html).window.document;
}

describe("DownloadPage", () => {
  it("gives the desktop app and the CLI a section each, named by its heading", async () => {
    const doc = await render("en");
    expect(doc.querySelector("h1")?.textContent).toBe(en.download.title);
    const section = (id: string) => doc.getElementById(id)!;
    const heading = (id: string) => doc.getElementById(section(id).getAttribute("aria-labelledby")!)?.textContent;
    expect(heading("desktop")).toBe(en.download.app.title);
    expect(heading("cli")).toBe(en.download.cli.title);
    // The desktop section has both installers; the CLI's has the command.
    const links = [...section("desktop").querySelectorAll("a.way")].map((a) => a.getAttribute("href"));
    expect(links).toEqual(expect.arrayContaining([DOWNLOADS.macArm64.href, DOWNLOADS.windowsX64.href]));
    expect(section("cli").textContent).toContain(INSTALL_SH);
    expect(section("desktop").textContent).not.toContain(INSTALL_SH);
  });

  it("marks the nav's Download as this page, and switches language to the same page", async () => {
    const doc = await render("en");
    expect(doc.querySelector('header a[aria-current="page"]')?.getAttribute("href")).toBe("/download/");
    expect([...doc.querySelectorAll('a[hreflang="zh-CN"]')].map((a) => a.getAttribute("href"))).toEqual(["/zh/download/", "/zh/download/"]);
    expect(doc.querySelector('link[rel="alternate"][hreflang="zh-CN"]')?.getAttribute("href")).toMatch(/\/zh\/download\/$/);
  });

  it("serves the Chinese page in Chinese, with its links on the Chinese pages", async () => {
    const doc = await render("zh");
    expect(doc.documentElement.lang).toBe("zh-CN");
    expect(doc.querySelector("h1")?.textContent).toBe(zh.download.title);
    expect(doc.querySelector('header a[aria-current="page"]')?.getAttribute("href")).toBe("/zh/download/");
    expect([...doc.querySelectorAll('a[hreflang="en"]')].map((a) => a.getAttribute("href"))).toEqual(["/download/", "/download/"]);
  });
});
