//! Texture URL seam — the client fetches a stem's tiers from the world SERVER's
//! `/textures` routes and packs whatever bytes come back. The server owns the
//! module math: it resolves each stem to a master albedo and DERIVES the lower
//! resolutions on demand (see `server/src/textures.rs`), so `bin/art` writes only
//! masters and the client never downscales.
//!
//! Two tiers, both keyed by stem (`<category>/<kind>`):
//!   • `master`  — the full-res master albedo (the resolver's top tier).
//!   • `preview` — the gate's half-res derivation (the geo→real placeholder).

/** The texture tree's root URL: the world server's `/textures` routes, the same
 *  origin the client fetches `/content` from. Pass the server's HTTP base
 *  (`client.assetBase()`, derived from the login-discovered server URL) so
 *  textures and content travel the one route. */
export function texturesRoot(serverBase: string): string {
  return `${serverBase.replace(/\/$/, "")}/textures`;
}

/** The texture MAPS a stem can carry, one PNG per map in the same variant leaf. `albedo`
 *  (colour) is mandatory and the default; `normal` (tangent-space normal) and `depth`
 *  (height) are optional — a stem lacking one stays on that channel's flat fallback.
 *  `albedo` is the residual reconstruction BASE (RGB); `layers` (per-material weight, RGB)
 *  adds the material tints on top; `surface` (R=height, G=ao, B=coverage) is the one silhouette
 *  source. `split_layers` emits albedo(residual)+layers; a stem lacking `layers` renders its
 *  albedo unchanged. Mirrors the server's `textures::MAPS` allowlist. */
export type TexMap = "albedo" | "normal" | "depth" | "emissive" | "layers" | "surface";

/** Zoom bounds (screen px per world px). 1 = tiles at their native `SQUARE` (128) px, i.e. 1:1 with
 *  the art. The range spans the three-level lod ladder — lod 0 covers `[1, 2)`, lod 2 bottoms out at
 *  0.25. Zoom-in past 1 MAGNIFIES: 128 px is the maximum art size by design (a slot cannot show more),
 *  so the top of each lod band upscales by up to 2× rather than fetching art that does not exist
 *  (work `2026-07-26-textile-slot` F2).
 *
 *  0.25, not 0.125: lod 3 would put 192×128 tiles ≈ 225 zones in the window, which streams in over a
 *  second or two ([B-1](../../../docs/work/2026-07-26-textile-slot/blockers.md)). The `u2` lod field
 *  still has room for lod 3, so restoring it needs no layout change — only a loading-priority system. */
export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 2;

/** The power-of-two LOD tier a zoom sits in — the master LOD the resolver targets
 *  (`64 × tier` px). Rounded UP so a magnified square is super-sampled, not upscaled. */
export function lodTier(zoom: number): number {
  const t = Math.pow(2, Math.ceil(Math.log2(zoom)));
  return Math.min(Math.max(t, ZOOM_MIN), ZOOM_MAX);
}

/** The ART size to fetch for a tile at `zoom` — the pow2 bucket at/above its on-screen size, capped
 *  at `square` (= `SQUARE`, the maximum art size). NOTE this no longer sizes a composite slot: on the
 *  fixed grid the slot is CONSTANT at `SQUARE` texels and what varies is TILES PER SLOT. Kept as the
 *  art-ladder query only. */
export function lodArtPx(zoom: number, square: number): number {
  return Math.min(square, pickLodForSize(square * zoom));
}

/** The full-res master URL for a stem under a texture `root` ({@link texturesRoot}) —
 *  the gate resolves it to the canonical master albedo. */
export function realUrl(root: string, stem: string): string {
  return `${root}/master/${stem}`;
}

/** The half-res preview URL for a stem — the gate derives it from the master. */
export function previewUrl(root: string, stem: string): string {
  return `${root}/preview/${stem}`;
}

/** The `meta.json` sidecar URL for a stem — channel tints + the shadow-cast silhouette outline
 *  (`GET /textures/meta/{stem}`, JSON), fetched on demand by the lighting scatter. */
export function metaUrl(root: string, stem: string): string {
  return `${root}/meta/${stem}`;
}

/** The LOD buckets the resolver requests, in px (short axis). A stem's texture is
 *  fetched at the smallest bucket ≥ its on-screen size, clamped to the master. */
export const LOD_SIZES = [16, 32, 64, 128, 256, 512, 1024] as const;

/** The smallest LOD bucket that covers `px` on-screen (so a texel never stretches
 *  past a screen px), capped at the largest bucket. */
export function pickLodForSize(px: number): number {
  for (const s of LOD_SIZES) if (s >= px) return s;
  return LOD_SIZES[LOD_SIZES.length - 1];
}

/** One LOD's URL for a stem's `map` (albedo|normal|depth|emissive) — the gate derives it
 *  from that map's master (short axis `size` px), clamped to the master's own size. The
 *  `map` segment precedes the catch-all `stem`. The `hash` (from the manifest) is a path
 *  segment, so the URL is content-addressed: a re-master changes it, a stale URL 404s (→
 *  manifest refetch), and a hit is `immutable`-cacheable. */
export function lodUrl(root: string, stem: string, hash: string, size: number, map: TexMap = "albedo"): string {
  return `${root}/lod/${hash}/${size}/${map}/${stem}`;
}

/** The texture-manifest URL (`…/textures` → `…/textures-manifest`) — the authoritative
 *  index of what LODs exist, the resolver builds every URL from it. */
export function manifestUrl(root: string): string {
  return `${root}-manifest`;
}

/** The cheap manifest-version fingerprint the client polls for changes. */
export function manifestVersionUrl(root: string): string {
  return `${root}-manifest-version`;
}
