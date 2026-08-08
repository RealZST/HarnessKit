import i18n from "i18next";
import { initReactI18next } from "react-i18next";

export const SUPPORTED_LANGUAGES = ["en", "zh-TW", "zh"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];
export const LANGUAGE_PREFERENCES = ["system", ...SUPPORTED_LANGUAGES] as const;
export type LanguagePreference = (typeof LANGUAGE_PREFERENCES)[number];

export const LANGUAGE_STORAGE_KEY = "hk-language";

// Single source of truth for language fallbacks; languages not listed fall
// straight back to English. Consumed by the i18next config below and by
// changelog localization, so UI strings and release notes degrade the same way.
export const FALLBACK_CHAINS: Partial<
  Record<SupportedLanguage, SupportedLanguage[]>
> = {
  "zh-TW": ["zh", "en"],
};

// Eagerly load all locale JSON at build time. Path format: "./locales/<lang>/<namespace>.json"
const localeModules = import.meta.glob<Record<string, unknown>>(
  "./locales/*/*.json",
  { eager: true, import: "default" },
);

type Resources = Record<string, Record<string, Record<string, unknown>>>;

const resources: Resources = {};
for (const [path, content] of Object.entries(localeModules)) {
  const match = path.match(/\.\/locales\/([^/]+)\/([^/]+)\.json$/);
  if (!match) continue;
  const [, lang, ns] = match;
  resources[lang] ??= {};
  resources[lang][ns] = content;
}

// Derive namespace list from the English locale (source of truth)
const NAMESPACES = Object.keys(resources.en ?? {});

function isLanguagePreference(
  value: string | null | undefined,
): value is LanguagePreference {
  return (LANGUAGE_PREFERENCES as readonly string[]).includes(value ?? "");
}

export function mapLocaleToSupportedLanguage(
  locale: string | null | undefined,
): SupportedLanguage | null {
  if (!locale) return null;

  const normalized = locale.toLowerCase();
  if (
    normalized === "zh-tw" ||
    normalized === "zh-hk" ||
    normalized === "zh-mo" ||
    normalized === "zh-hant" ||
    normalized.startsWith("zh-hant-")
  )
    return "zh-TW";
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh";
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
  return null;
}

export function detectSystemLanguage(): SupportedLanguage {
  if (typeof navigator === "undefined") return "en";

  const candidates = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];

  for (const candidate of candidates) {
    const mapped = mapLocaleToSupportedLanguage(candidate);
    if (mapped) return mapped;
  }

  return "en";
}

export function getStoredLanguagePreference(): LanguagePreference {
  if (typeof localStorage === "undefined") return "system";

  const value = localStorage.getItem(LANGUAGE_STORAGE_KEY);
  return isLanguagePreference(value) ? value : "system";
}

export function resolveLanguagePreference(
  preference: LanguagePreference,
): SupportedLanguage {
  return preference === "system" ? detectSystemLanguage() : preference;
}

export async function applyLanguagePreference(
  preference: LanguagePreference,
): Promise<void> {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(LANGUAGE_STORAGE_KEY, preference);
  }

  await i18n.changeLanguage(resolveLanguagePreference(preference));
}

const initialPreference = getStoredLanguagePreference();
const initialLanguage = resolveLanguagePreference(initialPreference);

i18n.use(initReactI18next).init({
  resources,
  lng: initialLanguage,
  fallbackLng: { ...FALLBACK_CHAINS, default: ["en"] },
  supportedLngs: SUPPORTED_LANGUAGES,
  defaultNS: "common",
  ns: NAMESPACES,
  interpolation: {
    escapeValue: false,
  },
});

export default i18n;
