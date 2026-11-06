import { defineRouteMiddleware } from "@astrojs/starlight/route-data";
// Starlight's entry point carries the types of Astro.locals.starlightRoute.
import type {} from "@astrojs/starlight";

// The docs header's "HarnessKit Docs" goes to the docs overview in the page's language, not to the landing page at
// "/" (Starlight's default). The way back to the landing is the Home link beside it (src/components/docs/SocialIcons.astro).
export const onRequest = defineRouteMiddleware((context) => {
  context.locals.starlightRoute.siteTitleHref = context.locals.starlightRoute.locale === "zh" ? "/zh/docs/" : "/docs/";
});
