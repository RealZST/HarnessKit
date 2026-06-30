import { mapLocaleToSupportedLanguage } from "./index";

// Matches a language fence like `<!-- lang:en -->` / `<!-- lang:zh -->`.
const LANG_FENCE = /<!--\s*lang:([a-z-]+)\s*-->/gi;

/**
 * Pick the section of a changelog body matching `language`.
 *
 * Release notes can be authored bilingually by fencing each language:
 *
 *   <!-- lang:en -->
 *   ## What's new ...
 *   <!-- lang:zh -->
 *   ## 更新内容 ...
 *
 * Returns the section for the active language, falling back to English, then to
 * the first section present. Notes without any fence are returned unchanged, so
 * single-language releases keep working.
 */
export function localizeChangelog(body: string, language: string): string {
  const fences = [...body.matchAll(LANG_FENCE)];
  if (fences.length === 0) return body.trim();

  const sections: Record<string, string> = {};
  fences.forEach((fence, i) => {
    const start = (fence.index ?? 0) + fence[0].length;
    const end =
      i + 1 < fences.length
        ? (fences[i + 1].index ?? body.length)
        : body.length;
    const key =
      mapLocaleToSupportedLanguage(fence[1]) ?? fence[1].toLowerCase();
    sections[key] = body.slice(start, end).trim();
  });

  const lang = mapLocaleToSupportedLanguage(language) ?? "en";
  return (
    sections[lang] ?? sections.en ?? Object.values(sections)[0] ?? body.trim()
  );
}
