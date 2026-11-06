// Posters for the docs' demo videos: `node scripts/docs-posters.mjs` finds every <video> in the docs pages, reads
// its first frame straight from the GitHub user-attachments URL with ffmpeg (which fetches only what that frame
// needs), and writes it to public/docs-posters/<asset id>.jpg, 1120px wide and at most 40 KB. The first frame is
// the one playback opens on, so pressing play doesn't jump. Needs ffmpeg on the PATH (brew install ffmpeg).
import { execFileSync } from "node:child_process";
import { globSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SITE = fileURLToPath(new URL("..", import.meta.url));
const DOCS = path.join(SITE, "src/content/docs");
const OUT = path.join(SITE, "public/docs-posters");
const VIDEO_SRC = /<video\b[^>]*\bsrc="(https:\/\/github\.com\/user-attachments\/assets\/([0-9a-f-]+))"/g;
const WIDTH = 1120;
const MAX_BYTES = 40_000;

const videos = new Map();
for (const page of globSync("**/*.{md,mdx}", { cwd: DOCS })) {
  for (const [, url, id] of readFileSync(path.join(DOCS, page), "utf8").matchAll(VIDEO_SRC)) videos.set(id, url);
}

mkdirSync(OUT, { recursive: true });
for (const [id, url] of videos) {
  const frame = execFileSync("ffmpeg", ["-v", "error", "-i", url, "-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"], { maxBuffer: 64 * 1024 * 1024 });
  // Quality 60 keeps every current poster under the limit; a busier frame steps down until it fits.
  let jpeg;
  for (const quality of [60, 55, 50, 45, 40]) {
    jpeg = await sharp(frame).resize({ width: WIDTH }).jpeg({ quality, mozjpeg: true }).toBuffer();
    if (jpeg.length <= MAX_BYTES) break;
  }
  writeFileSync(path.join(OUT, `${id}.jpg`), jpeg);
  console.log(`${id}.jpg  ${(jpeg.length / 1000).toFixed(1)} KB`);
}
