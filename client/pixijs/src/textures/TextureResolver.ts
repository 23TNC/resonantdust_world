//! The N-LOD texture resolver — the adapted, albedo-only descendant of the old
//! game's `LodTextureManager`. Every prim that names a texture resolves through it
//! to the best LOD available RIGHT NOW, and kicks the async load that upgrades it
//! toward the zoom's target LOD:
//!
//!   geo (white)  →  a low LOD (streamed / IndexedDB floor)  →  the target LOD
//!
//! - `resolve()` never blocks: it returns the best LOD on hand for the stem and
//!   fires the load for the target ({@link setTargetLod}, driven by the zoom). Each
//!   landing fires {@link onLoad} so the viewport re-bakes and picks the upgrade up.
//! - Every URL is built from the {@link TextureManifest}: the resolver requests a LOD
//!   only for a stem the manifest lists, clamped to the master's `maxSize`, with the
//!   master's content `hash` in the path — so it never speculatively 404s R2, and a
//!   hit is served `immutable`. A re-master moves the hash: the stale URL 404s and we
//!   refetch the manifest, or the poll catches it — either way stale LODs are dropped
//!   and re-fetched with the fresh hash.
//! - LODs come from the world server's `/textures/lod/<hash>/<size>/<stem>` route (the
//!   server derives + clamps to the master); the buckets persist in IndexedDB keyed by
//!   hash ({@link getLod}), so a returning player has a floor with zero network.
//! - "geo" here is the flat white sprite (tinted by the prim), the placeholder until
//!   any real LOD lands.
//!
//! It composes, not owns: each LOD size packs into its own {@link LodPool}; loads are
//! deduped so a `(stem, size)` is fetched at most once.

import { ImageSource, Rectangle, Texture, type Renderer } from "pixi.js";
import { LodPool } from "./LodPool";
import { getLod, putLod } from "./previewCache";
import { LOD_SIZES, lodUrl, pickLodForSize, type TexMap } from "./lod";
import { TextureManifest } from "./textureManifest";

/** The index key for one (stem, map) — the packed/hash/pending maps carry every map of a
 *  stem side by side (albedo|normal|depth|…), so the key is `stem|map`. */
const mapKey = (stem: string, map: TexMap): string => `${stem}|${map}`;
/** Recover the stem from a {@link mapKey} (for a manifest lookup keyed by bare stem). */
const stemOf = (key: string): string => key.slice(0, key.lastIndexOf("|"));

/** A resolve result: the texture to bake, and whether it's the GEO tier (the white
 *  fill) rather than real pixels. The caller tints geo with the prim's silhouette
 *  colour and a loaded LOD with the prim's texture tint. */
export interface ResolvedTexture {
  texture: Texture;
  geo: boolean;
}

/** LOD-pool occupancy for the debug HUD: total atlas pages, the packed-texture
 *  count per LOD size, and the preview (floor) tier's live size + count. The
 *  preview size is reported (not hard-coded in the HUD) so it tracks {@link FLOOR_LOD}
 *  if the floor ever moves. */
export interface LodStats {
  /** Atlas pages summed across every LOD pool. */
  pages: number;
  /** LOD size (px) → textures packed at that size. */
  counts: ReadonlyMap<number, number>;
  /** The preview/floor LOD size in px (the fast placeholder tier). */
  previewSize: number;
  /** Textures packed at the preview size. */
  previewCount: number;
}

/** One tile at native scale — the target LOD floor (zoom 1 → 64px). */
const BASE_LOD_PX = 64;

/** A cheap low LOD fetched alongside the target while a stem has nothing on hand, so
 *  a fast placeholder lands before the (larger) target — geo → floor → target. */
const FLOOR_LOD = 32;

/** LODs at or below this pack into a 1024² page; larger into 2048². */
const SMALL_LOD_MAX = 64;

/** The reserved stem of the built-in white fill. Baked into the preview pool at
 *  boot (never fetched from the server) and short-circuited by the caller — a prim
 *  naming "white" resolves to a flat tint-rect, so `resolve` is never asked for it. */
const WHITE_STEM = "white";

export class TextureResolver {
  private readonly renderer: Renderer;
  /** The geo fallback: the all-purpose white fill, tinted by the caller's prim.
   *  Baked at the preview (floor) size and packed into the preview pool, so it
   *  shares the preview textures' page rather than owning an atlas of its own. */
  readonly white: Texture;
  /** The texture tree root the URL builders hang off (the world server's
   *  `/textures`). Empty at boot (the server is discovered at login); loads no-op
   *  while empty. */
  private root: string;

