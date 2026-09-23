import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig(() => {
  const target = process.env.GROVE_DEV_API ?? "http://127.0.0.1:8080";
  const key = process.env.GROVE_DEV_TLS_KEY;
  const cert = process.env.GROVE_DEV_TLS_CERT;
  if (Boolean(key) !== Boolean(cert))
    throw new Error("Both TLS key and certificate are required");
  return {
    base: "/api/admin/console/",
    plugins: [react()],
    server: {
      host: "127.0.0.1",
      port: 5173,
      strictPort: true,
      https:
        key && cert
          ? { key: readFileSync(key), cert: readFileSync(cert) }
          : undefined,
      proxy: Object.fromEntries(
        ["/api/admin/v1", "/readyz"].map((path) => [
          path,
          { target, changeOrigin: false },
        ]),
      ),
    },
  };
});
