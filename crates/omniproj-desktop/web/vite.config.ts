/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The build is embedded into the omniproj binary (see build.rs / rust-embed), so output
// goes to a committed `dist/`. Relative base so it works served from `/`. R0 is pull-only
// and talks to the backend over Tauri IPC, so there is no dev HTTP proxy.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // One JS + one CSS keeps the embed and the serve handler simple.
    rollupOptions: { output: { manualChunks: undefined } },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    css: true,
    globals: false,
    clearMocks: true,
    mockReset: true,
    restoreMocks: true,
    include: ["src/**/*.test.{ts,tsx}"],
  },
});