  /** One packing pool per LOD size present (shared across maps — a 64px albedo and a
   *  64px normal pack into the same page, keyed apart by {@link mapKey}). */
  private readonly pools = new Map<number, LodPool>();
  /** `stem|map` → its loaded LODs, keyed by size. */
  private readonly packed = new Map<string, Map<number, Texture>>();
  /** The authoritative LOD index — the resolver builds every URL from it and stays on
   *  geo for a stem it doesn't list (no speculative request). */
  private readonly manifest = new TextureManifest();
  /** `stem|map` → the content hash its packed LODs were loaded at; a differing manifest
   *  hash (a re-master) marks them stale. */
  private readonly packedHash = new Map<string, string>();
  /** In-flight loads, deduped so a `(stem, map, size)` is fetched at most once. */
  private readonly pending = new Set<string>();
  /** Linked-atlas cell sub-frames, keyed first by the packed atlas texture they view into
   *  (a {@link WeakMap} — an entry dies with its base texture, so a re-master that drops the
   *  packed LODs drops these too) then by cell index. Lets a bake reuse a cell's UV sub-frame
   *  instead of allocating one each time. */
  private readonly cellFrames = new WeakMap<Texture, Map<number, Texture>>();

  /** Target on-screen px for one tile (`64 × zoom`) — the LOD the resolver aims for. */
  private targetPx = BASE_LOD_PX;

  private readonly listeners = new Set<() => void>();

  constructor(renderer: Renderer, texturesRoot: string) {
    this.renderer = renderer;
    this.root = texturesRoot;
    (globalThis as any).__resolver = this;
    // Bake the white fill into the preview pool at the preview size, so the geo
    // placeholder lives beside the preview LODs it stands in for (no dedicated
    // atlas page). Tinted per-prim by the caller.
    const white = this.poolFor(FLOOR_LOD).add(WHITE_STEM, Texture.WHITE, FLOOR_LOD, FLOOR_LOD);
    if (!white) throw new Error("TextureResolver: failed to pack the white fill");
    this.white = white;
    // A manifest change (re-master / new LOD) drops stale-hash LODs and re-bakes so
    // the next resolve re-fetches with the fresh hash.
    this.manifest.onChange(() => this.onManifestChange());
  }

  /** Repoint the texture root — called once the world server is known (at login).
   *  Also fetches + polls the manifest from that root. */
  setRoot(texturesRoot: string): void {
    this.root = texturesRoot;
    this.manifest.setRoot(texturesRoot);
  }

  /** Set the target LOD from the viewport's on-screen tile size (`64 × zoom`). The
   *  next `resolve` aims loads at the matching bucket; a landing re-bakes in place. */
  setTargetLod(px: number): void {
    this.targetPx = Math.max(LOD_SIZES[0], px);
  }

