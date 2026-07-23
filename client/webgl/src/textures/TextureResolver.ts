//! The N-LOD texture resolver (webgl port) — every prim that names a texture resolves through it to
//! the best LOD available RIGHT NOW, and kicks the async load that upgrades it toward the zoom's
//! target LOD:
//!
//!   geo (white)  →  a low LOD (streamed / IndexedDB floor)  →  the target LOD
//!
//! Ported from the pixijs resolver: the tier logic, manifest, hash-addressed IndexedDB cache, dedupe
//! and LOD math are already Pixi-free and carry over verbatim. Only the GL seams change — a packed LOD
//! is now a {@link TexFrame} (a UV sub-frame onto a shared atlas page) instead of a Pixi `Texture`,
//! `packInto` uploads the `ImageBitmap` into an engine {@link Texture} (premultiply per-map), and
//! `cellFrame` narrows a `TexFrame` rather than a `Rectangle`.
//!
//! GL is LAZY: the resolver is built at boot with no renderer (the viewport's GL context doesn't exist
//! until the world scene), so {@link attachRenderer} wires the {@link Blitter} + pools when the
//! viewport comes up. Before then `resolve` returns geo (nothing is packed yet). Geo returns a null
//! frame — the caller supplies its own white fill (unlike pixijs, which packs a shared white stem).

import { Blitter, type Renderer, Texture, TexFrame } from "../gl";
import { LodPool } from "./LodPool";
import { getLod, putLod } from "./previewCache";
import { LOD_SIZES, lodUrl, pickLodForSize, type TexMap } from "./lod";
import { TextureManifest } from "./textureManifest";

/** The index key for one (stem, map). */
const mapKey = (stem: string, map: TexMap): string => `${stem}|${map}`;
const stemOf = (key: string): string => key.slice(0, key.lastIndexOf("|"));

/** A resolve result: the atlas sub-frame to bake (null for the GEO tier — the caller uses its own
 *  white fill + the prim's silhouette colour), and whether it's geo rather than real pixels. */
export interface ResolvedTexture {
  frame: TexFrame | null;
  geo: boolean;
}

/** LOD-pool occupancy for the debug HUD. */
export interface LodStats {
  pages: number;
  counts: ReadonlyMap<number, number>;
  previewSize: number;
  previewCount: number;
}

/** One tile at native scale — the target LOD floor (zoom 1 → 64px). */
const BASE_LOD_PX = 64;
/** A cheap low LOD fetched alongside the target while a stem has nothing on hand. */
const FLOOR_LOD = 32;
/** LODs at or below this pack into a 1024² page; larger into 2048². */
const SMALL_LOD_MAX = 64;

export class TextureResolver {
  private renderer: Renderer | null;
  private blitter: Blitter | null = null;
  private root: string;

  /** One packing pool per LOD size present (shared across maps — keyed apart by {@link mapKey}). */
  private readonly pools = new Map<number, LodPool>();
  /** ONE shared pool for every SURFACE frame across ALL lod sizes. The shadow gather binds a single
   *  surface page while prims hold defs at MIXED lods (immutable per-lod defs), so every lod's
   *  silhouette frame must live on the same page — per-size pools put each lod on its own page and
   *  every mid-session lod change went off-page (solid quads). Mixed pow2 ≥16 frames keep the
   *  16-px def alignment (MaxRects coordinates are sums of inserted extents, all multiples of 16).
   *  2048² ≈ a thousand frames; a spill hits the off-page warn (the C5 texture array lifts this). */
  private surfacePool: LodPool | null = null;
  /** `stem|map` → its loaded LODs, keyed by size. */
  private readonly packed = new Map<string, Map<number, TexFrame>>();
  private readonly manifest = new TextureManifest();
  private readonly packedHash = new Map<string, string>();
  private readonly pending = new Set<string>();
  /** Cached linked-atlas cell sub-frames, keyed by the packed frame then cell index. */
  private readonly cellFrames = new WeakMap<TexFrame, Map<number, TexFrame>>();
  /** stem → its sprite silhouette's opaque bbox (frame fractions, POST-ingest-transform),
   *  computed once on surface decode. */
  private readonly spriteBBox = new Map<string, { fx: number; fy: number; fw: number; fh: number }>();
  /** stem → the RAW (pre-scale) surface bbox fractions — drives the ingest re-centring. */
  private readonly rawBBox = new Map<string, { fx: number; fy: number; fw: number; fh: number }>();
  /** stem → pre-atlas sprite scale (def-frame-anchors P5): applied at pack — scaled, clipped
   *  to the same pow2 frame, transparent-filled, re-centred on the surface presence. */
  private readonly spriteScale = new Map<string, [number, number]>();
  private scaleWarned = false;

  private targetPx = BASE_LOD_PX;
  private readonly listeners = new Set<() => void>();

