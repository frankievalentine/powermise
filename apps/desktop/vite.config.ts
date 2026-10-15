import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// `--mode demo` builds the browser-stub UI into the marketing site, where it
// runs inside an iframe as the live demo. Tauri builds use the default mode.
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  base: mode === "demo" ? "./" : "/",
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build:
    mode === "demo"
      ? { target: "es2022", outDir: "../web/public/demo", emptyOutDir: true }
      : { target: "safari16", outDir: "dist" },
}));
