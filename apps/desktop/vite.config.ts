import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
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
