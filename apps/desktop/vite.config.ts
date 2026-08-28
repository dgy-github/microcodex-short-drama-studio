import { tmpdir } from "node:os";
import { join } from "node:path";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { configDefaults } from "vitest/config";

export default defineConfig({
  cacheDir: join(tmpdir(), `microcodex-vite-${process.pid}`),
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  resolve: {
    conditions: ["browser"],
  },
  test: {
    globals: true,
    environment: "jsdom",
    exclude: [...configDefaults.exclude, "e2e/**"],
    setupFiles: ["./src/test-setup.ts"],
    alias: {
      "svelte/internal/server": "svelte/internal/client",
    },
    server: {
      deps: {
        inline: [/svelte/],
      },
    },
    coverage: {
      provider: "v8",
      reporter: ["text", "html", "json"],
      exclude: [
        "node_modules/**",
        "src-tauri/**",
        "**/*.test.ts",
        "**/*.spec.ts",
        "src/test-setup.ts",
      ],
    },
  },
});
