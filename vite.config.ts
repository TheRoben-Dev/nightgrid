import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

const host = process.env.TAURI_DEV_HOST;

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/app/**", "**/core/**", "**/target/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    // Tauri webviews: WebView2 (Chromium), WKWebView (Safari 15+), WebKitGTK.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome110" : "safari15",
    minify: !process.env.TAURI_ENV_DEBUG ? "oxc" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    root: ".",
    include: ["ui/src/**/*.test.ts"],
    environment: "jsdom",
  },
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
});
