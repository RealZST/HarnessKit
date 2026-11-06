// Measures the landing's touch targets on the built pages: every [data-hit="own"] link and every .tap-target must take
// taps over a 44×44px square centred on it, its ::after included, on a phone (390px, iPhone) and on touch tablets
// (768px and 1024px). It hit-tests points across that square, so a neighbour covering part of it fails too.
// tap-targets.test.ts checks the templates mark each link; this checks the marks are true.
//   npm run build && npx astro preview      (or serve any build)
//   node scripts/check-targets.mjs [baseUrl]      → exits 1 and lists each target that falls short
// Puppeteer is the App's devDependency, not the site's, so it is resolved from the repo root.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(fileURLToPath(new URL("../../package.json", import.meta.url)));
const puppeteer = require("puppeteer");

const BASE = process.argv[2] ?? "http://localhost:4321";
const PAGES = ["/", "/zh/", "/download/", "/zh/download/"];
const IPHONE_UA = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";
const TABLET_UA = "Mozilla/5.0 (Linux; Android 14; SM-X710) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
const RUNS = [
  { name: "phone 390", width: 390, height: 844, ua: IPHONE_UA },
  { name: "tablet 768", width: 768, height: 1024, ua: TABLET_UA },
  { name: "tablet 1024", width: 1024, height: 768, ua: TABLET_UA },
];
// The square's half side, 1px inside 22px: the browser snaps boxes to whole pixels when it hit-tests, so a 44px row
// that starts at a fraction of a pixel loses up to 1px at an edge. And how many points a side it tests.
const HALF = 21;
const STEPS = 5;

const browser = await puppeteer.launch({ headless: "new" });
let failures = 0;
for (const pagePath of PAGES) {
  for (const run of RUNS) {
    const page = await browser.newPage();
    page.on("pageerror", (e) => console.error(pagePath, run.name, e.message));
    await page.setUserAgent(run.ua);
    // Chromium's own userAgentData would name the Mac it runs on; the page's platform script then reads the UA.
    await page.evaluateOnNewDocument(() => Object.defineProperty(navigator, "userAgentData", { get: () => undefined }));
    await page.setViewport({ width: run.width, height: run.height, deviceScaleFactor: 1, isMobile: true, hasTouch: true });
    // Reduced motion: the gathers and draw-ins sit in place, so nothing moves under the probe.
    await page.emulateMediaFeatures([{ name: "prefers-reduced-motion", value: "reduce" }]);
    await page.goto(new URL(pagePath, BASE).href, { waitUntil: "networkidle0" });
    const short = await page.evaluate(async ({ HALF, STEPS }) => {
      const found = [];
      for (const el of document.querySelectorAll('[data-hit="own"], .tap-target')) {
        if (!el.checkVisibility({ checkOpacity: false, checkVisibilityCSS: true })) continue;
        el.scrollIntoView({ block: "center", inline: "center", behavior: "instant" });
        await new Promise(requestAnimationFrame);
        const box = el.getBoundingClientRect();
        if (!box.width || !box.height) continue;
        const cx = box.left + box.width / 2;
        const cy = box.top + box.height / 2;
        let missed = 0;
        for (let i = 0; i < STEPS; i++) {
          for (let j = 0; j < STEPS; j++) {
            const x = cx - HALF + (2 * HALF * i) / (STEPS - 1);
            const y = cy - HALF + (2 * HALF * j) / (STEPS - 1);
            const hit = document.elementFromPoint(x, y);
            if (!hit || !el.contains(hit)) missed++;
          }
        }
        if (missed) {
          const name = (el.getAttribute("aria-label") || el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 40);
          found.push(`<${el.tagName.toLowerCase()}> "${name}" ${Math.round(box.width)}×${Math.round(box.height)}px, ${missed}/${STEPS * STEPS} points miss`);
        }
      }
      return found;
    }, { HALF, STEPS });
    for (const line of short) console.log(`${pagePath} ${run.name}: ${line}`);
    failures += short.length;
    await page.close();
  }
}
await browser.close();
console.log(failures ? `${failures} target(s) under 44×44px` : "Every target takes a 44×44px tap.");
process.exit(failures ? 1 : 0);
