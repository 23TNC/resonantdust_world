//! On-demand cache of per-stem shadow OUTLINE sidecars — the `meta.json` `outline` (boundary polygons
//! + earcut triangulation, normalized to the content box). The tiered-lighting scatter (lighting P3)
//! looks up a caster's outline by stem; a miss fires an async fetch (`GET /textures/meta/{stem}`) and
//! caches the result, returning `null` until it lands (the caster throws no shadow that frame, then
//! does). Mirrors the old game's on-demand geometry-sidecar fetch; the Rust `resonantdust_geometry`
//! types are the source of truth for the shape.

import { metaUrl } from "./lod";

/** One silhouette piece — an outer ring, its holes, and earcut triangle indices into the flattened
 *  vertex list (`contour ++ holes[0] ++ holes[1] ++ …`). Coords are normalized to the content box
 *  (0..1; × the rendered card size to place them). Mirrors `resonantdust_geometry::Polygon`. */
export interface OutlinePolygon {
  contour: [number, number][];
  holes: [number, number][][];
  triangles: number[];
}

/** A stem's shadow-cast silhouette (the `outline` field of its `meta.json`). */
export interface Outline {
  /** Master sprite `[w, h]` px the normalized coords divide by. */
  bbox: [number, number];
  /** Disjoint silhouette pieces (usually one). */
  polygons: OutlinePolygon[];
}

/** On-demand, cached fetch of per-stem outlines for the shadow scatter. */
export class OutlineCache {
  private root = "";
  /** `undefined` = not fetched; `null` = fetched, no outline (a 404 / no caster silhouette). */
  private readonly cache = new Map<string, Outline | null>();
  private readonly pending = new Set<string>();
  /** Set when a real outline lands since the last {@link takeResolved} — lets the owner coalesce a
   *  whole startup burst of async loads into one re-dirty (re-bake the shadows now that they exist). */
  private resolved = false;

  /** Point at the world server's `/textures` root (empty until login; a repoint clears the cache). */
  setRoot(root: string): void {
    if (root === this.root) return;
    this.root = root;
    this.cache.clear();
    this.pending.clear();
  }

  /** True (once) if any outline has landed since the previous call — the owner re-dirties the shadow
   *  bake on a `true`, coalescing the whole async load burst into one re-bake per frame. */
  takeResolved(): boolean {
    const r = this.resolved;
    this.resolved = false;
    return r;
  }

  /** This stem's outline, or `null` if it has none / isn't loaded yet — a miss fires one async fetch
   *  and caches; a later frame gets the result. Never throws, never blocks. */
  get(stem: string): Outline | null {
    const hit = this.cache.get(stem);
    if (hit !== undefined) return hit;
    if (this.root && !this.pending.has(stem)) void this.fetch(stem);
    return null;
  }

  private async fetch(stem: string): Promise<void> {
    this.pending.add(stem);
    try {
      const res = await fetch(metaUrl(this.root, stem));
      if (!res.ok) {
        this.cache.set(stem, null); // 404 (or error) = this stem has no outline
        return;
      }
      const meta = (await res.json()) as { outline?: Outline };
      this.cache.set(stem, meta.outline ?? null);
      if (meta.outline) this.resolved = true; // signal a re-dirty so this caster's square re-bakes
    } catch {
      // Transient (network) — leave UNcached so a later frame retries (don't hammer, `pending` gates).
    } finally {
      this.pending.delete(stem);
    }
  }
}
