//! Texture URL seam (formerly `lod.ts` — renamed by one-resolution P4 when the atlas
//! ladder died) — the client fetches each stem's maps from the world SERVER's `/textures`
//! routes at ONE size (the manifest's max) and packs whatever bytes come back. The server
//! keeps serving derived lower sizes for anyone who asks; this client no longer does. The
//! `/lod/` path segment below is the SERVER'S route name — a wire literal, not a client
//! concept.

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

/** Zoom bounds (screen px per world px). 1 = tiles at their native `SQUARE` (128) px, 1:1 with
 *  the art; the range spans the three PARTITION LEVELS — level 0 covers `[1, 2)`, level 2 bottoms
 *  out at 0.25. Zoom past 1 MAGNIFIES (a slot cannot show more than `SQUARE` texels).
 *
 *  0.25, not 0.125: level 3 would put 192×128 tiles ≈ 225 zones in the window, which streams in
 *  over a second or two ([B-1](../../../docs/work/2026-07-26-textile-slot/blockers.md)); restoring
 *  it needs only a loading-priority system. (Housed here since the ladder days; the partition
 *  ladder in `squareMath` is the real owner — moved next time the seam is touched.) */
export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 2;

// one-resolution (2026-08-02): the LADDER helpers are DELETED — `lodTier`, `lodArtPx`,
// `LOD_SIZES`, `pickLodForSize`, plus the orphaned `realUrl`/`previewUrl`/`metaUrl`
// (zero consumers). One URL per (stem, map) at the manifest's max size; git is history.

/** A stem's `map` URL at `size` px (the manifest's `maxSize` — the one size this client asks
 *  for; the `/lod/` path segment is the SERVER's route name). The `map` segment precedes the
 *  catch-all `stem`. The `hash` (from the manifest) is a path segment, so the URL is
 *  content-addressed: a re-master changes it, a stale URL 404s (→ manifest refetch), and a
 *  hit is `immutable`-cacheable. */
export function texUrl(root: string, stem: string, hash: string, size: number, map: TexMap = "albedo"): string {
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
