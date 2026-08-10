import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The build is embedded into the omniproj binary (see build.rs / rust-embed), so output
// goes to a committed `dist/`. Relative base so it works served from `/`. During dev,
// `vite` proxies /api to a locally-running `omniproj dashboard`.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // One JS + one CSS keeps the embed and the serve handler simple.
    rollupOptions: { output: { manualChunks: undefined } },
  },
  server: {
    proxy: { "/api": "http://127.0.0.1:7700" },
  },
});
