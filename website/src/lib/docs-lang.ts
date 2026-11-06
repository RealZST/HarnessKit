// The docs' two languages: English at /docs/, Chinese at /zh/docs/ (astro.config.mjs `locales`). Components on docs
// pages read a page's language from its path, so a Chinese page uses them exactly as its English page does.
export type DocsLang = "en" | "zh";

export const docsLang = (pathname: string): DocsLang => (pathname.startsWith("/zh/") ? "zh" : "en");

/**
 * The same page in the other language: /docs/faq/ ↔ /zh/docs/faq/. From a not-found page (/404, /zh/404/), which
 * stands in for any missing address, the other language's docs overview.
 */
export const otherLangPath = (pathname: string): string => {
  const zh = docsLang(pathname) === "zh";
  if (/\/404(?:\.html|\/)?$/.test(pathname)) return zh ? "/docs/" : "/zh/docs/";
  return zh ? pathname.slice("/zh".length) : `/zh${pathname}`;
};
