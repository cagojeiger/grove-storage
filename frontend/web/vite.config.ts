import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { consoleHeaders } from "./security-headers.mjs";

export default defineConfig(() => {
  const target = process.env.GROVE_DEV_API ?? "http://127.0.0.1:8080";
  const key = process.env.GROVE_DEV_TLS_KEY;
  const cert = process.env.GROVE_DEV_TLS_CERT;
  if (Boolean(key) !== Boolean(cert))
    throw new Error("Both TLS key and certificate are required");
  return {
    base: "/api/admin/console/",
    plugins: [react()],
    build: {
      rollupOptions: {
        output: { manualChunks: (id) => id.includes("node_modules") ? "vendor" : undefined },
      },
    },
    server: {
      host: "127.0.0.1",
      port: 5173,
      strictPort: true,
      cors: false,
      // Vite injects React refresh scripts/styles and opens an HMR websocket.
      headers: consoleHeaders(true),
      https:
        key && cert
          ? { key: readFileSync(key), cert: readFileSync(cert) }
          : undefined,
      proxy: Object.fromEntries(
        ["/api/admin/identity/v1", "/api/admin/console-commands/v1", "/readyz"].map((path) => [
          path,
          { target, changeOrigin: false },
        ]),
      ),
    },
  };
});
