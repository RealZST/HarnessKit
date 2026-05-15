import i18n from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { initReactI18next } from "react-i18next";

export const SUPPORTED_LANGUAGES = ["en", "zh"] as const;
export type SupportedLanguage = (typeof SUPPORTED_LANGUAGES)[number];

export const LANGUAGE_STORAGE_KEY = "hk-language";

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

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: "en",
    supportedLngs: SUPPORTED_LANGUAGES,
    defaultNS: "common",
    ns: NAMESPACES,
    interpolation: {
      escapeValue: false,
    },
    detection: {
      // localStorage only — do NOT auto-detect from navigator.language.
      // The product default is English; users opt into other languages
      // via Settings, which persists their choice in localStorage.
      order: ["localStorage"],
      lookupLocalStorage: LANGUAGE_STORAGE_KEY,
      caches: ["localStorage"],
    },
  });

export default i18n;
