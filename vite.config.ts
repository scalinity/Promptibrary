import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Tauri expects a fixed dev server port and ignores changes to it.
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

// When VITE_E2E_MODE=true, swap the real IPC layer for the deterministic
// fixture mock so Playwright visual regressions render without Tauri.
// Production builds and `pnpm tauri dev` leave the alias unset and use the
// real Tauri invoke. A console.info in the mock entrypoint makes a stray
// flag obvious if someone ships a build with it set.
const ipcAlias: Record<string, string> =
  process.env.VITE_E2E_MODE === "true"
    ? {
        "@/shared/api/ipc": path.resolve(
          __dirname,
          "./src/shared/api/ipc.mock.ts",
        ),
      }
    : {};

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      ...ipcAlias,
      "@": path.resolve(__dirname, "./src"),
    },
  },
  // prevent Vite from obscuring Rust errors
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: TAURI_DEV_HOST ?? false,
    hmr: TAURI_DEV_HOST
      ? { protocol: "ws", host: TAURI_DEV_HOST, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari16",
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