  constructor(renderer: Renderer | null, texturesRoot: string) {
    this.renderer = renderer;
    this.root = texturesRoot;
    if (renderer) this.blitter = new Blitter(renderer);
    this.manifest.onChange(() => this.onManifestChange());
  }

  /** Attach the viewport's GL context once it exists (F6: the viewport self-canvases). Idempotent. */
  attachRenderer(renderer: Renderer): void {
    if (this.renderer) return;
    this.renderer = renderer;
    this.blitter = new Blitter(renderer);
  }

  /** Repoint the texture root at login + fetch/poll the manifest. */
  setRoot(texturesRoot: string): void {
    this.root = texturesRoot;
    this.manifest.setRoot(texturesRoot);
  }
  rootUrl(): string {
    return this.root;
  }

  /** Set the target LOD from the viewport's on-screen tile size (`64 × zoom`). */
  setTargetLod(px: number): void {
    this.targetPx = Math.max(LOD_SIZES[0], px);
  }

  /** Subscribe to "a LOD landed" — the viewport re-bakes to pick up the upgrade. */
  /** The tight opaque bbox (fractions of the frame, `0..1`) of `stem`'s sprite silhouette, computed
   *  once on the CPU at decode; null until the albedo has loaded. Used to size shadow-cast quads. */
  opaqueBBox(stem: string | undefined): { fx: number; fy: number; fw: number; fh: number } | null {
    return stem ? this.spriteBBox.get(stem) ?? null : null;
  }

  /** Register `stem`'s pre-atlas sprite scale (from the DSL layout, def-frame-anchors P5). A
   *  CHANGE evicts the stem's packed LODs + bboxes so they repack under the new transform
   *  (bytes stay cached — only the pack redoes). */
  setSpriteScale(stem: string, sw: number, sh: number): void {
    const cur = this.spriteScale.get(stem);
    if (cur && cur[0] === sw && cur[1] === sh) return;
    if (!cur && sw === 1 && sh === 1) return;
    this.spriteScale.set(stem, [sw, sh]);
    for (const key of [...this.packed.keys()]) {
      if (stemOf(key) !== stem) continue;
      this.packed.delete(key);
      this.packedHash.delete(key);
    }
    this.spriteBBox.delete(stem);
    this.rawBBox.delete(stem);
  }

