# HarnessKit website

The landing page and the docs site, built with [Astro](https://astro.build) and [Starlight](https://starlight.astro.build). Static output, no server.

| Path | What it is |
|---|---|
| `/` | Landing page (English) |
| `/zh/` | Landing page (Chinese) |
| `/docs/` | Documentation |
| `/zh/docs/` | Documentation (Chinese) |

## Run it

Node 22 or newer and npm (`engines` in package.json).

```sh
cd website
npm install
npm run dev        # http://localhost:4321
npm test           # vitest
npm run check      # astro check (types + .astro)
npm run build      # fetch releases → astro check → astro build → dist/
node scripts/check-targets.mjs [url]   # touch targets on a served build (default http://localhost:4321)
```

Before shipping a landing change, check its touch targets by hand: `npm run build && npx astro preview`, then `node scripts/check-targets.mjs`. On a phone (390px, iPhone) and on touch tablets (768px, 1024px) it hit-tests a 44×44px square around every `[data-hit="own"]` link and `.tap-target`, its `::after` included, and exits 1 with each one that falls short or is covered by a neighbour. `npm test` doesn't run it, as it needs a built site and a browser; `src/components/landing/tap-targets.test.ts`, which does, only checks that every landing link carries one of the two marks.

`npm run build` first runs `scripts/fetch-releases.mjs`, which writes the latest release's desktop installers (Apple Silicon and Intel DMGs, Windows setup) to `src/data/downloads.json` and the star count to `src/data/repo.json`. Offline or rate-limited, it keeps the committed files and the build goes on. Set `GITHUB_TOKEN` in CI to avoid the anonymous rate limit. The download buttons link those installers directly. A release needs no rebuild: once a page has loaded, `src/lib/live-release.ts` asks GitHub for the latest release and the star count and updates the links (`a[data-asset]`) and the nav (`[data-stars]`); where GitHub can't be reached, the built-in links stay. An installer the release lacks links the release page instead.

## Where things live

| You want to change | Edit |
|---|---|
| Landing copy | `src/data/copy/en.ts`, `src/data/copy/zh.ts` (same shape, `CopyShape`) |
| Landing blocks | `src/pages/index.astro`, `src/pages/zh/index.astro` (same block order), components in `src/components/landing/` |
| Download page and the hero's buttons | `src/pages/download.astro`, `src/pages/zh/download.astro` (both render `DownloadPage.astro`); the hero's and the final block's two buttons are `GetButtons.astro`; the platform check that picks the leading download is in `src/layouts/Base.astro` |
| Animated mocks in the feature blocks, and the Problem block's before/after | `src/components/landing/mocks/` (`mock-data.ts` holds the fake rows; `MOCK_GATHER` is the Problem block's), `src/components/landing/Gather.astro` |
| Colors, fonts, spacing | `src/styles/tokens.css` only (the App's Tiesen theme, light and dark). No color literals anywhere else |
| Docs pages | `src/content/docs/docs/**` (Markdown / MDX), their Chinese pages at the same paths under `src/content/docs/zh/docs/**`; sidebar order and group names in `astro.config.mjs` |
| Docs theme overrides | `src/styles/starlight.css` |
| Agent list and counts | `src/data/agents.ts` (order must match `src/lib/types.ts` `AGENT_ORDER` in the app, a test checks it) |
| Agent directories and formats (docs reference table) | `src/data/agent-reference.ts`, its sentences in Chinese in `src/data/agent-reference-zh.ts` |
| One MCP server in several agents' config files (docs, Installing an MCP server) | `src/data/mcp-snippets.ts` (file and format come from `agent-reference.ts`), `src/components/docs/McpSheets.astro` |
| FAQ | `src/data/copy/en.ts` and `src/content/docs/docs/faq.md` must say the same thing, as must `zh.ts` and `src/content/docs/zh/docs/faq.md` (`faq-sync.test.ts`) |

### Adding an agent

1. Add the row to `src/data/agents.ts` in `AGENT_ORDER` position, and to `src/data/agent-reference.ts`.
2. Add its mascot to the app first (`src/components/shared/agent-mascot/`). The site imports the app's mascots through the `@app-mascots/*` alias; nothing to copy.
3. Give its Overview card a sample count in `MOCK_EXT_COUNTS` (`src/components/landing/mocks/mock-data.ts`) and a start position in `SCATTER` (`src/components/landing/OverviewLive.tsx`).
4. Run `npm test`. Counts in copy come from `AGENT_COUNT`, so the prose updates itself, and `src/data/agent-card.test.ts` says if the App's card shows a shorter name or a new click duration.

## Writing rules

These are enforced by tests where possible. See `/docs/contributing/writing-docs/` for the full version.

- Short sentences, plain words. English technical terms stay English in the Chinese copy.
- Banned words: blazing, seamless, powerful, supercharge, unlock, effortless, robust, elevate, revolutionize, frontier.
- Numbers come from constants (`AGENT_COUNT`, `AUDIT_RULE_COUNT`), never typed into prose.
- Source citations only in a docs page's "Under the hood" section, as repo paths without line numbers. `src/content/cited-paths.test.ts` checks the paths exist.
- `--ink-faint` is for strokes and dividers, never text.
- One hand-drawn annotation per screen, at most.

## Media

The site hosts no video. The Overview under the hero is live markup with the App's mascots (`OverviewLive.tsx`), and the docs embed the README's feature clips by their GitHub user-attachments URLs, so the README's GIF and videos stay the source of truth.

Each docs video has a poster, its first frame (`public/docs-posters/<asset id>.jpg`, at most 40 KB), so iOS Safari shows the clip instead of a black box before it plays. After adding or replacing a video, give its `<video>` a `poster="/docs-posters/<asset id>.jpg"` and run:

```sh
node scripts/docs-posters.mjs   # needs ffmpeg; reads each video's first frame from GitHub → public/docs-posters/
```

`src/content/docs-posters.test.ts` fails while a docs video has no poster or one over 40 KB.

The share images (`public/og.png`, `public/og-zh.png`, 1200×630) are the page `src/pages/og/[lang].astro`, which only builds in dev and for the script. After changing the hero title or the page:

```sh
node scripts/og.mjs   # build with the /og/ pages → screenshot → public/og.png, public/og-zh.png
```

Run it on a Mac: the zh title takes its glyphs from PingFang SC. `npm run dev` shows the page at `/og/en/` and `/og/zh/`.

## Chinese type

The zh pages, landing and docs, set Chinese as Apple's Chinese site does: the system's own Chinese sans (PingFang SC on Apple devices, Microsoft YaHei on Windows, Noto Sans CJK on Linux), headings semibold, with the Latin inside it in Geist (`--font-zh` in `src/styles/tokens.css`, rules in `src/styles/global.css` and, for the docs, `src/styles/starlight.css`). No font file is served for it.

## Assets still missing

The code for each is in place and waits for the file.

| Asset | Where it goes | Notes |
|---|---|---|
| Three app screenshots (macOS, Windows, browser) | `src/components/landing/mocks/EverywherePanel.astro` | Until then the block draws one App window, and its switch changes only the frame. |
| Monochrome logo | `Nav.astro`, `Footer.astro` | Currently the App's two icons, side by side, next to the wordmark. |
| Current screenshots | `src/assets/screens/` (hero, download page, docs Overview) | `overview.png` was shot for the site on 2026-11-05: eight agents (Claude Code, Codex, Gemini CLI, Cursor, DeepSeek, OpenCode, Antigravity, Copilot), cut out of the capture inside the macOS outline, corners re-masked with a faint hairline, no shadow (the site adds its own). The repo's `media/overview.png` should get the same picture. `scripts/clean-window-shot.py` cuts a window out of a capture this way. The Settings guide's two theme shots are the repo's `media/theme-*.png`, referenced in place from the MDX (no copy under `website/`); they predate Kits and the rename of Windsurf to Devin Desktop, so re-shoot them when the App next changes. |

## Deploying

Static output in `dist/`. For Cloudflare Pages: root directory `website`, build command `npm run build`, output directory `dist`, Node 22 or newer, optional `GITHUB_TOKEN`. The site reads four places outside `website/`: the App's mascots (`src/components/shared/agent-mascot/`), `src/lib/release-assets.mjs` (the build's release fetch), `media/theme-*.png` and `src/lib/types.ts` (a test), so a build-watch-paths filter should include those too. Any static host works: the only request a page makes at runtime is to the GitHub API for the latest release's download links and star count (`src/lib/live-release.ts`), with the build's values as the fallback, so a release needs no redeploy.
