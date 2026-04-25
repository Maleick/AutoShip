import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

// Read the textquest crate version from the workspace Cargo.toml at build time
// so the web UI never drifts from the Rust crate version (see issue #2511).
function readCrateVersion(): string {
  const cargoPath = resolve(__dirname, "../../textquest/Cargo.toml");
  const cargoToml = readFileSync(cargoPath, "utf8");
  // textquest/Cargo.toml starts with its own [package] table, so the first
  // `version = "..."` line is the crate version.
  const match = cargoToml.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error(
      `Could not extract version from ${cargoPath}. ` + `Expected a line like: version = "x.y.z"`,
    );
  }
  return match[1];
}

const crateVersion = readCrateVersion();

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __APP_VERSION__: JSON.stringify(`v${crateVersion}`),
  },
  server: {
    port: 5173,
    proxy: {
      // Proxy API requests to the textquest-web Axum backend
      "/api": {
        target: "http://localhost:3001",
        changeOrigin: true,
      },
      "/ws": {
        target: "ws://localhost:3001",
        ws: true,
      },
    },
  },
  build: {
    outDir: "dist",
    sourcemap: true,
  },
});
