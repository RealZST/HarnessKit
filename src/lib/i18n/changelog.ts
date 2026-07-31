import { mapLocaleToSupportedLanguage } from "./index";

// Matches a language fence like `<!-- lang:en -->` / `<!-- lang:zh -->`.
const LANG_FENCE = /<!--\s*lang:([a-z-]+)\s*-->/gi;

// Matches a language comment BLOCK: the whole translation lives inside one
// HTML comment, e.g. `<!-- lang:zh\n## 更新内容 ...\n-->`. The newline after
// the language code is what distinguishes a block from a legacy fence (whose
// `-->` closes on the same line), so the two formats can't shadow each other.
const LANG_COMMENT_BLOCK = /<!--\s*lang:([a-z-]+)[ \t]*\n([\s\S]*?)-->/gi;

/**
 * Pick the section of a changelog body matching `language`.
 *
 * Preferred authoring format — English as plain text, translations inside
 * comment blocks, so GitHub's release page (which doesn't render comments)
 * shows only English while clients localize from the same body:
 *
 *   ## What's new ...
 *   <!-- lang:zh
 *   ## 更新内容 ...
 *   -->
 *
 * Clients without comment-block support (≤1.7.0) don't display comments in
 * rendered markdown, so old versions degrade gracefully to English.
 *
 * The legacy fence format is still supported:
 *
 *   <!-- lang:en -->
 *   ## What's new ...
 *   <!-- lang:zh -->
 *   ## 更新内容 ...
 *
 * Returns the section for the active language, falling back to English, then to
 * the first section present. Notes without any fence or block are returned
 * unchanged, so single-language releases keep working.
 */
// GitHub's auto-generated release tail. It sits in the English plain text,
// but PR titles are English regardless of UI language — non-English
// sections borrow it from the English one (with a localized heading).
const NEUTRAL_TAIL = /^## What's Changed\s*$/m;

export function localizeChangelog(body: string, language: string): string {
  const sections: Record<string, string> = {};

  // Bodies fetched from the GitHub API use \r\n; LANG_COMMENT_BLOCK anchors
  // on \n, so normalize first or block extraction silently fails.
  const normalized = body.replace(/\r\n/g, "\n");

  // Pass 1: pull out comment-block translations; what remains is plain text.
  const remainder = normalized
    .replace(LANG_COMMENT_BLOCK, (_match, code: string, content: string) => {
      const key = mapLocaleToSupportedLanguage(code) ?? code.toLowerCase();
      sections[key] = content.trim();
      return "";
    })
    .trim();

  // Pass 2: legacy fences split the remaining text into per-language sections.
  const fences = [...remainder.matchAll(LANG_FENCE)];
  if (fences.length > 0) {
    fences.forEach((fence, i) => {
      const start = (fence.index ?? 0) + fence[0].length;
      const end =
        i + 1 < fences.length
          ? (fences[i + 1].index ?? remainder.length)
          : remainder.length;
      const key =
        mapLocaleToSupportedLanguage(fence[1]) ?? fence[1].toLowerCase();
      sections[key] = remainder.slice(start, end).trim();
    });
  } else if (remainder && !sections.en) {
    // Comment-block format: the un-commented plain text IS the English section.
    sections.en = remainder;
  }

  if (Object.keys(sections).length === 0) return normalized.trim();

  const lang = mapLocaleToSupportedLanguage(language) ?? "en";
  const selected =
    sections[lang] ?? sections.en ?? Object.values(sections)[0] ?? normalized.trim();

  // Borrow the English tail for non-English sections.
  if (lang !== "en" && sections.en && selected !== sections.en) {
    const tailStart = sections.en.search(NEUTRAL_TAIL);
    if (tailStart !== -1) {
      const tail = sections.en
        .slice(tailStart)
        .replace(NEUTRAL_TAIL, "## 变更列表");
      return `${selected}\n\n${tail}`;
    }
  }
  return selected;
}