  onLoad(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
  private emit(): void {
    for (const fn of this.listeners) fn();
  }

  lodStats(): LodStats {
    const counts = new Map<number, number>();
    let pages = 0;
    for (const [size, pool] of this.pools) {
      counts.set(size, pool.count);
      pages += pool.pageCount;
    }
    return { pages, counts, previewSize: FLOOR_LOD, previewCount: this.pools.get(FLOOR_LOD)?.count ?? 0 };
  }

  /** Best sub-frame for `stem`'s `map` available now (+ tier), kicking the upgrade toward the target
   *  LOD. A falsy stem, no root/renderer, an unlisted stem, or a map the stem lacks → geo (null). */
  resolve(stem: string | undefined, map: TexMap = "albedo", cell?: number): ResolvedTexture {
    if (!stem || !this.root || !this.renderer) return { frame: null, geo: true };

    const entry = this.manifest.entry(stem);
    if (!entry) return { frame: null, geo: true };
    if (map !== "albedo" && !entry.maps.includes(map)) return { frame: null, geo: true };

    const key = mapKey(stem, map);
    const cap = entry.maxSize;
    const gridFactor = entry.grid ? Math.max(entry.grid[0], entry.grid[1]) : 1;
    let desired = pickLodForSize(Math.min(this.targetPx * gridFactor, cap));
    if (desired > cap) desired = LOD_SIZES.filter((s) => s <= cap).pop() ?? LOD_SIZES[0];
    const loaded = this.packed.get(key);

    if (!loaded?.has(desired)) void this.ensureLod(stem, desired, entry.hash, map);
    if (!(loaded && loaded.size > 0) && desired > FLOOR_LOD && FLOOR_LOD <= cap)
      void this.ensureLod(stem, FLOOR_LOD, entry.hash, map);

    if (loaded && loaded.size > 0) {
      const best = this.bestLoaded(loaded, desired);
      if (best) {
        if (entry.grid && cell != null) return { frame: this.cellFrame(best, cell, entry.grid, entry.pad), geo: false };
        return { frame: best, geo: false };
      }
    }
    return { frame: null, geo: true };
  }

  /** A cached UV sub-frame of a packed linked-atlas frame for `cell` (row-major, 0-based), trimmed by
   *  the manifest inset `pad`. Narrows the packed frame's page rect to the cell's inset rect. */
  private cellFrame(base: TexFrame, cell: number, grid: [number, number], pad?: [number, number]): TexFrame {
    let byCell = this.cellFrames.get(base);
    if (!byCell) this.cellFrames.set(base, (byCell = new Map()));
    const hit = byCell.get(cell);
    if (hit) return hit;

    const [cols, rows] = grid;
    const [pu, pv] = pad ?? [0, 0];
    const cx = cell % cols;
    const cy = Math.floor(cell / cols) % rows;
    const u0 = cx / cols + pu;
    const v0 = cy / rows + pv;
    const uw = 1 / cols - 2 * pu;
    const vh = 1 / rows - 2 * pv;
    // Project the cell's [0,1] UV rect onto the packed frame's PIXEL rect on its page.
    const t = new TexFrame(base.source, base.x + u0 * base.w, base.y + v0 * base.h, uw * base.w, vh * base.h);
    byCell.set(cell, t);
    return t;
  }

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

  /** The best-fit loaded LOD for `desired`: largest ≤ it, else smallest above. */
  private bestLoaded(loaded: Map<number, TexFrame>, desired: number): TexFrame | null {
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
  private async ensureLod(stem: string, size: number, hash: string, map: TexMap): Promise<void> {
    const key = mapKey(stem, map);
    const pkey = `${key}@${size}`;
    if (this.pending.has(pkey) || this.packed.get(key)?.has(size)) return;
    this.pending.add(pkey);
    try {
      const cached = await getLod(stem, size, map);
      let bytes: ArrayBuffer;
      if (cached && cached.v === hash) {
        bytes = cached.bytes;
      } else {
        const res = await fetch(lodUrl(this.root, stem, hash, size, map));
        if (res.status === 404) {
          this.manifest.refresh();
          return;
        }
        if (!res.ok) throw new Error(`lod fetch ${res.status}: ${lodUrl(this.root, stem, hash, size, map)}`);
        bytes = await res.arrayBuffer();
        void putLod(stem, size, map, { v: hash, bytes });
      }

      // albedo/layers/normal/surface are DATA maps — load them STRAIGHT-alpha (no premultiply) so
      // their RGB survives intact; only true colour maps stay premultiplied.
      const raw = map === "layers" || map === "albedo" || map === "normal" || map === "surface";
      const bmp = await createImageBitmap(new Blob([bytes]), raw ? { premultiplyAlpha: "none" } : {});
      // Ingest transform (def-frame-anchors P5): a stem with a sprite_scale packs SCALED — clipped
      // to the same pow2 frame, transparent-filled, re-centred on its surface presence. The raw
      // (pre-scale) surface bbox drives the re-centre, so it must decode FIRST: a non-surface map
      // arriving early defers (bytes are already cached — the next resolve retries cheaply).
      const scale = this.spriteScale.get(stem);
      const scaled = !!scale && (scale[0] !== 1 || scale[1] !== 1) && !this.manifest.entry(stem)?.grid;
      if (scale && this.manifest.entry(stem)?.grid && !this.scaleWarned) {
        this.scaleWarned = true;
        console.warn(`[resolver] ${stem}: sprite_scale on a GRID stem is unsupported — packed unscaled`);
      }
      // Pre-compute the sprite's opaque bbox ONCE on the CPU from the SURFACE map's coverage (B channel;
      // the albedo alpha is full, so it's the surface that holds the silhouette). No GPU readback → no
      // mid-render corruption. Fractions of the frame → LOD-independent. With a scale, the RAW bbox
      // drives the re-centre and the STORED bbox is the post-transform one.
      if (map === "surface" && !this.spriteBBox.has(stem)) {
        const rawB = computeSpriteBBox(bmp);
        this.rawBBox.set(stem, rawB);
        this.spriteBBox.set(stem, scaled ? transformedBBox(rawB, scale![0], scale![1]) : rawB);
      }
      if (scaled && map !== "surface" && !this.rawBBox.has(stem)) {
        if (this.manifest.entry(stem)?.maps.includes("surface")) {
          void this.ensureLod(stem, size, hash, "surface"); // the re-centre needs the surface first
          return;
        } // no surface map at all → centre on the frame (no presence to centre on)
      }
      const actualShort = Math.min(bmp.width, bmp.height);
      const achieved = Math.min(size, actualShort);
      const ok = this.packInto(key, achieved, bmp, raw, scaled ? scale : undefined, this.rawBBox.get(stem));
      bmp.close();
      if (ok) {
        this.packedHash.set(key, hash);
        this.emit();
      }
    } catch {
      /* LOD unavailable — stay on the current tier */
    } finally {
      this.pending.delete(pkey);
    }
  }

  /** Pack a decoded bitmap into `size`'s pool under `(key, size)`. Uploads the bitmap into an engine
   *  Texture (premultiply for colour, straight for data maps), blits it into the atlas, then drops the
   *  scratch upload (the atlas page kept its own copy). With a `scale`, the sprite draws SCALED into
   *  the same-size frame — clipped to it, transparent-filled, re-centred so the (pre-scale) surface
   *  bbox's centre lands at the frame centre (or plain frame-centred without a bbox). */
  private packInto(
    key: string, size: number, bmp: ImageBitmap, raw = false,
    scale?: [number, number], rawB?: { fx: number; fy: number; fw: number; fh: number },
  ): boolean {
    if (!this.renderer) return false;
    const src = new Texture(this.renderer.gl, { width: bmp.width, height: bmp.height, data: bmp, premultiply: !raw });
    let packed: TexFrame | null;
    try {
      let draw;
      if (scale) {
        const W = bmp.width, H = bmp.height;
        const dw = W * scale[0], dh = H * scale[1];
        // Content origin so the scaled (pre-scale-bbox) centre sits at the frame centre.
        const cx = rawB ? rawB.fx + rawB.fw / 2 : 0.5, cy = rawB ? rawB.fy + rawB.fh / 2 : 0.5;
        const ox = W / 2 - cx * dw, oy = H / 2 - cy * dh;
        // Clip the dest rect to the frame; narrow the source UVs to match (exact, no scissor).
        const d0x = Math.max(0, ox), d1x = Math.min(W, ox + dw);
        const d0y = Math.max(0, oy), d1y = Math.min(H, oy + dh);
        draw = {
          dx: d0x, dy: d0y, dw: d1x - d0x, dh: d1y - d0y,
          sx: ((d0x - ox) / dw) * W, sy: ((d0y - oy) / dh) * H,
          sw: ((d1x - d0x) / dw) * W, sh: ((d1y - d0y) / dh) * H,
        };
      }
      const pool = key.endsWith("|surface") ? this.surfacePoolFor() : this.poolFor(size);
      packed = pool.add(key, src, bmp.width, bmp.height, draw);
    } finally {
      src.destroy();
    }
    if (!packed) return false;
    let byKey = this.packed.get(key);
    if (!byKey) this.packed.set(key, (byKey = new Map()));
    byKey.set(size, packed);
    return true;
  }

  /** The pool for LOD `size`, created empty on first use (small → 1024², larger → 2048²). */
  private poolFor(size: number): LodPool {
    let pool = this.pools.get(size);
    if (!pool) {
      pool = new LodPool(this.renderer!, this.blitter!, size <= SMALL_LOD_MAX ? 1024 : 2048);
      this.pools.set(size, pool);
    }
    return pool;
  }

  /** The single shared surface pool (see {@link surfacePool}), created on first surface pack. */
  private surfacePoolFor(): LodPool {
    if (!this.surfacePool) this.surfacePool = new LodPool(this.renderer!, this.blitter!, 2048);
    return this.surfacePool;
  }
}

/** The post-ingest-transform bbox fractions: scale the raw bbox about the origin, shift by the
 *  re-centring offset (raw-bbox centre → frame centre), clamp to the frame (clipping may cut it). */
function transformedBBox(
  b: { fx: number; fy: number; fw: number; fh: number }, sw: number, sh: number,
): { fx: number; fy: number; fw: number; fh: number } {
  const cx = b.fx + b.fw / 2, cy = b.fy + b.fh / 2;
  const ox = 0.5 - cx * sw, oy = 0.5 - cy * sh; // fraction-space content origin after re-centre
  const x0 = Math.max(0, ox + b.fx * sw), x1 = Math.min(1, ox + (b.fx + b.fw) * sw);
  const y0 = Math.max(0, oy + b.fy * sh), y1 = Math.min(1, oy + (b.fy + b.fh) * sh);
  return { fx: x0, fy: y0, fw: Math.max(0, x1 - x0), fh: Math.max(0, y1 - y0) };
}

/** Tight opaque bbox of a decoded SURFACE sprite (fractions of the frame, `0..1`), from the coverage
 *  (B) channel — the sprite's silhouette. Pure CPU (OffscreenCanvas getImageData); no GPU readback. */
function computeSpriteBBox(bmp: ImageBitmap): { fx: number; fy: number; fw: number; fh: number } {
  const w = bmp.width, h = bmp.height;
  const c = new OffscreenCanvas(w, h);
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return { fx: 0, fy: 0, fw: 1, fh: 1 };
  ctx.drawImage(bmp, 0, 0);
  const d = ctx.getImageData(0, 0, w, h).data;
  let minX = w, minY = h, maxX = -1, maxY = -1;
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++)
      if (d[(y * w + x) * 4 + 2] > 127) { // B = coverage/presence
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
  if (maxX < 0) return { fx: 0, fy: 0, fw: 1, fh: 1 }; // fully transparent → treat as full frame
  return { fx: minX / w, fy: minY / h, fw: (maxX - minX + 1) / w, fh: (maxY - minY + 1) / h };
}
