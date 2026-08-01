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
import { type AtlasDraw } from "./TextureAtlas";
import { getLod, putLod } from "./previewCache";
import { LOD_SIZES, lodUrl, pickLodForSize, type TexMap } from "./lod";
import { SQUARE } from "../game/viewport/squareMath";
import { TextureManifest } from "./textureManifest";

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

/** One tile at native scale — the target LOD floor AND ceiling (zoom 1 → `SQUARE` px). `SQUARE` is the
 *  maximum art size on the fixed slot grid: a slot holds one tile in `SQUARE` texels at lod 0, so a
 *  larger master could never be shown. Zoom past 1 magnifies instead of fetching bigger art. */
const BASE_LOD_PX = SQUARE;
/** A cheap low LOD fetched alongside the target while a stem has nothing on hand. */
const FLOOR_LOD = 32;
/** LODs at or below this pack into a 1024² page; larger into 2048². */
const SMALL_LOD_MAX = 64;

export class TextureResolver {
  private renderer: Renderer | null;
  private blitter: Blitter | null = null;
  private root: string;

  /** ONE shared pool for every CO-PACKED sprite frame across ALL lod sizes. Each frame is `2N × 2N`
   *  holding the stem's four maps as quadrants (albedo TL, normal TR, surface BL, layers BR). The shadow
   *  gather + the lighting bake bind a SINGLE page while prims hold defs at MIXED lods (immutable per-lod
   *  defs), so every lod's frame must live on the same page — per-size pools put each lod on its own page
   *  and every mid-session lod change went off-page. Co-packing keeps all four maps co-located at every lod
   *  (normal = frame + a fixed quadrant offset), so there is no separate normal page/band. Mixed pow2 ≥16
   *  frames keep the 16-px def alignment. 2048² spill hits the off-page warn (the C5 texture array lifts it). */
  private spritePool: LodPool | null = null;
  /** stem → its loaded CO-PACKED `2N` frames, keyed by lod size. */
  private readonly packed = new Map<string, Map<number, TexFrame>>();
  /** Cached per-map quadrant sub-frames of a co-packed frame (frame → map → quadrant). */
  private readonly quadFrames = new WeakMap<TexFrame, Map<TexMap, TexFrame>>();
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
  /** stem → `[sw, sh, pivotX, pivotY]` — the pre-atlas sprite transform (def-frame-anchors P5,
   *  pivot added by pawn-part-placement F4): applied at pack — scaled about the pivot's point on
   *  the surface presence, clipped to the same pow2 frame, transparent-filled. */
  private readonly spriteScale = new Map<string, [number, number, number, number]>();
  /** texture-generalization: linked stem (the `<stem>/l` name) → its DSL `internal_padding`
   *  (UNITS, of a 16-unit cell) — the BETWEEN-CELL inset inside the atlas. Distinct from the
   *  manifest's external `pad` (fractions), which stays 0 per R5. Applied by `cellFrame`. */
  private readonly linkedPad = new Map<string, number>();
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

  /** build-walls P1: a LINKED tile stem's grid (`[cols, rows]`) — the manifest entry for
   *  `<stem>/l` — or null for an ordinary stem. The tile expansion uses this to route a
   *  tile through the linked-atlas path (`<stem>/l` + a neighbor-context cell). */
  linkedGridFor(stem: string): [number, number] | null {
    return this.manifest.entry(`${stem}/l`)?.grid ?? null;
  }

  /** texture-generalization: register a linked stem's DSL `internal_padding` (units of the
   *  16-unit cell). A CHANGE evicts the stem's packed frames so the cell sub-frame cache
   *  (keyed by frame object) rebuilds under the new inset — same eviction discipline as
   *  {@link setSpriteScale}. `name` is the `<stem>/l` form the draw path resolves. */
  setLinkedPad(name: string, padUnits: number): void {
    const cur = this.linkedPad.get(name) ?? 0;
    if (cur === padUnits) return;
    this.linkedPad.set(name, padUnits);
    this.packed.delete(name);
    this.packedHash.delete(name);
  }

  /** Repoint the texture root at login + fetch/poll the manifest. */
  setRoot(texturesRoot: string): void {
    this.root = texturesRoot;
    this.manifest.setRoot(texturesRoot);
  }
  rootUrl(): string {
    return this.root;
  }

  /** build-walls P2: a stem's manifest hash (DOM icon URLs are hash-addressed), or null. */
  manifestHashFor(stem: string): string | null {
    return this.manifest.entry(stem)?.hash ?? null;
  }

  /** Set the target LOD from the viewport's on-screen tile size (`SQUARE × zoom`), CLAMPED to
   *  `BASE_LOD_PX`. Zooming past 1 magnifies rather than fetching art above the maximum size — the
   *  slot holds a tile in `SQUARE` texels, so a bigger master could not be displayed anyway. */
  setTargetLod(px: number): void {
    this.targetPx = Math.min(BASE_LOD_PX, Math.max(LOD_SIZES[0], px));
  }

