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
