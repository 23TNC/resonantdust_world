//! The ONE-RESOLUTION texture resolver (2026-08-02-one-resolution) — every prim that names a
//! texture resolves through it to the stem's single co-packed frame, packed once at the stem's
//! MAXIMUM served size (the manifest's `maxSize`, F2):
//!
//!   geo (white)  →  the one co-pack
//!
//! The LOD ladder is DELETED: this game is zoom-reliant and always wants the maximum, so per-size
//! tiers, target tracking, the preview floor and the below/above chooser were retained complexity
//! (the old-game theory they served is gone). Minification is the MIP CHAIN's job at sample time.
//! A packed frame is a {@link TexFrame} (a UV sub-frame onto a shared atlas page); `cellFrame`
//! narrows a `TexFrame` for linked-atlas cells.
//!
//! GL is LAZY: the resolver is built at boot with no renderer (the viewport's GL context doesn't exist
//! until the world scene), so {@link attachRenderer} wires the {@link Blitter} + pools when the
//! viewport comes up. Before then `resolve` returns geo (nothing is packed yet). Geo returns a null
//! frame — the caller supplies its own white fill (unlike pixijs, which packs a shared white stem).

import { Blitter, type Renderer, Texture, TexFrame } from "../gl";
import { SpritePool } from "./SpritePool";
import { type AtlasDraw } from "./TextureAtlas";
import { getCachedMap, putCachedMap } from "./previewCache";
import { texUrl, type TexMap } from "./urls";
import { TextureManifest } from "./textureManifest";

/** The PREVIEW size, in px (F6). ONE fixed size — never chosen by zoom, never consulted for
 *  geometry, dropped the moment the master lands. That is the whole distinction from the deleted
 *  LOD ladder, whose VARYING drawing tier made the runtime bbox depend on decode order
 *  (`docs/work/2026-08-02-subframe-ingest/issues.md` I7).
 *
 *  32 because all four maps at 32 px cost 2.2 KB (conifer) to 4.3 KB (flora) — LESS than the 5.8 KB
 *  median a drawn placeholder would have needed, and it is real art rather than an approximation
 *  that has to keep matching. The edge derives and caches this size on demand from the master
 *  (`server/edge/src/textures.rs`: "no offline pyramid"), so nothing is generated ahead of time. */
const PREVIEW_PX = 32;

/** A resolve result: the atlas sub-frame to bake (null for the GEO tier — the caller uses its own
 *  white fill + the prim's silhouette colour), and whether it's geo rather than real pixels. */
export interface ResolvedTexture {
  frame: TexFrame | null;
  geo: boolean;
}

/** Sprite-pool occupancy for the debug HUD — one frame per stem now, so a plain counter.
 *  `partial` counts packed stems missing a LISTED map's bytes (lod-aftermath P0: the
 *  black-flora class — a transient 404 must never silently bake an incomplete co-pack). */
export interface PoolStats {
  pages: number;
  frames: number;
  partial: number;
}

/** lod-aftermath P0: bounded re-pack for a co-pack that landed incomplete. */
const PARTIAL_RETRY_MAX = 3;
const PARTIAL_RETRY_BASE_MS = 2000;

export class TextureResolver {
  private renderer: Renderer | null;
  private blitter: Blitter | null = null;
  private root: string;

