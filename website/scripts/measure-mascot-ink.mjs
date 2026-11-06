// Measures where each App mascot draws inside its square, for src/data/mascot-ink.ts. The landing's agent cards show
// every mascot at 36px; with the cards' words hidden, it screenshots each mascot with room around it (some draw past
// their square), finds the rows its ink covers, and prints how far the ink's middle sits from the square's, as a
// fraction of the size (negative: above). Re-run when the App's mascots change, and paste the result into the table.
//   npm run dev      (or serve any build)
//   node scripts/measure-mascot-ink.mjs [baseUrl]
// Puppeteer is the App's devDependency, not the site's, so it is resolved from the repo root.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(fileURLToPath(new URL("../../package.json", import.meta.url)));
const puppeteer = require("puppeteer");

const BASE = process.argv[2] ?? "http://localhost:4321";
const SCALE = 4;

const browser = await puppeteer.launch({ headless: "new" });
const page = await browser.newPage();
await page.setViewport({ width: 1440, height: 900, deviceScaleFactor: SCALE });
// At rest, light: no hover pose, ink on the paper.
await page.emulateMediaFeatures([{ name: "prefers-reduced-motion", value: "reduce" }, { name: "prefers-color-scheme", value: "light" }]);
await page.goto(BASE, { waitUntil: "networkidle0" });
await page.addStyleTag({ content: ".ov-name, .ov-caption, .note, .overview .heading { visibility: hidden !important; }" });
await page.evaluate(() => document.querySelector(".ov-wall").scrollIntoView({ block: "center" }));
await new Promise((r) => setTimeout(r, 1500));

const boxes = await page.$$eval(".ov-card [class^='mascot-']", (els) => els.map((m) => {
  const r = m.getBoundingClientRect();
  // Named by the App's mascot class; the table is by agent id, and Devin Desktop's id is still windsurf.
  const mascot = m.className.split(" ")[0].replace("mascot-", "");
  return { id: mascot === "devin" ? "windsurf" : mascot, x: r.left, y: r.top + window.scrollY, size: r.height };
}));
const offsets = {};
for (const b of boxes) {
  const pad = b.size * 0.6;
  const clip = { x: b.x - pad, y: b.y - pad, width: b.size + 2 * pad, height: b.size + 2 * pad };
  const png = await page.screenshot({ clip, encoding: "base64" });
  // The ink's rows: any pixel that differs from the corner's (the paper).
  const [top, bottom] = await page.evaluate(async (data) => {
    const img = new Image();
    img.src = `data:image/png;base64,${data}`;
    await img.decode();
    const canvas = Object.assign(document.createElement("canvas"), { width: img.width, height: img.height });
    const ctx = canvas.getContext("2d");
    ctx.drawImage(img, 0, 0);
    const { data: px } = ctx.getImageData(0, 0, img.width, img.height);
    const bg = [px[0], px[1], px[2]];
    let first = -1, last = -1;
    for (let y = 0; y < img.height; y++) {
      for (let x = 0; x < img.width; x++) {
        const i = (y * img.width + x) * 4;
        if (Math.abs(px[i] - bg[0]) + Math.abs(px[i + 1] - bg[1]) + Math.abs(px[i + 2] - bg[2]) > 40) {
          if (first < 0) first = y;
          last = y;
          break;
        }
      }
    }
    return [first, last + 1];
  }, png);
  const inkMiddle = clip.y + (top + bottom) / 2 / SCALE;
  offsets[b.id] = Math.round(((inkMiddle - (b.y + b.size / 2)) / b.size) * 1000) / 1000;
}
await browser.close();
console.log(JSON.stringify(offsets, null, 2));
