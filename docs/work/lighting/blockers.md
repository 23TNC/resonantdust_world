# Blockers — lighting

_Dependencies gating a `todo` item. Each: what's blocked, why, the plan to clear. Move the item back to
`todo` once cleared (dated resolution line here)._

---

## B1 · Outline sidecar not yet served to the client (blocks P3 scatter) — 2026-07-18

**Server half ✅ DONE 2026-07-18.** The edge serves the sidecar on demand: `GET /textures/meta/{*stem}`
→ the leaf's `meta.json` as `application/json` (`TextureSource::serve_meta`, `TextureTier::Meta`).
Verified: `…/conifer/default/e` → the 179-triangle outline JSON. **Chosen on-demand** (not manifest-fold)
because outlines are large — mirrors the old game's `/textures/geo/` sidecars. **Client half ✅ DONE:**
`textures/OutlineCache.ts` — `get(stem)` returns the cached `Outline` (polygons + earcut tris, decoded
from the sidecar) or `null` on a miss, firing one async `metaUrl` fetch + caching. Ready for P3's scatter
to look up a caster's silhouette; P3 wires `setRoot(texturesRoot)` on login + feeds it the caster stems.
**B1 fully resolved** — P3 unblocked.

**Blocked.** _(historical)_ P3's projected-silhouette scatter needs each caster's `outline` (earcut tris)
**client-side**; today it's only in the per-leaf `meta.json` on disk.

**Why.** `bin/art` now writes `meta.json` (art-metadata P1/P2), but nothing folds it into the content
manifest the client fetches. `atlas.json` is already folded server-side (edge/content path) — `meta.json`
presumably rides the same mechanism, but it's unverified.

**Plan to clear.** art-metadata **P3**: confirm/extend the manifest fold to include `meta.json`
(`outline` + `channel_tints`); client decodes the `Sidecar` (the `resonantdust_geometry` types are
`Deserialize`, so the wasm client can reuse them). Small, but it gates the whole scatter — do it in
lighting **P0**.

## B2 · No static-light content source (soft — blocks the cold tier's payoff, not the code) — 2026-07-18

**Partially blocked.** The cold tier exists to amortize **many static lights**, but content authors none
yet (the DSL has no point-light primitive).

**Why.** "Dense many-lights" needs authored cold lights (torches, glows) classified to the cold tier at
worldgen/load. Without them, the `cold_lightmap` bake is real but has nothing to bake.

**Plan to clear.** Not a hard blocker for the *plumbing* — P1's bake + textures can be built and verified
against a **hardcoded / debug** cold light. The DSL point-light primitive + worldgen classification is
**P5**; do it once the pipeline is proven so we're authoring into a working system.