  /** ONE shared pool for every CO-PACKED sprite frame — one frame per stem, at its manifest max.
   *  Each frame is `2N × 2N` holding the stem's four maps as quadrants (albedo TL, normal TR,
   *  surface BL, layers BR) across the TWIN pages (F4: graphics filtered, data exact — same
   *  coordinates, two textures). Pow2-square ≥16 frames keep the 16-px def alignment (the F3
   *  LAW, enforced by refusal). 2048² spill hits the off-page warn (the C5 texture array lifts it). */
  private spritePool: SpritePool | null = null;
  /** stem → its ONE loaded co-packed `2N` frame (one-resolution: the ladder is gone). */
  private readonly packed = new Map<string, TexFrame>();
  /** stem → the SIZE its resident frame was packed at. The NEVER-DOWNGRADE guard: a preview that
   *  loses the race to its master must be discarded, not written over it (subframe-ingest taught
   *  the cost of two sources for one value; this is the same rule for pixels). 0 = nothing packed. */
  private readonly packedSize = new Map<string, number>();
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
  /** subframe-ingest: stem → the DSL-authored `[x, y, w, h, anchorX, anchorY]` — which FRACTION of
   *  the master is actually art, plus the pivot on it. Applied at ingest by {@link packCoPack},
   *  identically to all four maps, which is what registers them with each other.
   *
   *  Keyed **`stem#cell`** — but the cell axis belongs to GRID masters alone (a linked autotile
   *  cell, or a facing kind whose manifest entry carries `grid`), applied per cell at `cellFrame`.
   *  Everything else is one master per stem and reads `#0` at ingest: a pawn facing rides the
   *  stem (`…/e`), and so does a cold thing's VARIANT (`…/<v>/e`, lod-aftermath I3 — keying
   *  variants as cells of the canonical stem gave every conifer variant 0's crop and clipped
   *  eight of nine, because `resolve()` drops the cell on a non-grid entry). West needs no entry:
   *  `FACING_BY_ROTATION` maps a west facing to the EAST stem plus `flipX`, so it inherits east's
   *  rect by construction — the whole of F4, for free. */
  private readonly subframe = new Map<string, [number, number, number, number, number, number]>();
  /** texture-generalization: linked stem (the `<stem>/l` name) → its DSL `internal_padding`
   *  (UNITS, of a 16-unit cell) — the BETWEEN-CELL inset inside the atlas.
   *
   *  **Slated for deletion into {@link setSubframe}** ([F6](../../../../docs/work/2026-08-02-subframe-ingest/forks.md#f6)):
   *  a uniform inset IS a uniform subframe. It survives only because a grid stem's subframe applies
   *  per CELL at RESOLVE (each cell is a sub-rect of one packed image), not per stem at INGEST, so
   *  the substitution is at `cellFrame` rather than at `packCoPack` and wants `cellFrame` to take a
   *  rect instead of a symmetric pad. Deleting it half-way would silently shift every autotile cell. */
  private readonly linkedPad = new Map<string, number>();

  /** texture-generalization: register a linked stem's DSL `internal_padding` (units of the
   *  16-unit cell). See {@link linkedPad} — this is on its way out. */
  setLinkedPad(name: string, padUnits: number): void {
    const cur = this.linkedPad.get(name) ?? 0;
    if (cur === padUnits) return;
    this.linkedPad.set(name, padUnits);
    this.packed.delete(name);
    this.packedSize.delete(name);   // see the note in setSpriteScale
    this.packedHash.delete(name);
  }
  private scaleWarned = false;

  /** lod-aftermath P0: stems whose LAST master pack was missing a listed map's bytes (a 404
   *  during an outage window) — completeness bookkeeping + a bounded TIME-DRIVEN retry, so
   *  ONE transient failure can never silently bake an incomplete co-pack for a session (I2:
   *  black flora). Time-driven because resolve() goes quiet once the bakes drain — a
   *  resolve-side trigger parks forever (learned in the first drill). */
  private readonly partialPacks = new Map<string, { missing: TexMap[]; retries: number }>();

  private readonly listeners = new Set<(stem?: string) => void>();

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

  /** The manifest grid for an EXACT stem name, or null — the variant-routing probe: a facing
   *  kind whose master really is a cell grid keeps the cell dialect; a folder-variant kind
   *  (conifer `0..8/`) rides the stem instead, because `resolve()` drops the cell on a
   *  non-grid entry and ingest crops with `#0` alone. */
  gridOf(name: string): [number, number] | null {
    return this.manifest.entry(name)?.grid ?? null;
  }

  /** subframe-ingest: register a stem's DSL-authored SUBFRAME — the fraction of the master that is
   *  actually art (`x, y, w, h`, all `0..1`) plus the pivot on it (`ax, ay`).
   *
   *  The atlas then ingests **that rect and nothing else**, identically for albedo, normal, surface
   *  and layers. This replaces deriving the rect from decoded pixels: the derived bbox ran on
   *  whichever size happened to decode first and so moved between sessions
   *  (`docs/work/2026-08-02-subframe-ingest/issues.md` I7), which is not something placement can be
   *  registered against.
   *
   *  It also replaces `internal_padding` — a uniform inset IS a uniform subframe, so a linked tile
   *  grid authors its inset the same way everything else does (F6).
   *
   *  A CHANGE evicts the stem's packed frames so they repack under the new rect — the same eviction
   *  discipline as {@link setSpriteScale} (bytes stay cached; only the pack redoes). */
  setSubframe(stem: string, cell: number, x: number, y: number, w: number, h: number, ax = 0.5, ay = 0.5): void {
    const key = `${stem}#${cell}`;
    const cur = this.subframe.get(key);
    if (cur && cur[0] === x && cur[1] === y && cur[2] === w && cur[3] === h && cur[4] === ax && cur[5] === ay) return;
    // The whole frame with a centre pivot is the DEFAULT — registering it would make every
    // unauthored stem take the crop path for a no-op, and evict on first registration.
    if (!cur && x === 0 && y === 0 && w === 1 && h === 1 && ax === 0.5 && ay === 0.5) return;
    this.subframe.set(key, [x, y, w, h, ax, ay]);
    // `cellFrames` is keyed by (packed frame, cell) and a registered rect changes that cell's UVs,
    // so the stem's frames must go: dropping `packed` orphans the old TexFrame objects and the
    // WeakMap entries go with them. Bytes stay cached — only the pack redoes.
    this.packed.delete(stem);
    this.packedSize.delete(stem);   // or the guard blocks the re-pack at the same size, forever
    this.packedHash.delete(stem);
  }

