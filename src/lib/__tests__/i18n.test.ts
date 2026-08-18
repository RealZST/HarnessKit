import { beforeEach, describe, expect, it, vi } from "vitest";

function setNavigatorLanguage(
  language: string,
  languages: string[] = [language],
): void {
  Object.defineProperty(window.navigator, "language", {
    configurable: true,
    value: language,
  });
  Object.defineProperty(window.navigator, "languages", {
    configurable: true,
    value: languages,
  });
}

describe("i18n language preference helpers", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.resetModules();
    setNavigatorLanguage("en-US");
  });

  it("defaults to system preference when storage is empty", async () => {
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("system");
  });

  it("keeps explicit language preferences from storage", async () => {
    localStorage.setItem("hk-language", "zh");
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("zh");
  });

  it("treats invalid stored values as system", async () => {
    localStorage.setItem("hk-language", "ja");
    const { getStoredLanguagePreference } = await import("../i18n");
    expect(getStoredLanguagePreference()).toBe("system");
  });

  it("maps system locales to supported languages", async () => {
    const { mapLocaleToSupportedLanguage } = await import("../i18n");
    expect(mapLocaleToSupportedLanguage("zh-CN")).toBe("zh");
    expect(mapLocaleToSupportedLanguage("en-GB")).toBe("en");
    expect(mapLocaleToSupportedLanguage("ja-JP")).toBeNull();
  });

  it("maps Traditional Chinese locales to zh-TW before the generic zh match", async () => {
    const { mapLocaleToSupportedLanguage } = await import("../i18n");
    expect(mapLocaleToSupportedLanguage("zh-TW")).toBe("zh-TW");
    expect(mapLocaleToSupportedLanguage("zh-HK")).toBe("zh-TW");
    expect(mapLocaleToSupportedLanguage("zh-MO")).toBe("zh-TW");
    expect(mapLocaleToSupportedLanguage("zh-Hant")).toBe("zh-TW");
    expect(mapLocaleToSupportedLanguage("zh-Hant-TW")).toBe("zh-TW");
    expect(mapLocaleToSupportedLanguage("zh-Hans")).toBe("zh");
    expect(mapLocaleToSupportedLanguage("zh-SG")).toBe("zh");
  });

  it("resolves Traditional Chinese system locales to zh-TW", async () => {
    setNavigatorLanguage("zh-TW");
    const { resolveLanguagePreference } = await import("../i18n");
    expect(resolveLanguagePreference("system")).toBe("zh-TW");
  });

  it("falls back zh-TW → zh → en for missing keys", async () => {
    const { default: i18n } = await import("../i18n");
    // Probe keys are test-only, so they sit outside the typed resource keys.
    const t = i18n.t as (key: string, options?: { lng?: string }) => string;
    i18n.addResource("zh", "common", "fallbackProbe", "简体值");
    i18n.addResource("en", "common", "fallbackProbe", "english value");
    expect(t("fallbackProbe", { lng: "zh-TW" })).toBe("简体值");
    i18n.addResource("en", "common", "englishOnlyProbe", "english only");
    expect(t("englishOnlyProbe", { lng: "zh-TW" })).toBe("english only");
  });

  it("resolves system preference from navigator languages", async () => {
    setNavigatorLanguage("fr-FR", ["fr-FR", "zh-CN"]);
    const { resolveLanguagePreference } = await import("../i18n");
    expect(resolveLanguagePreference("system")).toBe("zh");
  });

  it("falls back to English for unsupported system locales", async () => {
    setNavigatorLanguage("ja-JP");
    const { resolveLanguagePreference } = await import("../i18n");
    expect(resolveLanguagePreference("system")).toBe("en");
  });

  it("applies system preference without overwriting the stored setting", async () => {
    setNavigatorLanguage("zh-CN");
    const { applyLanguagePreference, default: i18n } = await import("../i18n");

    await applyLanguagePreference("system");

    expect(localStorage.getItem("hk-language")).toBe("system");
    expect(i18n.resolvedLanguage).toBe("zh");
  });

  it("applies explicit preferences directly", async () => {
    setNavigatorLanguage("en-US");
    const { applyLanguagePreference, default: i18n } = await import("../i18n");

    await applyLanguagePreference("zh");

    expect(localStorage.getItem("hk-language")).toBe("zh");
    expect(i18n.resolvedLanguage).toBe("zh");
  });
});

describe("audit rules locale parity", () => {
  // Every AUDIT_RULES entry renders its label/description through
  // `rules.<camelCase id>.*`, but audit.tsx passes `defaultValue: rule.label`,
  // so a locale missing the key silently renders the English registry string
  // instead of failing loudly. Drive the check off the registry (not off the
  // English bundle) so a rule added with no i18n at all is caught too.
  it("defines label and description for every rule in every supported language", async () => {
    const { default: i18n, SUPPORTED_LANGUAGES } = await import("../i18n");
    // Import the SAME transform audit.tsx renders with — a local copy could
    // drift and leave this guard green while the UI reads untranslated keys.
    const { AUDIT_RULES, ruleI18nKey } = await import("@/pages/audit-utils");

    expect(AUDIT_RULES.map((r) => r.id)).toContain("dsh-js-env-no-fallback");

    for (const lang of SUPPORTED_LANGUAGES) {
      for (const rule of AUDIT_RULES) {
        for (const field of ["label", "description"]) {
          const fullKey = `rules.${ruleI18nKey(rule.id)}.${field}`;
          // Own bundle only — the zh-TW → zh → en fallback chain (and the
          // defaultValue in audit.tsx) would mask a missing translation.
          expect(
            typeof i18n.getResource(lang, "audit", fullKey),
            `${lang} is missing ${fullKey}`,
          ).toBe("string");
        }
      }
    }
  });

  it("has no orphan rules.* keys and keeps en labels in sync with the registry", async () => {
    const { default: i18n } = await import("../i18n");
    const { AUDIT_RULES, ruleI18nKey } = await import("@/pages/audit-utils");
    const englishRules = (i18n.getResource("en", "audit", "rules") ??
      {}) as Record<string, { label?: string; description?: string }>;
    const registryKeys = new Set(AUDIT_RULES.map((r) => ruleI18nKey(r.id)));

    // Reverse direction: an i18n entry whose rule was renamed or removed is
    // dead copy that no longer renders anywhere.
    for (const key of Object.keys(englishRules)) {
      expect(registryKeys, `rules.${key} matches no AUDIT_RULES id`).toContain(
        key,
      );
    }

    // audit.tsx falls back to the registry strings via defaultValue, so both
    // fields must agree or the UI text silently changes with the user's
    // language.
    for (const rule of AUDIT_RULES) {
      expect(
        englishRules[ruleI18nKey(rule.id)]?.label,
        `en label for ${rule.id} differs from the registry`,
      ).toBe(rule.label);
      expect(
        englishRules[ruleI18nKey(rule.id)]?.description,
        `en description for ${rule.id} differs from the registry`,
      ).toBe(rule.description);
    }
  });
});