  /** Subscribe to "a LOD landed" — the viewport re-bakes to pick up the upgrade. */
  /** The tight opaque bbox (fractions of the frame, `0..1`) of `stem`'s sprite silhouette, computed
   *  once on the CPU at decode; null until the albedo has loaded. Used to size shadow-cast quads. */
  opaqueBBox(stem: string | undefined): { fx: number; fy: number; fw: number; fh: number } | null {
    return stem ? this.spriteBBox.get(stem) ?? null : null;
  }

  /** Register `stem`'s pre-atlas sprite scale (from the DSL layout, def-frame-anchors P5), with
   *  the PIVOT it scales about (`sprite_anchor`, fractions of the sprite's opaque bbox —
   *  pawn-part-placement F4). The pivot is the art point that must not move: bottom-anchored art
   *  (`sprite_anchor.y = 1`) keeps its feet on the same line instead of being re-centred off them.
   *  The default (0.5, 0.5) is the bbox centre — the behaviour this generalises.
   *
   *  A CHANGE evicts the stem's packed LODs + bboxes so they repack under the new transform
   *  (bytes stay cached — only the pack redoes). */
  setSpriteScale(stem: string, sw: number, sh: number, px = 0.5, py = 0.5): void {
    const cur = this.spriteScale.get(stem);
    if (cur && cur[0] === sw && cur[1] === sh && cur[2] === px && cur[3] === py) return;
    if (!cur && sw === 1 && sh === 1) return;
    this.spriteScale.set(stem, [sw, sh, px, py]);
    this.packed.delete(stem); // co-packed by stem → drop the whole stem's frames; bytes stay cached
    this.packedHash.delete(stem);
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
    // Co-pack: ONE shared sprite pool holds every stem's 2N frame. Report per-lod frame counts + total pages.
    const counts = new Map<number, number>();
    for (const byStem of this.packed.values())
      for (const size of byStem.keys()) counts.set(size, (counts.get(size) ?? 0) + 1);
    const pages = this.spritePool?.pageCount ?? 0;
    return { pages, counts, previewSize: FLOOR_LOD, previewCount: counts.get(FLOOR_LOD) ?? 0 };
  }

  /** Best sub-frame for `stem`'s `map` available now (+ tier), kicking the upgrade toward the target
   *  LOD. A falsy stem, no root/renderer, an unlisted stem, or a map the stem lacks → geo (null). */
  /** Whether the serving manifest lists `stem` (human-pawns P3): the variant-folder
   *  fallback — a def variant with no mastered folder degrades to the canonical stem
   *  instead of resolving geo forever (the wolf's variant-0 case). */
  has(stem: string): boolean {
    return this.manifest.entry(stem) !== undefined;
  }

  /** The stem's frame WORLD SPAN in tiles, from the manifest (meta.json → DSL-authored), or
   *  null when unstamped. Lighting-correctness P1b/I2: the RECORD layer's span source — span
   *  must never be derived from streamed atlas px, which is lod-dependent and simply wrong. */
  spanOf(stem: string): number | null {
    return this.manifest.entry(stem)?.span ?? null;
  }

  resolve(stem: string | undefined, map: TexMap = "albedo", cell?: number): ResolvedTexture {
    if (!stem || !this.root || !this.renderer) return { frame: null, geo: true };

    const entry = this.manifest.entry(stem);
    if (!entry) return { frame: null, geo: true };
    if (map !== "albedo" && !entry.maps.includes(map)) return { frame: null, geo: true };

    const cap = entry.maxSize;
    const gridFactor = entry.grid ? Math.max(entry.grid[0], entry.grid[1]) : 1;
    let desired = pickLodForSize(Math.min(this.targetPx * gridFactor, cap));
    if (desired > cap) desired = LOD_SIZES.filter((s) => s <= cap).pop() ?? LOD_SIZES[0];
    // CO-PACK: one 2N frame per (stem, size) holds all four maps as quadrants; a map is a quadrant of it.
    const loaded = this.packed.get(stem);

    if (!loaded?.has(desired)) void this.ensureCoPack(stem, desired, entry.hash);
    if (!(loaded && loaded.size > 0) && desired > FLOOR_LOD && FLOOR_LOD <= cap)
      void this.ensureCoPack(stem, FLOOR_LOD, entry.hash);

    if (loaded && loaded.size > 0) {
      const best = this.bestLoaded(loaded, desired); // the 2N co-packed frame at the best-loaded size
      if (best) {
        const quad = this.quadrant(best, map); // this map's N×N quadrant of the co-packed frame
        if (entry.grid && cell != null) {
          // texture-generalization: the DSL internal_padding (units of the 16-unit cell) joins
          // the manifest's external pad — converted to whole-atlas UV fractions (1 unit of a
          // cols-cell atlas = 1/(16·cols) of its width).
          const ip = this.linkedPad.get(stem) ?? 0;
          const pad: [number, number] = [
            (entry.pad?.[0] ?? 0) + ip / (16 * entry.grid[0]),
            (entry.pad?.[1] ?? 0) + ip / (16 * entry.grid[1]),
          ];
          return { frame: this.cellFrame(quad, cell, entry.grid, pad), geo: false };
        }
        return { frame: quad, geo: false };
      }
    }
    return { frame: null, geo: true };
  }

