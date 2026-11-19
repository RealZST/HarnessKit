import { RELEASES_URL } from "./agents";

// The docs sidebar (astro.config.mjs); /llms.txt lists the docs in the same groups and order (src/pages/llms.txt.ts).
// Groups start closed; Starlight opens the one holding the current page.
// Each label with its Chinese, as the App's zh UI and the landing name them; the pages under each come from
// their own titles.
// Getting started stays open: it is the group a first visit needs. Troubleshooting and the FAQ sit before
// Contributing, in the order a reader with a problem looks.
export const DOCS_SIDEBAR = [
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
];