  /** Subscribe to "a LOD landed" — the viewport re-bakes so prims pick up the
   *  upgrade. Returns an unsubscribe. */
  onLoad(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private emit(): void {
    for (const fn of this.listeners) fn();
  }

  /** Per-LOD atlas occupancy for the debug HUD (pages + packed count per size,
   *  plus the preview/floor tier). A size with no pool simply isn't in `counts`
   *  (the HUD reads it as 0). Cheap — a walk over the handful of live pools. */
  lodStats(): LodStats {
    const counts = new Map<number, number>();
    let pages = 0;
    for (const [size, pool] of this.pools) {
      counts.set(size, pool.count);
      pages += pool.pageCount;
    }
    return {
      pages,
      counts,
      previewSize: FLOOR_LOD,
      previewCount: this.pools.get(FLOOR_LOD)?.count ?? 0,
    };
  }

  /** Best texture for `stem`'s `map` (albedo|normal|depth|…) available now (plus its
   *  tier), kicking the load that upgrades it toward the target LOD. A falsy `stem` (an
   *  untextured tint-rect prim), or a `map` the manifest says this stem lacks, resolves
   *  straight to geo — the caller applies that channel's flat fallback. Cheap enough to
   *  call every bake: the hot path is a couple of map lookups. */
  resolve(stem: string | undefined, map: TexMap = "albedo", cell?: number): ResolvedTexture {
    if (!stem || !this.root) return { texture: this.white, geo: true };

    // Only stems the manifest lists are requestable — an unlisted stem stays on geo
    // (never a speculative fetch). The row gives the ceiling + the URL's hash.
    const entry = this.manifest.entry(stem);
    if (!entry) return { texture: this.white, geo: true };
    // Skip a map this stem has no leaf for (most art has no normal/depth) — geo, no fetch.
    if (map !== "albedo" && !entry.maps.includes(map)) return { texture: this.white, geo: true };

    const key = mapKey(stem, map);
    const cap = entry.maxSize;
    // A LINKED atlas packs cols×rows cells into ONE texture: to render a cell at ~targetPx
    // the whole atlas must be fetched at targetPx × the grid's larger axis. Scale the target
    // up so a cell resolves crisp rather than 1/cols of the on-screen size.
    const gridFactor = entry.grid ? Math.max(entry.grid[0], entry.grid[1]) : 1;
    let desired = pickLodForSize(Math.min(this.targetPx * gridFactor, cap));
    if (desired > cap) desired = LOD_SIZES.filter((s) => s <= cap).pop() ?? LOD_SIZES[0];
    const loaded = this.packed.get(key);

    // Fire the upgrade toward the target LOD (deduped inside ensureLod) …
    if (!loaded?.has(desired)) void this.ensureLod(stem, desired, entry.hash, map);
    // … plus a cheap floor while nothing is on hand yet, so a placeholder lands before
    // the target does (geo → floor → target).
    if (!(loaded && loaded.size > 0) && desired > FLOOR_LOD && FLOOR_LOD <= cap)
      void this.ensureLod(stem, FLOOR_LOD, entry.hash, map);

    if (loaded && loaded.size > 0) {
      const best = this.bestLoaded(loaded, desired);
      if (best) {
        // A gridded stem with a chosen cell bakes a UV sub-frame of the atlas, not the whole
        // sheet; an ordinary stem (or a linked prim with no cell yet) bakes the texture as-is.
        if (entry.grid && cell != null) return { texture: this.cellFrame(best, cell, entry.grid, entry.pad), geo: false };
        return { texture: best, geo: false };
      }
    }
    return { texture: this.white, geo: true };
  }

  /** A cached UV sub-frame of a packed linked-atlas texture for `cell` (row-major, 0-based),
   *  trimmed by the manifest inset `pad` so a downscale can't bleed a neighbour cell across a
   *  cell edge. `base.frame` is the atlas's region on its pool page; the sub-frame narrows it
   *  to the cell's inset rect. Cached per (base texture, cell) via {@link cellFrames}. */
  private cellFrame(base: Texture, cell: number, grid: [number, number], pad?: [number, number]): Texture {
    let byCell = this.cellFrames.get(base);
    if (!byCell) this.cellFrames.set(base, (byCell = new Map()));
    const hit = byCell.get(cell);
    if (hit) return hit;

    const [cols, rows] = grid;
    const [pu, pv] = pad ?? [0, 0];
    const cx = cell % cols;
    const cy = Math.floor(cell / cols) % rows;
    // The cell's inset UV rect within the whole atlas [0,1] …
    const u0 = cx / cols + pu;
    const v0 = cy / rows + pv;
    const uw = 1 / cols - 2 * pu;
    const vh = 1 / rows - 2 * pv;
    // … projected onto the atlas's frame on its pool page.
    const f = base.frame;
    const rect = new Rectangle(f.x + u0 * f.width, f.y + v0 * f.height, uw * f.width, vh * f.height);
    const t = new Texture({ source: base.source, frame: rect });
    byCell.set(cell, t);
    return t;
  }

  /** A manifest change (re-master / new LOD): drop any packed LODs whose hash no
   *  longer matches the manifest (stale bytes), then re-bake so `resolve` re-fetches
   *  them with the fresh hash. */
  private onManifestChange(): void {
    for (const key of [...this.packed.keys()]) {
      const e = this.manifest.entry(stemOf(key));
      if (!e || this.packedHash.get(key) !== e.hash) {
        this.packed.delete(key);
        this.packedHash.delete(key);
      }
    }
    this.emit();
  }

  /** The best-fit loaded LOD for `desired`: the largest bucket at or below it, or —
   *  when none is loaded yet at that scale — the smallest loaded bucket above it
   *  (still real pixels, better than geo). */
  private bestLoaded(loaded: Map<number, Texture>, desired: number): Texture | null {
    let below = -1;
    let above = Infinity;
    for (const size of loaded.keys()) {
      if (size <= desired) {
        if (size > below) below = size;
      } else if (size < above) above = size;
    }
    const pick = below >= 0 ? below : above < Infinity ? above : -1;
    return pick >= 0 ? loaded.get(pick) ?? null : null;
  }

  // ── loads ───────────────────────────────────────────────────────────────────

  /** Fetch + pack one LOD (idempotent, deduped). The URL is content-addressed by
   *  `hash`, so a matching IndexedDB entry (keyed by the same hash) is served with NO
   *  network; otherwise it fetches fresh and re-caches under the current hash. A `404`
   *  is a stale hash (a re-master the manifest hasn't caught) — it refetches the
   *  manifest so the retry uses the fresh hash. Fires {@link onLoad} once packed. */
  private async ensureLod(stem: string, size: number, hash: string, map: TexMap): Promise<void> {
    const key = mapKey(stem, map);
    const pkey = `${key}@${size}`;
    if (this.pending.has(pkey) || this.packed.get(key)?.has(size)) return;
    this.pending.add(pkey);
    try {
      const cached = await getLod(stem, size, map);
      let bytes: ArrayBuffer;
      if (cached && cached.v === hash) {
        bytes = cached.bytes; // hash-addressed: the cached bytes are current
      } else {
        const res = await fetch(lodUrl(this.root, stem, hash, size, map));
        if (res.status === 404) {
          this.manifest.refresh(); // stale hash — refetch the manifest, retry with the new one
          return;
        }
        if (!res.ok) throw new Error(`lod fetch ${res.status}: ${lodUrl(this.root, stem, hash, size, map)}`);
        bytes = await res.arrayBuffer();
        void putLod(stem, size, map, { v: hash, bytes });
      }

      // `albedo` (residual base), `layers` (weights), `normal` (vector) and `surface`
      // (height/ao/coverage) are all DATA maps — their RGB must survive intact. Load them with
      // STRAIGHT alpha so premultiplying (the default) can't scale their RGB by a non-opaque
      // alpha (e.g. legacy AO packed in normal.α) and corrupt the reconstruction / vector /
      // coverage. Nothing reads their alpha anymore (the silhouette lives in surface.B). Only
      // true colour maps (emissive) stay premultiplied.
      const raw = map === "layers" || map === "albedo" || map === "normal" || map === "surface";
      const bmp = await createImageBitmap(new Blob([bytes]), raw ? { premultiplyAlpha: "none" } : {});
      // The server clamps to the master; pack under the size we actually got.
      const actualShort = Math.min(bmp.width, bmp.height);
      const achieved = Math.min(size, actualShort);
      if (this.packInto(key, achieved, bmp, raw)) {
        this.packedHash.set(key, hash);
        this.emit();
      }
    } catch {
      /* LOD unavailable — stay on the current tier */
    } finally {
      this.pending.delete(pkey);
    }
  }

  /** Pack a decoded bitmap into `size`'s pool and index it under `(key, size)` where `key`
   *  is the `stem|map` id. Returns whether it packed (a whole-page overflow drops it).
   *  Drops the bitmap-backed source afterwards (the atlas kept its own copy). */
  private packInto(key: string, size: number, bmp: ImageBitmap, raw = false): boolean {
    const pool = this.poolFor(size);
    // Straight-alpha (weight map): set `no-premultiply-alpha` AT SOURCE CONSTRUCTION, before
    // the GPU upload — a `Texture.from(...)` then alphaMode set is too late (upload already
    // premultiplied it). The atlas blit copies it verbatim (blendMode none) so the RGB
    // weights survive even where the alpha (4th material channel) is 0.
    const src = raw
      ? new Texture({ source: new ImageSource({ resource: bmp, alphaMode: "no-premultiply-alpha" }) })
      : Texture.from(bmp);
    let packed: Texture | null;
    try {
      packed = pool.add(key, src, bmp.width, bmp.height);
    } finally {
      src.destroy(true);
    }
    if (!packed) return false;
    let byKey = this.packed.get(key);
    if (!byKey) this.packed.set(key, (byKey = new Map()));
    byKey.set(size, packed);
    return true;
  }

  /** The pool for LOD `size`, created empty on first use. Small buckets pack into a
   *  1024² page, larger into 2048² — same page sizing whether the pool is spun up
   *  lazily by a load or eagerly for the boot-time white fill. */
  private poolFor(size: number): LodPool {
    let pool = this.pools.get(size);
    if (!pool) {
      pool = new LodPool(this.renderer, size <= SMALL_LOD_MAX ? 1024 : 2048);
      this.pools.set(size, pool);
    }
    return pool;
  }
}
