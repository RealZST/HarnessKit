// Screenshots of the landing pages for review: `node scripts/shots.mjs [baseUrl]` writes shots/<page>-<run>.png.
// Puppeteer is the App's devDependency, not the site's, so it is resolved from the repo root.
import { createRequire } from "node:module";
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const require = createRequire(fileURLToPath(new URL("../../package.json", import.meta.url)));
const puppeteer = require("puppeteer");

const BASE = process.argv[2] ?? "http://localhost:4321";
const OUT = fileURLToPath(new URL("../shots/", import.meta.url));
const PAGES = ["/", "/zh/"];
const RUNS = [
  { name: "desktop", width: 1440, height: 900 },
  { name: "phone", width: 390, height: 844, mobile: true },
];
const MAC_UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

mkdirSync(OUT, { recursive: true });
const browser = await puppeteer.launch({ headless: "new" });
for (const pagePath of PAGES) {
  for (const run of RUNS) {
    const page = await browser.newPage();
    page.on("pageerror", (e) => console.error(pagePath, run.name, e.message));
    await page.setUserAgent(MAC_UA);
    await page.setViewport({ width: run.width, height: run.height, deviceScaleFactor: 1, isMobile: !!run.mobile, hasTouch: !!run.mobile });
    await page.goto(new URL(pagePath, BASE).href, { waitUntil: "networkidle0" });
    await sleep(3000);
    // Scroll through once so client:visible islands hydrate and their draw-ins run.
    const height = await page.evaluate(() => document.documentElement.scrollHeight);
    for (let y = 0; y < height; y += 600) {
      await page.evaluate((yy) => window.scrollTo(0, yy), y);
      await sleep(200);
    }
    await page.evaluate(() => window.scrollTo(0, 0));
    await sleep(800);
    const slug = pagePath === "/" ? "en" : pagePath.replaceAll("/", "");
    await page.screenshot({ path: path.join(OUT, `${slug}-${run.name}-fold.png`) });
    await page.screenshot({ path: path.join(OUT, `${slug}-${run.name}-full.png`), fullPage: true });
    console.log(`${slug} ${run.name}: ${height}px`);
    await page.close();
  }
}
await browser.close();
