import { configDefaults, defineConfig } from "vitest/config";
import path from "path";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
    // The website under website/ has its own package.json and test runner.
    exclude: [...configDefaults.exclude, "website/**"],
  },
  define: {
    __APP_VERSION__: JSON.stringify("1.2.1"),
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
