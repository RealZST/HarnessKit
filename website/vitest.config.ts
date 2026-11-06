/// <reference types="vitest/config" />
// getViteConfig applies astro.config.mjs (React integration, tsconfig aliases, dedupe) to tests.
import { getViteConfig } from "astro/config";

export default getViteConfig({
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    include: ["src/**/*.test.{ts,tsx}", "scripts/**/*.test.mjs"],
  },
});
