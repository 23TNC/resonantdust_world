import { defineConfig } from "vite";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Repo root (one up from pixijs/), so we can reach the wasm bundle (shared/pkg)
// and the DSL content corpus (content/) that live outside this view.
const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const sharedPkg = fileURLToPath(new URL("../shared/pkg", import.meta.url));
const contentDir = fileURLToPath(new URL("../content", import.meta.url));

// Build fingerprints (bin/versions output at repo root). Injected as a compile-
// time constant so the bundle carries the source-closure hashes it was built
// against; the debug panel's Versions tab compares these to the gate's live
// /versions to flag a stale client/gate/shard. Best-effort — a missing snapshot
// (fresh checkout before first `bin/versions`) degrades to null, never a build
// break.
let buildVersions: unknown = null;
try {
  const p = fileURLToPath(new URL("../versions.json", import.meta.url));
  buildVersions = JSON.parse(readFileSync(p, "utf8"));
} catch {
  buildVersions = null;
}

export default defineConfig({
  define: {
    __BUILD_VERSIONS__: JSON.stringify(buildVersions),
  },
  resolve: {
    alias: {
      // The Rust→wasm client bundle (`bin/rd build shared`) and the DSL content
      // corpus, both outside the pixijs root — imported via stable aliases.
      "@shared": sharedPkg,
      "@content": contentDir,
    },
  },
  server: {
    port: 5173,
    fs: {
      // Allow serving the wasm bundle + content from the repo root (outside the
      // pixijs project dir) in dev.
      allow: [repoRoot],
    },
  },
  build: {
    sourcemap: false,
  },
});
