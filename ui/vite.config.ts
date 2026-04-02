/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    exclude: ["e2e/**", "node_modules/**"],
  },
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:8080",
      },
      "/ws": {
        target: "http://localhost:8080",
        ws: true,
      },
      "/oauth": {
        target: "http://localhost:9099",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/oauth/, ""),
      },
    },
  },
});
