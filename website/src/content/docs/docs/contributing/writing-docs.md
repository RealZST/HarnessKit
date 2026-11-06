---
title: "Writing docs"
description: "Page template, style guide and source citations for these docs."
sidebar:
  order: 2
---

The website is the `website/` folder of this repository, and the maintainers keep it in step with the app. This page holds the conventions it follows. If a page is wrong or something is missing, open an issue, or a pull request that changes the page.

These docs are Markdown and MDX files in `website/src/content/docs/docs/`, built with Astro and Starlight. Each folder there is one sidebar group, and a page's `sidebar.order` sets its place in the group. The groups are listed in `website/astro.config.mjs`: a new folder needs its own entry there, and so does a page outside a folder, such as the FAQ.

## Page template

Every How it works page except the overview (`how-it-works/index.mdx`) has these sections, in this order, and runs 500 to 1200 words. A Chinese page uses the Chinese section names listed under [Chinese pages](#chinese-pages).

1. **In short.** A `:::note[In short]` box that answers the page's question, so a reader can stop there.
2. **The ground it works on.** The facts the mechanism has to deal with, stated as facts: where each agent keeps its files, how the agents differ. Its heading names the content ("How agents store MCP servers"), never a problem to fix. A page with no such ground, such as the audit, skips this section and says in one sentence of In short why the feature exists.
3. **What HarnessKit does.** The steps in the order the data moves: where it comes from, where it is written, how it is updated and removed. Show files a user sees, before and after. Never show Rust or TypeScript.
4. **Why we built it this way.** A `:::tip` box: a short bold title, a few sentences of reasons, and the option we didn't take, with its cost.
5. **Limitations.** A `:::caution[Limitations]` box listing what a user must know to avoid trouble. Describe only what HarnessKit does now and what the user can do about it. Leave out a fault the user can't work around, and keep plans for fixes out of the docs.
6. **Under the hood.** Up to five bullets that name the source files behind the page.
7. **Related.** Link cards to the pages to read next.

## Style guide

Write for two readers. One uses HarnessKit and stops after What HarnessKit does. The other is a developer who reads to the end.

- Use short sentences and plain words, one idea per sentence.
- In Chinese, technical terms stay in English (see Chinese pages, below). The Chinese landing page is written for newcomers, so it keeps only agent, skill and MCP server in English and doesn't stack config, memory, rules, hooks or plugins into a sentence. Its product nouns follow the app's Chinese UI: 扩展, 市场, 信任评分, 每日一帖, 工具集.
- Leave out marketing words. These are banned: blazing, seamless, powerful, supercharge, unlock, effortless, robust, elevate, revolutionize, frontier.
- State facts, and check each one in the code before you write it. When you are not sure, say less.
- Take counts from constants, never type them. `AGENT_COUNT` and `AUDIT_RULE_COUNT` live in `website/src/data/agents.ts`. An MDX page imports them and writes `{AGENT_COUNT}`. Keep counts out of headings: the table of contents and the heading's link show the constant's name, not its value. `website/src/content/docs-counts.test.ts` fails on a count typed into a page.
- Use the display names in `website/src/data/agents.ts`, such as Devin Desktop, DeepSeek Harness, PenguinHarness and Qoder CN.
- Take colours from `website/src/styles/tokens.css` only.
- In a terminal block that shows what a command prints, start each command with `$ `. The prompt shows muted and isn't copied, the output shows plain, and the copy button copies only the commands (`website/src/lib/ec-terminal.mjs`). A block of commands alone needs no `$`.
- Embed a demo video by its GitHub user-attachments URL and give it `poster="/docs-posters/<asset id>.jpg"`. `node scripts/docs-posters.mjs` (run in `website/`, needs ffmpeg) writes each video's first frame there, into `website/public/docs-posters/`. `website/src/content/docs-posters.test.ts` fails on a video without one.

### Chinese pages

Each page has a Chinese page at the same path under `website/src/content/docs/zh/docs/`, served at `/zh/docs/<path>/`. Change both in the same edit. Two rules come before the rest:

1. Use the Chinese the app already uses: its UI strings in `src/lib/i18n/locales/zh/`.
2. Write Chinese that reads as Chinese, not a sentence-by-sentence translation. Keep every fact and add none. Drop phrasing that reads as translated, such as 进行, 通过……来…… and 对于……来说, and words like 强大, 无缝, 一站式 and 赋能.

- **Terms.** agent, skill, plugin, hook, MCP server, CLI and subagent stay English, lowercase, with no plural. In prose, agent is never 代理 or 智能体. Extension is 扩展, kit 工具集, Marketplace 市场, trust score 信任评分, finding 问题, scope 范围 (全局 / 项目), Tip of the Day 每日一帖, web mode Web 模式, desktop app 桌面应用.
- **On-screen labels.** Bold, in the app's Chinese exactly, with → for menu paths: **设置 → 智能体路径**. So the Agents page is **智能体**, while agents in prose stay agent.
- **Spacing and punctuation.** One half-width space between Chinese and Latin letters, numbers or inline code, none next to full-width punctuation. Chinese sentences take full-width punctuation, with 、 between list items, and numbers in Arabic numerals. A bold lead-in keeps its 。 outside the bold (`**扫描**。启动时…`), or CommonMark won't render it.
- **Frontmatter.** Translate `title` and `description`, and `sidebar.label` when the page has one: the Chinese sidebar shows it. Keep the other keys.
- **How it works sections.** `:::note[简单来说]`, a content-named opening section, `## HarnessKit 怎么做`, `:::tip[为什么这样设计]`, `## 局限`, `:::caution[局限]`, `## 相关源码` and `## 延伸阅读`. The citation and count tests read the Chinese pages too.
- **Links.** Site links go to `/zh/docs/…`. An anchor is the Chinese heading's id: English letters lowercased, spaces turned into `-`, punctuation dropped and Chinese characters kept. So `## 启用和禁用` → `#启用和禁用`.
- **Shared text.** The agents reference table's notes are in `website/src/data/agent-reference-zh.ts`, and the Chinese FAQ opens with the Chinese landing's questions (`zh.ts`), as the English one does with `en.ts`. Tests fail when either falls out of step.

## Citing source

- On How it works and Guides pages, cite source files only in the Under the hood section at the end of the page. Contributing pages, like this one, name files wherever a step needs them.
- Write each path relative to the repo root, in backticks, such as `crates/hk-core/src/deployer.rs`. End a folder with `/`.
- Name the file, never a line number. Line numbers go stale with the next change.
- Use at most five bullets, each saying in one sentence what the file does.

`website/src/content/cited-paths.test.ts` checks every How it works page, and any Reference or Contributing page with an Under the hood section. It fails when a cited path is gone from the repo, when a line number appears, or when there are more than five bullets.

## Components and tests

- **Language.** Components on docs pages, such as the agent facts and MCP sheets in `website/src/components/docs/`, read a page's language from its path (`website/src/lib/docs-lang.ts`). A Chinese page uses them exactly as its English page does.
- **Styles.** Starlight's look comes from `website/src/styles/starlight.css`, built on the tokens in `tokens.css`.
- **Screenshots.** App pictures live in `website/src/assets/screens/`. `python3 scripts/clean-window-shot.py` (run in `website/`, needs Pillow) cuts the window out of a desktop capture. Its header comment gives the arguments.
- **A new agent.** Three tests fail until the site catches up with the app. `website/src/data/agents.test.ts` checks `AGENTS` against the app's `AGENT_ORDER`, and its agent count asks for the Overview window's `MOCK_EXT_COUNTS` and `SCATTER` entries in `OverviewLive.tsx`. `website/src/data/agent-card.test.ts` checks `CLICK_DURATIONS` and the card clipping against the app's `agent-card.tsx`. `website/src/data/mascot-ink.test.ts` wants a measured offset for every mascot: run `node scripts/measure-mascot-ink.mjs` with the dev server up, and paste its output into `mascot-ink.ts`.

## Run the site locally

The site has its own `package.json` in `website/`. It needs Node 22 or later, because the tests use `fs.globSync`, and npm.

```sh
cd website
npm ci
npm run dev        # http://localhost:4321
npm test           # vitest, including the citation and agent data checks
npx astro check    # type-checks the pages and components
```

`npm run build` runs the same check and writes the static site to `website/dist/`. First it fetches the latest release's installers and the star count from GitHub. Offline, it keeps the saved copy.
