import { defineConfig } from "astro/config";
import react from "@astrojs/react";
import starlight from "@astrojs/starlight";
import { createInlineSvgUrl } from "@astrojs/starlight/expressive-code";
import { pluginTerminal } from "./src/lib/ec-terminal.mjs";
import rehypeExternalLinks from "./src/lib/rehype-external-links.mjs";
import rehypeTableRegions from "./src/lib/rehype-table-regions.mjs";
import { en } from "./src/data/copy/en.ts";
import { RELEASES_URL } from "./src/data/agents.ts";

const SITE = "https://harness-kit.dev";
// The landing's English share image (public/og.png, scripts/og.mjs); share tags want an absolute URL.
const SHARE_IMAGE = new URL("/og.png", SITE).href;

export default defineConfig({
  site: SITE,
  output: "static",
  // The CLI getting-started page was folded into the web-UI install page and the command reference (2026-11-05).
  redirects: {
    "/docs/getting-started/cli/": "/docs/reference/cli/",
    "/zh/docs/getting-started/cli/": "/zh/docs/reference/cli/",
  },
  // Docs tables scroll inside a focusable, labelled region, and links that leave the site end in ↗ (Starlight's MDX
  // pages extend this config).
  markdown: { rehypePlugins: [rehypeTableRegions, [rehypeExternalLinks, { site: SITE }]] },
  integrations: [
    react(),
    starlight({
      title: { en: "HarnessKit Docs", "zh-CN": "HarnessKit 文档" },
      // English at /docs/, Chinese at /zh/docs/ (src/content/docs/zh/), as the landing is at / and /zh/. Starlight's
      // zh-CN strings name its own controls (search, "On this page", previous and next).
      defaultLocale: "root",
      locales: { root: { label: "English", lang: "en" }, zh: { label: "简体中文", lang: "zh-CN" } },
      favicon: "/favicon.png",
      // Same mark as the landing nav (Nav.astro); the title stays next to it. Starlight serves the file as is, so
      // this is a 200×120 copy (3× its 40px height in the header), not the 720×433 original.
      logo: { src: "./src/assets/harnesskit-mark-docs.png", alt: "" },
      customCss: ["./src/styles/tokens.css", "./src/styles/starlight.css"],
      components: {
        // Adds the script that keeps docs tables and code sheets out of the tab order while they fit.
        MarkdownContent: "./src/components/docs/MarkdownContent.astro",
        // The landing's three-state theme toggle in place of Starlight's select.
        ThemeSelect: "./src/components/docs/ThemeSelect.astro",
        // A Home link back to the landing, now that the site title opens the docs overview (starlight-route.ts).
        SocialIcons: "./src/components/docs/SocialIcons.astro",
        // The landing's language switch, a link to the same page in the other language, in place of Starlight's select.
        LanguageSelect: "./src/components/docs/LanguageSelect.astro",
      },
      routeMiddleware: "./src/lib/starlight-route.ts",
      // Code blocks on the site's paper and ink: a hairline frame a step above the page, no shadow, and GitHub's
      // syntax colours, which read calmly next to the indigo accent. Terminal blocks name their window and tell a
      // session's commands from its output (src/lib/ec-terminal.mjs; header layout in starlight.css).
      expressiveCode: {
        themes: ["github-dark", "github-light"],
        plugins: [pluginTerminal()],
        styleOverrides: {
          borderRadius: "var(--radius-sm)",
          borderColor: "var(--line)",
          codeBackground: "var(--window)",
          codeForeground: "var(--ink)",
          uiFontSize: "0.8125rem",
          frames: {
            frameBoxShadowCssValue: "none",
            editorBackground: "var(--window)",
            editorTabBarBackground: "var(--window)",
            editorTabBarBorderBottomColor: "var(--line)",
            editorActiveTabBackground: "var(--window)",
            editorActiveTabForeground: "var(--ink-muted)",
            editorActiveTabIndicatorTopColor: "transparent",
            terminalBackground: "var(--window)",
            terminalTitlebarBackground: "var(--window)",
            terminalTitlebarForeground: "var(--ink-muted)",
            terminalTitlebarBorderBottomColor: "var(--line)",
            terminalTitlebarDotsForeground: "var(--ink-muted)",
            terminalTitlebarDotsOpacity: "1",
            // Lucide's terminal icon (>_): the App marks shell access with it, Vercel's docs a terminal block.
            terminalIcon: createInlineSvgUrl(
              `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="4 17 10 11 4 5"/><line x1="12" x2="20" y1="19" y2="19"/></svg>`,
            ),
            inlineButtonForeground: "var(--ink-muted)",
            inlineButtonBackground: "var(--ink)",
            inlineButtonBorderOpacity: "0",
            inlineButtonBackgroundHoverOrFocusOpacity: "0.08",
            inlineButtonBackgroundActiveOpacity: "0.12",
          },
        },
      },
      // Starlight writes the other share tags (title, description, URL, summary_large_image card) itself.
      head: [
        { tag: "meta", attrs: { property: "og:image", content: SHARE_IMAGE } },
        { tag: "meta", attrs: { property: "og:image:width", content: "1200" } },
        { tag: "meta", attrs: { property: "og:image:height", content: "630" } },
        { tag: "meta", attrs: { property: "og:image:alt", content: `HarnessKit · ${en.hero.title}` } },
        { tag: "meta", attrs: { name: "twitter:image", content: SHARE_IMAGE } },
      ],
      // Groups start closed; Starlight opens the one holding the current page.
      // Each label with its Chinese, as the App's zh UI and the landing name them; the pages under each come from
      // their own titles.
      // Getting started stays open: it is the group a first visit needs. Troubleshooting and the FAQ sit before
      // Contributing, in the order a reader with a problem looks.
      sidebar: [
        { label: "Overview", translations: { "zh-CN": "概览" }, slug: "docs" },
        { label: "Getting started", translations: { "zh-CN": "快速开始" }, collapsed: false, autogenerate: { directory: "docs/getting-started" } },
        { label: "Guides", translations: { "zh-CN": "使用指南" }, collapsed: true, autogenerate: { directory: "docs/guides" } },
        { label: "How it works", translations: { "zh-CN": "工作原理" }, collapsed: true, autogenerate: { directory: "docs/how-it-works" } },
        { label: "Reference", translations: { "zh-CN": "参考" }, collapsed: true, autogenerate: { directory: "docs/reference" } },
        { label: "Troubleshooting", translations: { "zh-CN": "常见问题排查" }, slug: "docs/troubleshooting" },
        { label: "FAQ", translations: { "zh-CN": "常见问题" }, slug: "docs/faq" },
        { label: "Contributing", translations: { "zh-CN": "参与贡献" }, collapsed: true, autogenerate: { directory: "docs/contributing" } },
        // The landing footer's Changelog link, so release notes are one click from the docs too.
        { label: "Release notes ↗", translations: { "zh-CN": "更新日志 ↗" }, link: RELEASES_URL, attrs: { target: "_blank", rel: "noopener" } },
      ],
      social: [{ icon: "github", label: "GitHub", href: "https://github.com/RealZST/HarnessKit" }],
    }),
  ],
  vite: {
    // lets islands import the App's mascots from ../src/components/shared/agent-mascot
    server: { fs: { allow: [".", "../src/components/shared/agent-mascot"] } },
    // react/react-dom are already deduped by @vitejs/plugin-react; listed for explicitness. lucide-react must be here or the App's copy (repo-root node_modules) gets bundled next to the site's.
    resolve: { dedupe: ["react", "react-dom", "lucide-react"] },
  },
});
