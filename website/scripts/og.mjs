// The share images: `node scripts/og.mjs` builds the site with its /og/ pages (src/pages/og/[lang].astro, HK_OG=1)
// into a temporary folder, serves it, and screenshots each page at 1200×630 into public/og.png and public/og-zh.png.
// Puppeteer is the App's devDependency, not the site's, so it is resolved from the repo root. The zh headline
// takes its glyphs from the system's Chinese sans (tokens.css --font-heading-zh): run it on a Mac for PingFang SC.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { createServer } from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(fileURLToPath(new URL("../../package.json", import.meta.url)));
const puppeteer = require("puppeteer");

const SITE = fileURLToPath(new URL("..", import.meta.url));
const PUBLIC = path.join(SITE, "public");
const IMAGES = [
  { page: "/og/en/", file: "og.png" },
  { page: "/og/zh/", file: "og-zh.png" },
];
const TYPES = { ".html": "text/html", ".css": "text/css", ".js": "text/javascript", ".png": "image/png", ".webp": "image/webp", ".woff2": "font/woff2", ".svg": "image/svg+xml" };

const out = mkdtempSync(path.join(tmpdir(), "harnesskit-og-"));
try {
  execFileSync("npx", ["astro", "build", "--outDir", out], { cwd: SITE, stdio: ["ignore", "ignore", "inherit"], env: { ...process.env, HK_OG: "1" } });

  // The built pages use root-relative URLs (/_astro/…), so they need a server, not file://.
  const server = createServer((req, res) => {
    const urlPath = decodeURIComponent(new URL(req.url, "http://localhost").pathname);
    const file = path.join(out, urlPath.endsWith("/") ? `${urlPath}index.html` : urlPath);
    try {
      const body = readFileSync(file);
      res.writeHead(200, { "Content-Type": TYPES[path.extname(file)] ?? "application/octet-stream" }).end(body);
    } catch {
      res.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const base = `http://127.0.0.1:${server.address().port}`;

  const browser = await puppeteer.launch({ headless: "new" });
  try {
    const page = await browser.newPage();
    await page.setViewport({ width: 1200, height: 630, deviceScaleFactor: 1 });
    await page.emulateMediaFeatures([{ name: "prefers-color-scheme", value: "light" }, { name: "prefers-reduced-motion", value: "reduce" }]);
    for (const { page: pagePath, file } of IMAGES) {
      await page.goto(base + pagePath, { waitUntil: "networkidle0" });
      await page.evaluate(() => document.fonts.ready);
      await page.screenshot({ path: path.join(PUBLIC, file), clip: { x: 0, y: 0, width: 1200, height: 630 } });
      console.log(`public/${file}`);
    }
  } finally {
    await browser.close();
    server.close();
  }
} finally {
  rmSync(out, { recursive: true, force: true });
}