  /** CO-PACK quadrant order — MUST match {@link TextureAtlas.addCoPacked} + the shadow gather's normal offset. */
  private static readonly QUADRANT: Record<string, [number, number]> = {
    albedo: [0, 0], normal: [1, 0], surface: [0, 1], layers: [1, 1],
  };
  /** The `map`'s N×N quadrant sub-frame of a `2N × 2N` co-packed frame (cached per (frame, map)). */
  private quadrant(cf: TexFrame, map: TexMap): TexFrame {
    let byMap = this.quadFrames.get(cf);
    if (!byMap) this.quadFrames.set(cf, (byMap = new Map()));
    const hit = byMap.get(map);
    if (hit) return hit;
    const n = cf.w / 2;
    const [qx, qy] = TextureResolver.QUADRANT[map] ?? [0, 0];
    const q = new TexFrame(cf.source, cf.x + qx * n, cf.y + qy * n, n, n);
    byMap.set(map, q);
    return q;
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
    for (const stem of [...this.packed.keys()]) {
      const e = this.manifest.entry(stem);
      if (!e || this.packedHash.get(stem) !== e.hash) {
        this.packed.delete(stem);
        this.packedHash.delete(stem);
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

  // ── loads (CO-PACK) ───────────────────────────────────────────────────────────
  /** Load ALL of `stem`'s maps at `size` and CO-PACK them into one `2N` frame (albedo TL, normal TR,
   *  surface BL, layers BR). One 2N allocation per (stem, size) — the four maps land co-located on one
   *  page, so a prim's def frame + a fixed quadrant offset resolves any map at any lod. */
  private async ensureCoPack(stem: string, size: number, hash: string): Promise<void> {
    const pkey = `${stem}@${size}`;
    if (this.pending.has(pkey) || this.packed.get(stem)?.has(size)) return;
    this.pending.add(pkey);
    try {
      const entry = this.manifest.entry(stem);
      if (!entry) return;
      // CO-PACK order MUST match TextureResolver.QUADRANT / TextureAtlas.addCoPacked.
      const order: TexMap[] = ["albedo", "normal", "surface", "layers"];
      const want = order.map((m) => m === "albedo" || entry.maps.includes(m)); // absent → transparent quadrant
      const bytes = await Promise.all(order.map((m, i) => (want[i] ? this.loadMapBytes(stem, size, hash, m) : Promise.resolve(null))));
      // All four maps are DATA maps → STRAIGHT-alpha decode (RGB survives; no premultiply).
      const raw = { premultiplyAlpha: "none" as const };
      const bmps = await Promise.all(bytes.map((b) => (b ? createImageBitmap(new Blob([b]), raw) : Promise.resolve(null))));
      const albedo = bmps[0], surf = bmps[2];
      if (!albedo && !surf) return; // nothing usable
      const scale = this.spriteScale.get(stem);
      const scaled = !!scale && (scale[0] !== 1 || scale[1] !== 1) && !entry.grid;
      if (scale && entry.grid && !this.scaleWarned) {
        this.scaleWarned = true;
        console.warn(`[resolver] ${stem}: sprite_scale on a GRID stem is unsupported — packed unscaled`);
      }
      // Sprite opaque bbox from the SURFACE map's coverage (B channel) — LOD-independent fractions; drives
      // the re-centre + the shadow-cast quad. All maps decode together now, so there is no ordering defer.
      if (surf && !this.spriteBBox.has(stem)) {
        const rawB = computeSpriteBBox(surf);
        this.rawBBox.set(stem, rawB);
        this.spriteBBox.set(stem, scaled ? transformedBBox(rawB, scale!) : rawB);
      }
      const quadN = (albedo ?? surf)!.width; // square masters → every map is quadN × quadN at this lod
      const ok = this.packCoPack(stem, size, quadN, bmps, scaled ? scale : undefined, this.rawBBox.get(stem));
      for (const b of bmps) b?.close();
      if (ok) {
        this.packedHash.set(stem, hash);
        this.emit();
      }
    } catch {
      /* LOD unavailable — stay on the current tier */
    } finally {
      this.pending.delete(pkey);
    }
  }

  /** Fetch (or read the IndexedDB cache for) one map's LOD bytes; null on 404 (a stem lacking the map). */
  private async loadMapBytes(stem: string, size: number, hash: string, map: TexMap): Promise<ArrayBuffer | null> {
    const cached = await getLod(stem, size, map);
    if (cached && cached.v === hash) return cached.bytes;
    const res = await fetch(lodUrl(this.root, stem, hash, size, map));
    if (res.status === 404) {
      this.manifest.refresh();
      return null;
    }
    if (!res.ok) throw new Error(`lod fetch ${res.status}: ${lodUrl(this.root, stem, hash, size, map)}`);
    const bytes = await res.arrayBuffer();
    void putLod(stem, size, map, { v: hash, bytes });
    return bytes;
  }

  /** Upload the four decoded maps + CO-PACK them into one `2·quadN` frame in the shared sprite pool.
   *  A `scale` draws each quadrant SCALED + re-centred on the (pre-scale) surface bbox (rare — grids/scaled
   *  sprites); otherwise a straight quadrant blit. Stored under `(stem, size)`. */
  private packCoPack(
    stem: string, size: number, quadN: number, bmps: Array<ImageBitmap | null>,
    scale?: [number, number, number, number], rawB?: { fx: number; fy: number; fw: number; fh: number },
  ): boolean {
    if (!this.renderer) return false;
    const gl = this.renderer.gl;
    const srcs = bmps.map((b) => (b ? new Texture(gl, { width: b.width, height: b.height, data: b, premultiply: false }) : null));
    try {
      let draws: Array<AtlasDraw | null> | undefined;
      if (scale) {
        const W = quadN, H = quadN;
        const dw = W * scale[0], dh = H * scale[1];
        // F4: scale ABOUT THE PIVOT — the pivot's point on the opaque bbox is the one art point
        // that must land where it already was, so bottom-anchored art keeps its feet. Pivot
        // (0.5, 0.5) reduces to the bbox-centre re-centring this replaces.
        const px = rawB ? rawB.fx + rawB.fw * scale[2] : scale[2];
        const py = rawB ? rawB.fy + rawB.fh * scale[3] : scale[3];
        const ox = px * (W - dw), oy = py * (H - dh);
        const d0x = Math.max(0, ox), d1x = Math.min(W, ox + dw);
        const d0y = Math.max(0, oy), d1y = Math.min(H, oy + dh);
        const draw: AtlasDraw = {
          dx: d0x, dy: d0y, dw: d1x - d0x, dh: d1y - d0y,
          sx: ((d0x - ox) / dw) * W, sy: ((d0y - oy) / dh) * H,
          sw: ((d1x - d0x) / dw) * W, sh: ((d1y - d0y) / dh) * H,
        };
        draws = srcs.map((s) => (s ? draw : null));
      }
      const frame = this.spritePoolFor().addCoPacked(stem, srcs, quadN, draws);
      if (!frame) return false;
      let byStem = this.packed.get(stem);
      if (!byStem) this.packed.set(stem, (byStem = new Map()));
      byStem.set(size, frame);
      return true;
    } finally {
      for (const s of srcs) s?.destroy();
    }
  }

  /** The single shared CO-PACK pool (see {@link spritePool}), created on first pack. */
  private spritePoolFor(): LodPool {
    if (!this.spritePool) this.spritePool = new LodPool(this.renderer!, this.blitter!, 2048);
    return this.spritePool;
  }
}

/** The post-ingest-transform bbox fractions — the SAME transform `packCoPack` draws with (F4):
 *  scale about the pivot's point on the raw bbox, then clamp to the frame (clipping may cut it). */
function transformedBBox(
  b: { fx: number; fy: number; fw: number; fh: number }, scale: [number, number, number, number],
): { fx: number; fy: number; fw: number; fh: number } {
  const [sw, sh, pvx, pvy] = scale;
  const px = b.fx + b.fw * pvx, py = b.fy + b.fh * pvy;
  const ox = px * (1 - sw), oy = py * (1 - sh); // fraction-space content origin after the scale
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
  // lighting-visual P4: threshold at the VISUAL edge (~3%), not half-coverage — the blit renders
  // any nonzero coverage, so a >127 cut left softly-drawn feet BELOW the bbox bottom and every
  // shadow anchored a few px above where the art visibly ends (the user's remaining gap).
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++)
      if (d[(y * w + x) * 4 + 2] > 8) { // B = coverage/presence
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
  if (maxX < 0) return { fx: 0, fy: 0, fw: 1, fh: 1 }; // fully transparent → treat as full frame
  return { fx: minX / w, fy: minY / h, fw: (maxX - minX + 1) / w, fh: (maxY - minY + 1) / h };
}
