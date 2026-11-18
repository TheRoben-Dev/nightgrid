import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { execFileSync } from "node:child_process";
import { freezeSafe } from "./ui/tools/freezeSafe.ts";

/**
 * The bundled Library's content version (D-174): the time of the newest
 * commit touching it, the same rule scripts/kb-pack.mjs uses for packs, so
 * a pack is preferred only when it was built from later content.
 */
function kbVersion(): number {
  try {
    const t = execFileSync("git", ["log", "-1", "--format=%ct", "--", "docs/kb", "ui/src/lib/kb/catalog.ts"], { encoding: "utf8" }).trim();
    return Number(t) || 0;
  } catch {
    return 0;
  }
}

const host = process.env.TAURI_DEV_HOST;

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  root: "ui",
  plugins: [freezeSafe(), svelte()],
  define: { __KB_VERSION__: JSON.stringify(kbVersion()) },
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
    // xterm.js is most of the bundle; it is loaded locally, so size is not a network cost.
    chunkSizeWarningLimit: 900,
    // Tauri webviews: WebView2 (Chromium), WKWebView (Safari 15+), WebKitGTK.
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome110" : "safari15",
    minify: !process.env.TAURI_ENV_DEBUG ? "oxc" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    root: ".",
    include: ["ui/src/**/*.test.ts", "ui/tools/**/*.test.ts"],
    environment: "jsdom",
  },
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
});
