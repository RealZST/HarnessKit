// @vitest-environment node
import { globSync, readFileSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// iOS Safari draws a video with preload="metadata" as a black box until it plays, so every docs video carries a
// poster: its own first frame, made by scripts/docs-posters.mjs and named after the video's asset id.
const DOCS_DIR = fileURLToPath(new URL("./docs/", import.meta.url));
const PUBLIC_DIR = fileURLToPath(new URL("../../public", import.meta.url));
const MAX_BYTES = 40_000;

const videos = globSync("**/*.{md,mdx}", { cwd: DOCS_DIR })
  .sort()
  .flatMap((page) => [...readFileSync(DOCS_DIR + page, "utf8").matchAll(/<video\b[^>]*>/g)].map(([tag]) => ({ page, tag, label: tag.match(/\baria-label="([^"]+)"/)?.[1] })));

describe("Docs video posters", () => {
  it("finds the docs videos", () => {
    expect(videos.length).toBeGreaterThan(0);
  });

  it.each(videos)("$page: $label has its own poster", ({ tag }) => {
    const id = tag.match(/\bsrc="https:\/\/github\.com\/user-attachments\/assets\/([0-9a-f-]+)"/)?.[1];
    expect(id, "embed the video by its user-attachments URL").toBeDefined();
    const poster = tag.match(/\bposter="([^"]+)"/)?.[1];
    expect(poster, "run node scripts/docs-posters.mjs and add the poster").toBe(`/docs-posters/${id}.jpg`);
    expect(statSync(PUBLIC_DIR + poster).size).toBeLessThanOrEqual(MAX_BYTES);
  });
});
