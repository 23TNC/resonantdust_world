import { defineConfig } from "vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Repo root (two up from client/webgl/), so we can reach the wasm bundle (shared/pkg) and the DSL content
// corpus (content/) that live outside this view — same seams as client/pixijs.
const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const sharedPkg = fileURLToPath(new URL("../../shared/pkg", import.meta.url));
const contentDir = fileURLToPath(new URL("../../content", import.meta.url));

// Build fingerprints (bin/versions at repo root), injected as a compile-time constant for the debug panel's
// Versions tab. Best-effort — a missing snapshot degrades to null, never a build break.
let buildVersions: unknown = null;
try {
  buildVersions = JSON.parse(readFileSync(fileURLToPath(new URL("../../versions.json", import.meta.url)), "utf8"));
} catch {
  buildVersions = null;
}

export default defineConfig({
  define: {
    __BUILD_VERSIONS__: JSON.stringify(buildVersions),
  },
  resolve: {
    alias: {
      "@shared": sharedPkg,
      "@content": contentDir,
    },
  },
  server: {
    // 5174 so the webgl client runs ALONGSIDE the pixijs client (5173) during the migration.
    port: 5174,
    fs: {
      allow: [repoRoot],
    },
  },
  build: {
    sourcemap: false,
  },
});