  /** The authored rect for one addressable frame, or null. */
  private subframeFor(stem: string, cell: number): [number, number, number, number, number, number] | undefined {
    return this.subframe.get(`${stem}#${cell}`);
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

  /** one-resolution F2: the stem's ONE served size — the manifest's `maxSize`. DOM consumers
   *  (build-panel icons) build their URLs from this instead of a literal. */
  maxSizeFor(stem: string): number | null {
    return this.manifest.entry(stem)?.maxSize ?? null;
  }

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
    this.packed.delete(stem);
    this.packedSize.delete(stem);   // or the guard blocks the re-pack at the same size, forever
    this.packedHash.delete(stem);
    this.spriteBBox.delete(stem);
    this.rawBBox.delete(stem);
  }

  onLoad(fn: (stem?: string) => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
  /** Notify that `stem`'s frames changed — or, with no stem, that EVERY frame may have moved (a
   *  page repack, a manifest swap). The argument is what lets a listener dirty a handful of squares
   *  instead of all of them (render-performance I11). */
  private emit(stem?: string): void {
    for (const fn of this.listeners) fn(stem);
  }

  poolStats(): PoolStats {
    // one-resolution: ONE co-pack per stem — a plain counter is the whole story.
    return { pages: this.spritePool?.pageCount ?? 0, frames: this.packed.size, partial: this.partialPacks.size };
  }

  /** Whether the serving manifest lists `stem` (human-pawns P3): the variant-folder
   *  fallback — a def variant with no mastered folder degrades to the canonical stem
   *  instead of resolving geo forever (the wolf's variant-0 case). */
  has(stem: string): boolean {
    return this.manifest.entry(stem) !== undefined;
  }

  /** The stem's frame WORLD SPAN in tiles, from the manifest (meta.json → DSL-authored), or
   *  null when unstamped. Lighting-correctness P1b/I2: the RECORD layer's span source — span
   *  must never be derived from streamed atlas px, which is size-dependent and simply wrong. */
  spanOf(stem: string): number | null {
    return this.manifest.entry(stem)?.span ?? null;
  }

  resolve(stem: string | undefined, map: TexMap = "albedo", cell?: number): ResolvedTexture {
    if (!stem || !this.root || !this.renderer) return { frame: null, geo: true };

    const entry = this.manifest.entry(stem);
    if (!entry) return { frame: null, geo: true };
    if (map !== "albedo" && !entry.maps.includes(map)) return { frame: null, geo: true };

    // one-resolution: ONE co-pack per stem at its maximum served size (F2). No target, no
    // tiers, no preview floor — a stem is packed or it is geo, and the kick is idempotent.
    // F6: kick the PREVIEW and the MASTER in parallel. The preview is ~100x smaller so it almost
    // always lands first; serialising them would only delay the master. Both kicks are idempotent
    // and `packedSize` keeps a late preview from overwriting an early master.
    const cf = this.packed.get(stem);
    const have = this.packedSize.get(stem) ?? 0;
    if (have < entry.maxSize) {
      if (have < PREVIEW_PX && entry.maxSize > PREVIEW_PX) this.ensureCoPack(stem, PREVIEW_PX, entry.hash);
      this.ensureCoPack(stem, entry.maxSize, entry.hash);
    }
    if (!cf) return { frame: null, geo: true };
    const quad = this.quadrant(cf, map); // this map's N×N quadrant of the co-packed frame
    if (entry.grid && cell != null) {
      // texture-generalization: the DSL internal_padding (units of the 16-unit cell) joins
      // the manifest's external pad — converted to whole-atlas UV fractions (1 unit of a
      // cols-cell atlas = 1/(16·cols) of its width).
      const ip = this.linkedPad.get(stem) ?? 0;
      const pad: [number, number] = [
        (entry.pad?.[0] ?? 0) + ip / (16 * entry.grid[0]),
        (entry.pad?.[1] ?? 0) + ip / (16 * entry.grid[1]),
      ];
      const sf = this.subframeFor(stem, cell);
      const sub: [number, number, number, number] | undefined =
        sf ? [sf[0], sf[1], sf[2], sf[3]] : undefined;
      return { frame: this.cellFrame(quad, cell, entry.grid, pad, sub), geo: false };
    }
    return { frame: quad, geo: false };
  }

  /** CO-PACK quadrant order — MUST match {@link TextureAtlas.addCoPacked} + the shadow gather's normal offset. */
  private static readonly QUADRANT: Record<string, [number, number]> = {
    albedo: [0, 0], normal: [1, 0], surface: [0, 1], layers: [1, 1],
  };
  /** The `map`'s N×N quadrant sub-frame of a `2N × 2N` co-packed frame (cached per (frame, map)).
   *  F4: the co-pack frame references the DATA page; albedo/normal quadrants RE-SOURCE to the
   *  page's graphics twin (identical coordinates — one packer rect, two textures). */
  private quadrant(cf: TexFrame, map: TexMap): TexFrame {
    let byMap = this.quadFrames.get(cf);
    if (!byMap) this.quadFrames.set(cf, (byMap = new Map()));
    const hit = byMap.get(map);
    if (hit) return hit;
    const n = cf.w / 2;
    const [qx, qy] = TextureResolver.QUADRANT[map] ?? [0, 0];
    const graphics = map !== "surface"; // F4: surface alone is DATA; albedo/normal/layers filter
    const source = graphics ? this.spritePool?.graphicsTwin(cf.source) ?? cf.source : cf.source;
    const q = new TexFrame(source, cf.x + qx * n, cf.y + qy * n, n, n);
    byMap.set(map, q);
    return q;
  }

  /** A cached UV sub-frame of a packed linked-atlas frame for `cell` (row-major, 0-based), trimmed by
   *  the manifest inset `pad`. Narrows the packed frame's page rect to the cell's inset rect. */
  private cellFrame(base: TexFrame, cell: number, grid: [number, number], pad?: [number, number],
                    sub?: [number, number, number, number]): TexFrame {
    let byCell = this.cellFrames.get(base);
    if (!byCell) this.cellFrames.set(base, (byCell = new Map()));
    const hit = byCell.get(cell);
    if (hit) return hit;

    const [cols, rows] = grid;
    const cx = cell % cols;
    const cy = Math.floor(cell / cols) % rows;
    // A GRID stem's crop happens HERE, not at ingest: the atlas packs the whole grid as one frame
    // and a cell is a sub-rect of it, so there is no per-cell ingest to crop. This is the same site
    // `internal_padding` uses, which is why the subframe subsumes it (F6) — a uniform inset is just
    // the symmetric case of this rect. `sub` is in CELL fractions and wins where authored.
    let u0: number, v0: number, uw: number, vh: number;
    if (sub) {
      u0 = (cx + sub[0]) / cols;
      v0 = (cy + sub[1]) / rows;
      uw = sub[2] / cols;
      vh = sub[3] / rows;
    } else {
      const [pu, pv] = pad ?? [0, 0];
      u0 = cx / cols + pu;
      v0 = cy / rows + pv;
      uw = 1 / cols - 2 * pu;
      vh = 1 / rows - 2 * pv;
    }
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
        this.packedSize.delete(stem);   // or the guard blocks the re-pack at the same size, forever
        this.packedHash.delete(stem);
      }
    }
    this.emit();
  }

  // ── loads (CO-PACK) ───────────────────────────────────────────────────────────
  /** Load ALL of `stem`'s maps at the manifest max and CO-PACK them into one `2N` frame (albedo TL,
   *  normal TR, surface BL, layers BR). ONE allocation per stem — the four maps land co-located, so
   *  a prim's def frame + a fixed quadrant offset resolves any map. */
  private async ensureCoPack(stem: string, size: number, hash: string): Promise<void> {
    // Keyed by SIZE, not stem: the preview and the master are two in-flight packs for one stem and
    // must not dedupe each other (F6).
    const pkey = `${stem}@${size}`;
    // NEVER DOWNGRADE. A preview that loses the race to its master is discarded here rather than
    // overwriting it — the failure this guard exists for is a slow 2 KB preview landing after a
    // fast-cached master and visibly blurring an already-sharp stem.
    if (this.pending.has(pkey) || (this.packedSize.get(stem) ?? 0) >= size) return;
    this.pending.add(pkey);
    try {
      const entry = this.manifest.entry(stem);
      if (!entry) return;
      // CO-PACK order MUST match TextureResolver.QUADRANT / TextureAtlas.addCoPacked.
      const order: TexMap[] = ["albedo", "normal", "surface", "layers"];
      const want = order.map((m) => m === "albedo" || entry.maps.includes(m)); // absent → transparent quadrant
      const bytes = await Promise.all(order.map((m, i) => (want[i] ? this.loadMapBytes(stem, size, hash, m) : Promise.resolve(null))));
      // lod-aftermath P0 (I2): a LISTED map whose bytes did not come back is a PARTIAL pack —
      // pack anyway (progressive beats geo) but remember, so a bounded retry can complete it.
      // Only the MASTER tier is tracked; a partial preview is superseded by the master anyway.
      const missing = order.filter((m, i) => want[i] && !bytes[i]) as TexMap[];
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
      // Sprite opaque bbox from the SURFACE map's coverage (B channel) — size-independent fractions; drives
      // the re-centre + the shadow-cast quad. All maps decode together now, so there is no ordering defer.
      // The bbox is derived ONLY from the master. Deriving it from whichever size decoded first is
      // precisely the instability that `subframe-ingest` I7 recorded — the same art measured
      // fy 0.125 one session and 0.109 the next. The preview feeds PIXELS and nothing else (F2).
      if (surf && size >= (entry.maxSize ?? size) && !this.spriteBBox.has(stem)) {
        const rawB = computeSpriteBBox(surf);
        this.rawBBox.set(stem, rawB);
        this.spriteBBox.set(stem, scaled ? transformedBBox(rawB, scale!) : rawB);
      }
      const quadN = (albedo ?? surf)!.width; // square masters → every map is quadN × quadN
      // Re-check the guard: the master may have landed while these bytes were in flight.
      if ((this.packedSize.get(stem) ?? 0) >= size) { for (const b of bmps) b?.close(); return; }
      const ok = this.packCoPack(stem, size, quadN, bmps, scaled ? scale : undefined, this.rawBBox.get(stem));
      for (const b of bmps) b?.close();
      if (ok) {
        this.packedSize.set(stem, size);
        this.packedHash.set(stem, hash);
        if (size >= (entry.maxSize ?? size)) {
          if (missing.length) {
            const prev = this.partialPacks.get(stem);
            const retries = prev?.retries ?? 0;
            this.partialPacks.set(stem, { missing, retries });
            if (retries < PARTIAL_RETRY_MAX) {
              const delay = PARTIAL_RETRY_BASE_MS * Math.pow(2, retries);
              console.warn(`[resolver] ${stem}: packed WITHOUT [${missing.join(", ")}] — retry ${retries + 1}/${PARTIAL_RETRY_MAX} in ${delay} ms`);
              setTimeout(() => {
                const p = this.partialPacks.get(stem);
                const e = this.manifest.entry(stem);
                if (!p || !e) return;           // healed or unlisted meanwhile
                p.retries++;
                this.packedSize.delete(stem);   // reopen the size guard; the old frame keeps serving
                void this.ensureCoPack(stem, e.maxSize, e.hash);
              }, delay);
            } else {
              console.warn(`[resolver] ${stem}: still missing [${missing.join(", ")}] after ${PARTIAL_RETRY_MAX} retries — giving up (the partial counter stays raised)`);
            }
          } else {
            this.partialPacks.delete(stem); // complete — heal the ledger
          }
        }
        this.emit(stem);
      } else {
        // I2: a pack that FAILS must still notify. `if (ok) emit()` alone left a stem whose bytes had
        // arrived sitting on geo for the entire session, with no re-bake ever scheduled — the bytes
        // were in memory and nothing asked for them again.
        this.emit(stem);
      }
    } catch (e) {
      // Bytes unavailable — the stem stays geo. WARN, don't swallow: a silent catch here
      // once hid a whole-world geo regression (lighting-visual P5) behind zero console output.
      console.warn(`[resolver] co-pack failed for ${stem}@${size}:`, e);
    } finally {
      this.pending.delete(pkey);
    }
  }

  /** Fetch (or read the IndexedDB cache for) one map's bytes; null on 404 (a stem lacking the map). */
  private async loadMapBytes(stem: string, size: number, hash: string, map: TexMap): Promise<ArrayBuffer | null> {
    const cached = await getCachedMap(stem, map);
    if (cached && cached.v === hash) return cached.bytes;
    const res = await fetch(texUrl(this.root, stem, hash, size, map));
    if (res.status === 404) {
      this.manifest.refresh();
      return null;
    }
    if (!res.ok) throw new Error(`texture fetch ${res.status}: ${texUrl(this.root, stem, hash, size, map)}`);
    const bytes = await res.arrayBuffer();
    void putCachedMap(stem, map, { v: hash, bytes });
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
      const sub = this.manifest.entry(stem)?.grid ? undefined : this.subframeFor(stem, 0);
      if (sub) {
        // ── SUBFRAME INGEST (subframe-ingest P2) ───────────────────────────────────────────────
        // The crop the whole stream is about. ONE rect out of the master, blitted into the
        // quadrant, and — critically — the SAME rect for all four maps, which is what registers
        // albedo/normal/surface/layers with each other by construction rather than by two
        // derivations agreeing.
        const N = quadN;
        const [fx, fy, fw, fh, ax, ay] = sub;
        const sx = fx * N, sy = fy * N, sw = Math.max(1, fw * N), sh = Math.max(1, fh * N);
        // F2: scale to FIT, aspect preserved. The quadrant must stay square pow2 — `ppu` and the
        // F3 packing law both assume it — so exactly one axis fills and the other keeps a margin.
        // Stretching to fill would distort any non-square subframe; blitting at native px would
        // keep the letterbox this exists to remove.
        let k = Math.min(N / sw, N / sh);
        // F5: `sprite_scale` MULTIPLIES the fitted rect. It no longer performs a pivot re-centre of
        // its own — that was the second placement, and two placements at one seam is the bug class
        // this stream removes. Its job is now purely "how much of the span's footprint the art
        // occupies"; the subframe owns where the pixels go.
        const kw = k * (scale ? scale[0] : 1), kh = k * (scale ? scale[1] : 1);
        const dw = sw * kw, dh = sh * kh;
        // Placed so the art's OWN anchor point lands at the same fraction of the quadrant. With the
        // corpus's `sprite_anchor.y = 1` that puts the feet exactly on the frame's bottom edge —
        // which is why the plan line ends up on the feet rather than on a letterbox margin.
        const dx = ax * (N - dw), dy = ay * (N - dh);
        // Clip to the quadrant, carrying the source rect with it, so an over-large `sprite_scale`
        // cannot bleed into a neighbouring quadrant of the co-packed frame.
        const d0x = Math.max(0, dx), d1x = Math.min(N, dx + dw);
        const d0y = Math.max(0, dy), d1y = Math.min(N, dy + dh);
        if (d1x <= d0x || d1y <= d0y) return false;
        const draw: AtlasDraw = {
          dx: d0x, dy: d0y, dw: d1x - d0x, dh: d1y - d0y,
          sx: sx + ((d0x - dx) / kw), sy: sy + ((d0y - dy) / kh),
          sw: (d1x - d0x) / kw, sh: (d1y - d0y) / kh,
        };
        draws = srcs.map((s) => (s ? draw : null));
      } else if (scale) {
        const W = quadN, H = quadN;
        const dw = W * scale[0], dh = H * scale[1];
        // F4: scale ABOUT THE PIVOT — the pivot's point on the opaque bbox is the one art point
        // that must land where it already was, so bottom-anchored art keeps its feet. Pivot
        // (0.5, 0.5) reduces to the bbox-centre re-centring this replaces.
        //
        // LEGACY PATH: only reached by a stem with NO authored subframe. It re-centres on the
        // DERIVED bbox, which is what drifts between sessions (I7). Authoring a subframe for the
        // stem takes the branch above and this stops running for it.
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
      this.packed.set(stem, frame); // one-resolution: THE frame, not one-of-sizes
      return true;
    } finally {
      for (const s of srcs) s?.destroy();
    }
  }

  /** The single shared CO-PACK pool (see {@link spritePool}), created on first pack. */
  private spritePoolFor(): SpritePool {
    if (!this.spritePool) this.spritePool = new SpritePool(this.renderer!, this.blitter!, 2048);
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
