// Augments i18next with our resource shape so t() autocompletes and typechecks
// keys against the English locale (source of truth).
import "i18next";

import type agents from "@/lib/i18n/locales/en/agents.json";
import type audit from "@/lib/i18n/locales/en/audit.json";
import type common from "@/lib/i18n/locales/en/common.json";
import type extensions from "@/lib/i18n/locales/en/extensions.json";
import type kits from "@/lib/i18n/locales/en/kits.json";
import type marketplace from "@/lib/i18n/locales/en/marketplace.json";
import type navigation from "@/lib/i18n/locales/en/navigation.json";
import type overview from "@/lib/i18n/locales/en/overview.json";
import type settings from "@/lib/i18n/locales/en/settings.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "common";
    resources: {
      agents: typeof agents;
      audit: typeof audit;
      common: typeof common;
      extensions: typeof extensions;
      kits: typeof kits;
      marketplace: typeof marketplace;
      navigation: typeof navigation;
      overview: typeof overview;
      settings: typeof settings;
    };
  }
}
