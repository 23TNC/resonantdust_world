//! The square cache: a FIXED-SIZE, toroidal render-texture cache of the world, baked
//! from primitives on the {@link SQUARE} grid. Channel-agnostic — it bakes N sibling
//! CHANNELS (albedo today; normal / emissive / depth next) from ONE shared prim index,
//! window, dirty queue, partition and wrap apron. Only the OUTPUT differs per channel:
//! each {@link Channel} owns its ping-pong buffer pair + a `resolve` hook that maps a
//! prim to that channel's texture. This is the descendant of the old game's multi-channel
//! `RectComposite`.
//!
//! The mechanism (all shared across channels):
//!   • A `cols × rows` grid of square slots. World square `(wc,wr)` lives at physical
//!     slot `(mod(wc,cols), mod(wr,rows))`. The LOD sets the SLOT SIZE ({@link slotPx},
//!     CSS px): a world square is drawn into a `slotPx × slotPx` rect, so crossing a LOD
//!     boundary changes the slot size + count while the buffer stays a FIXED viewport
//!     size (allocated once — a ping-pong pair reused across zoom).
//!   • {@link recenter} advances the window in whole-square steps; {@link markStale}
//!     re-derives the re-bake set from slot ownership ground truth (never a stale slot).
//!   • {@link resize} re-partitions on a LOD change: builds into the OTHER buffer,
//!     {@link reproject}s the old content scaled old→new slot size, swaps, and holds the
//!     old one frame as `prev` (no blank flash).
//!   • A 1-slot wrap-apron ring (edge slots duplicated to the opposite border during the
//!     bake) makes the display sample slots edge-to-edge with no gutter and no wrap seam.
//!
//! {@link fillDisplay} emits one quad per resident world square (shared geometry); the
//! viewport samples a channel's composite through it, scaled by the zoom.

import {
  Buffer,
  BufferUsage,
  Container,
  Geometry,
  Matrix,
  Mesh,
  MeshGeometry,
  RenderTexture,
  Sprite,
  Texture,
  type Renderer,
} from "pixi.js";
import { makeMaterialBakeShader, type MaterialBakeShader } from "./materialBakeShader";
import { makeDepthBakeShader, type DepthBakeShader } from "./depthBakeShader";
import { makeNormalBakeShader, type NormalBakeShader } from "./normalBakeShader";
import { makeSurfaceBakeShader, type SurfaceBakeShader } from "./surfaceBakeShader";
import type { PackedChannel } from "./material";
import {
  mod,
  OVERSCAN,
  sameRange,
  SQUARE,
  squaresForAABB,
  squareWorldX,
  squareWorldY,
  type SquareRange,
} from "./squareMath";
import { ZOOM_MAX } from "../../textures/lod";

/** A renderable occupant of the map. Baked into every square its AABB overlaps.
 *  `textureName` (optional) names a texture in a channel's `resolve` chain — when set,
 *  the bake asks the channel for the best tier on hand instead of using `texture`
 *  directly; `texture` remains the fallback. `tint`/`geoColor` are albedo semantics a
 *  channel's `resolve` interprets (another channel may ignore them). `zIndex` orders
 *  overlapping prims within a square (low → first). */
export interface Primitive {
  readonly id: number;
  texture: Texture;
  textureName?: string;
  /** World-px top-left. */
  x: number;
  y: number;
  width: number;
  height: number;
  tint: number;
  geoColor?: number;
  /** Mirror the sprite horizontally when baked — the west facing reuses the east
   *  master flipped, so no separate west texture ever ships. */
  flipX?: boolean;
  /** For a LINKED (autotile) atlas texture: which cell (row-major, 0-based) of the atlas
   *  to bake. A channel's `resolve` passes it to the resolver, which returns that cell's UV
   *  sub-frame of the one atlas texture. Undefined for an ordinary single-image texture. */
  cell?: number;
  /** The prim's up-to-4 packed-map material bindings (`Content.tilePackedChannels`),
   *  by packed RGBA channel. When set AND the stem's `packed` map is loaded, the
   *  albedo channel bakes hue/chroma VARIATION instead of a flat tint; otherwise
   *  ignored (flat, as before). See {@link material.ts}. */
  packed?: readonly PackedChannel[];
  /** A STABLE per-instance seed the material bake hashes into a per-instance noise offset
   *  (cell-derived for static things, `objectId` for movers) so instances differ without the
   *  pattern swimming when a mover moves. `0` when unset. */
  seed?: number;
  zIndex: number;
}

/** What a channel's {@link ChannelSpec.resolve} returns: the texture to bake and the
 *  tint to bake it WITH (the channel picks the tier-appropriate colour). When `material`
 *  is set, the bake draws this prim through the {@link MaterialBakeShader} (the single
 *  real-tier albedo path) instead of a flat tinted sprite — see {@link MaterialResolve}. */
export interface ResolvedPrim {
  texture: Texture;
  tint: number;
  material?: MaterialResolve;
  /** When set, the prim bakes through the {@link DepthBakeShader} instead of a tinted sprite:
   *  `texture` is the SURFACE source (its B → presence) and this scalar (0..1) is the tile
   *  depth written into B where present. Used by the `depth` channel. */
  depth?: number;
  /** When set, the prim bakes through the {@link NormalBakeShader}: `rgb` is the normal LOD
   *  (or null → flat-up) and `alpha` (`texture`) is the SURFACE source (its B = soft coverage)
   *  the normal is premultiplied by. Used by the `normal` channel. */
  normal?: { rgb: Texture | null; alpha: Texture };
  /** When set, the prim bakes through the {@link SurfaceBakeShader}: `texture` is the SURFACE
   *  source (RGB), presence-premultiplied so the composite carries A = presence. `surface` channel. */
  surface?: boolean;
}

/** The extra inputs the material bake path needs, resolved from a prim's packed channels
 *  + the material registry. Reconstruction is `residual + Σ layersᵢ·jitter`, alpha applied
 *  from `surface.B` after the sum. `residual` is the `albedo` map (RGB base), `layers` the
 *  weight map (or null → residual only), `surface` supplies the alpha. `chA[i] = (tintR,
 *  tintG, tintB, hueSwing)`, `chB[i] = (chromaSwing, warmCoolBias, noiseRow, sampleSpace)`. */
export interface MaterialResolve {
  residual: Texture;
  layers: Texture | null;
  surface: Texture;
  chA: Float32Array;
  chB: Float32Array;
}

/** Fields a caller supplies to {@link SquareCache.addPrim}; `id` is assigned. */
export type PrimitiveSpec = Omit<Primitive, "id" | "tint" | "zIndex"> &
  Partial<Pick<Primitive, "tint" | "zIndex">>;

/** A channel to bake: a key (for the display / RT preview) and the hook that maps a prim
 *  to this channel's texture + tint. */
export interface ChannelSpec {
  key: string;
  resolve: (prim: Primitive) => ResolvedPrim;
}

interface PrimEntry {
  prim: Primitive;
  range: SquareRange;
}

const sqKey = (c: number, r: number): string => `${c},${r}`;

/** Slot-grid size is quantized to this many squares, so a continuous zoom re-partitions
 *  the (fixed) buffer only every few squares rather than every square. The toroidal
 *  window still slides freely within the quantized grid, so panning stays a strip
 *  re-bake — the quantization only affects zoom re-partitions. */
const SIZE_STEP = 4;

/** The composite bakes at this resolution (device px per CSS px). 1 = CSS resolution —
 *  the fixed buffer is ~viewport-sized and a quarter the memory of a device-res one; on
 *  a HiDPI screen the world is upsampled to device px at draw (mild softness, fine for
 *  simple sprites). Bump toward the device pixel ratio for a sharper HiDPI composite. */
const WORLD_RES = 1;

/** Extra CSS px reserved around the viewport in the FIXED buffer so the derived slot
 *  size stays ≥ the on-screen tile even at max zoom-in (where the OVERSCAN ring + the
 *  1-slot wrap-apron ring's tiles are largest) — i.e. the visible tiles never get
 *  squeezed below crisp. `OVERSCAN + 1` = the overscan ring plus the apron ring. */
const RESERVE_CSS = 2 * (OVERSCAN + 1) * SQUARE * ZOOM_MAX;

/** Dirty-square priority bands (lower = baked sooner). Within a band the ring distance
 *  from the viewport centre is added, so each band fills centre-out. HIGH = squares
 *  with NO pixels on hand (blank — get them first); STD = a re-bake of content already
 *  shown (a texture LOD landed, a prim moved); LOW = a reprojected placeholder that
 *  already shows the right thing at the wrong sharpness (crisp-up last). */
const PRIO_HIGH = 0;
const PRIO_STD = 256;
const PRIO_LOW = 512;
const RING_MAX = 255;

/** The shared slot state {@link SquareCache.fillDisplay} samples. During a LOD swap the
 *  outgoing layer is held one frame as `prev` so the display never catches the incoming
 *  (freshly-reprojected) buffers before they're ready — no blank flash. The buffers
 *  themselves are held per channel ({@link Channel.prevComposite}). */
interface PrevLayer {
  cols: number;
  rows: number;
  slotPx: number;
  winCol: number;
  winRow: number;
  ownerCol: Int32Array;
  ownerRow: Int32Array;
  baked: Uint8Array;
}

/** One bake target: a fixed-size ping-pong buffer pair + the resolve hook. The cache
 *  drives the shared `active` index / swap / reproject / bake; the channel just holds
 *  its buffers and how to resolve a prim into its texture. */
class Channel {
  readonly key: string;
  readonly resolve: (prim: Primitive) => ResolvedPrim;
  bufs: [RenderTexture, RenderTexture] | null = null;
  /** The outgoing buffer, held one frame during a LOD swap. */
  prevComposite: RenderTexture | null = null;

  constructor(spec: ChannelSpec) {
    this.key = spec.key;
    this.resolve = spec.resolve;
  }

  /** (Re)allocate the fixed ping-pong pair `cw × ch` at `res`, both cleared. */
  ensureBuffers(cw: number, ch: number, res: number, renderer: Renderer, empty: Container): void {
    if (this.bufs) {
      this.bufs[0].destroy(true);
      this.bufs[1].destroy(true);
    }
    const a = RenderTexture.create({ width: cw, height: ch, resolution: res });
    const b = RenderTexture.create({ width: cw, height: ch, resolution: res });
    // OPAQUE clear (α = 1). Every composite is opaque RGB data — nothing writes α < 1 — so the
    // verbatim slot blit (over-blend) always REPLACES the slot instead of leaving stale pixels
    // where the new content is transparent. This is the fix for the depth-RT stale-tree bug.
    renderer.render({ container: empty, target: a, clear: true, clearColor: [0, 0, 0, 1] });
    renderer.render({ container: empty, target: b, clear: true, clearColor: [0, 0, 0, 1] });
    this.bufs = [a, b];
    this.prevComposite = null;
  }

  destroy(): void {
    if (this.bufs) {
      this.bufs[0].destroy(true);
      this.bufs[1].destroy(true);
      this.bufs = null;
    }
    this.prevComposite = null;
  }
}

export class SquareCache {
  // ── the fixed-size composites (one ping-pong pair per channel, shared `active`) ──
  private readonly channels: Channel[];
  private active = 0;
  private fixedCW = 0;
  private fixedCH = 0;
  /** One-square scratch the bake renders into before blitting to each channel's slot. */
  private scratchRT: RenderTexture | null = null;
  private scratchTex: Texture | null = null;

  private cols = 0;
  private rows = 0;
  private viewW = 0;
  private viewH = 0;
  /** Slot interior size in CSS px = the LOD. A world square (SQUARE world px) is drawn
   *  into a `slotPx × slotPx` slot. */
  private slotPx = SQUARE;
  /** Composite resolution (device px per CSS px) — the LOD is carried by {@link slotPx},
   *  not by inflating this. */
  private res = 1;

  // ── the window over the world (discrete anchor) ──────────────────────────────
  private winCol = 0;
  private winRow = 0;
  private aimed = false;

  // ── per-slot ownership (shared across channels — same geometry) ──────────────
  // The world square each physical slot was last baked for. Ground truth that
  // {@link markStale} re-derives the dirty set from, and {@link fillDisplay} refuses to
  // sample a slot never baked for the square it's drawing.
  private slotOwnerCol = new Int32Array(0);
  private slotOwnerRow = new Int32Array(0);
  private slotBaked = new Uint8Array(0);

  // ── primitive ⇄ square index (shared) ────────────────────────────────────────
  private nextId = 1;
  private readonly prims = new Map<number, PrimEntry>();
  private readonly squarePrims = new Map<string, Set<number>>();
  /** Dirty squares → their bake priority (lower baked first). See the PRIO_* bands. */
  private readonly dirty = new Map<string, number>();

  /** The outgoing slot state, displayed for one frame after a LOD swap. Promoted (the
   *  channels' held buffers reused) at the top of the next {@link resize}. */
  private prev: PrevLayer | null = null;

  // ── bake scratch objects (reused across channels + squares) ──────────────────
  private readonly empty = new Container();
  private readonly bakeContainer = new Container();
  private readonly blitSprite = new Sprite();
  private readonly bakePool: Sprite[] = [];
  /** Material-bake meshes, pooled parallel to {@link bakePool} — a prim resolving to a
   *  {@link MaterialResolve} draws through one of these (its own shader = its own
   *  per-prim uniforms) instead of a flat sprite. All share {@link materialQuad}. */
  private readonly materialPool: Mesh[] = [];
  /** Depth-bake meshes, pooled parallel to {@link bakePool} — a prim resolving with a `depth`
   *  scalar draws through one of these (its own shader = its own tile-depth). Shares
   *  {@link materialQuad}. */
  private readonly depthPool: Mesh[] = [];
  /** Normal-bake meshes, pooled parallel to {@link bakePool} — a prim resolving with a `normal`
   *  binding draws through one of these (silhouette-masked normal). Shares {@link materialQuad}. */
  private readonly normalPool: Mesh[] = [];
  /** Surface-bake meshes, pooled parallel to {@link bakePool} — a prim resolving with `surface`
   *  draws through one of these (presence-premultiplied surface). Shares {@link materialQuad}. */
  private readonly surfacePool: Mesh[] = [];
  /** The unit-quad geometry every material mesh shares (per-prim world rect comes from the
   *  mesh transform; per-prim params from its shader). Lazily built (needs no renderer). */
  private materialQuad: MeshGeometry | null = null;
  /** The tiling noise atlas bound to every material bake (rows = fields). `null` until the
   *  viewport sets it — a null noise atlas disables the material path (flat, as before). */
  private noiseTex: Texture | null = null;
  /** Noise globals pushed to each material shader: `[rows, uvTile, worldTile]`. */
  private noiseGlobals: readonly [number, number, number] = [1, 1, 128];

  /** Diagnostics: squares baked on the last {@link bakeDirty}. */
  lastBaked = 0;

  constructor(channels: ChannelSpec[]) {
    this.channels = channels.map((c) => new Channel(c));
    // Verbatim slot copy — no premultiply/blend, so the scratch overwrites the slot exactly.
    this.blitSprite.blendMode = "none";
    (globalThis as any).__map = this;
  }

  /** Bind the shared noise atlas + its globals for the material bake path. `rows` = atlas
   *  field rows, `uvTile` = noise tiles across a sprite (uv space), `worldTile` = world px
   *  per noise tile (world space). A `null` texture disables material variation. */
  setNoise(texture: Texture | null, rows: number, uvTile: number, worldTile: number): void {
    this.noiseTex = texture;
    this.noiseGlobals = [rows, uvTile, worldTile];
  }

  /** The buffer the viewport's display mesh samples for channel `key` — the outgoing
   *  buffer while a LOD swap is in flight (held one frame), else the live buffer. */
  displayComposite(key: string): RenderTexture | null {
    const ch = this.channels.find((c) => c.key === key);
    if (!ch || !ch.bufs) return null;
    return this.prev ? ch.prevComposite : ch.bufs[this.active];
  }

  /** The channel keys, in bake order (the RT preview lists these). */
  channelKeys(): string[] {
    return this.channels.map((c) => c.key);
  }

  /** The band base + centre-out ring priority for square `(wc,wr)`. */
  private prio(wc: number, wr: number, band: number): number {
    const cc = this.winCol + (this.cols >> 1);
    const cr = this.winRow + (this.rows >> 1);
    const ring = Math.min(Math.max(Math.abs(wc - cc), Math.abs(wr - cr)), RING_MAX);
    return band + ring;
  }

  /** Mark square `(wc,wr)` dirty at `band`, keeping the most urgent (lowest) priority. */
  private markDirty(wc: number, wr: number, band: number): void {
    // The world is anchored at the origin with unsigned coords (region_x/y are u8); squares
    // left of / above it (col or row < 0) hold no content. Never dirty or bake them — the
    // window + OVERSCAN reach negative near the origin, but there's nothing there to draw.
    if (wc < 0 || wr < 0) return;
    const k = sqKey(wc, wr);
    const p = this.prio(wc, wr, band);
    const cur = this.dirty.get(k);
    if (cur === undefined || p < cur) this.dirty.set(k, p);
  }

  /** Physical slot counts. */
  get gridCols(): number {
    return this.cols;
  }
  get gridRows(): number {
    return this.rows;
  }

  /** True once sized + the buffers are allocated (geometry can be filled). */
  get ready(): boolean {
    return this.cols > 0 && this.channels.length > 0 && this.channels[0].bufs != null;
  }

  /** The fixed-buffer / toroidal mapping — for an external world-space RT (shadow-world) that must
   *  register 1:1 with the composites. A world square `(wc, wr)` lives at buffer slot
   *  `(mod(wc,cols)+1, mod(wr,rows)+1) · slotPx`, so a world px `(wx,wy)` maps to buffer px
   *  `(mod(wx/SQUARE, cols)+1)·slotPx` (the +1 = the wrap-apron ring). Buffer is `fixedCW × fixedCH` at
   *  resolution 1; the display samples it by normalized UV, so an external RT of the same size aligns. */
  bufferMapping(): { fixedCW: number; fixedCH: number; cols: number; rows: number; slotPx: number; winCol: number; winRow: number } {
    return { fixedCW: this.fixedCW, fixedCH: this.fixedCH, cols: this.cols, rows: this.rows, slotPx: this.slotPx, winCol: this.winCol, winRow: this.winRow };
  }

  /** Number of display quads {@link fillDisplay} emits — ONE per resident world square
   *  of the DISPLAYED layer (the held `prev` during a swap, else the live grid). */
  get displayQuadCount(): number {
    return this.prev ? this.prev.cols * this.prev.rows : this.cols * this.rows;
  }

  /** How much a channel composite's UV advances per WORLD px, `[du/dx, dv/dy]`, for the
   *  DISPLAYED layer. A world square (`SQUARE` world px) maps to a `slotPx`-CSS-px slot in
   *  the fixed buffer, so `du/dx = (slotPx/SQUARE)/fixedCW`. The lighting shader marches the
   *  depth composite in UV space with this (the toroidal composite has no linear world→UV
   *  map, but the per-square scale is uniform, so a short in-window ray steps correctly). */
  get displayUvPerWorld(): [number, number] {
    const slotPx = this.prev ? this.prev.slotPx : this.slotPx;
    const per = slotPx / SQUARE;
    return [this.fixedCW > 0 ? per / this.fixedCW : 0, this.fixedCH > 0 ? per / this.fixedCH : 0];
  }

  // ── sizing ───────────────────────────────────────────────────────────────────
  /** (Re)partition for a `screenW × screenH`-CSS-px viewport at `zoom`. The buffers are
   *  FIXED-size (allocated once per viewport size); the LOD only changes the SLOT size,
   *  DERIVED so the quantized `cols × rows` grid + apron packs the fixed buffer. When the
   *  grid or slot size changes it builds into the OTHER ping-pong buffer, {@link reproject}s
   *  the old content scaled old→new slot size, swaps, and holds the old one frame as
   *  `prev`. No RenderTexture is allocated per zoom. */
  resize(screenW: number, screenH: number, renderer: Renderer, zoom: number, anchorX: number, anchorY: number): void {
    if (screenW <= 0 || screenH <= 0) return;
    this.ensureBuffers(screenW, screenH, renderer);
    // Release a swap prepared last frame — the buffers are reused, nothing to destroy.
    this.prev = null;
    for (const ch of this.channels) ch.prevComposite = null;

    const worldW = screenW / zoom;
    const worldH = screenH / zoom;
    this.viewW = worldW;
    this.viewH = worldH;
    // Quantize to SIZE_STEP squares (+ the OVERSCAN ring): the visible-square count.
    const units = (extent: number): number =>
      Math.ceil(Math.ceil(extent / SQUARE) / SIZE_STEP) * SIZE_STEP + 2 * OVERSCAN;
    const cols = units(worldW);
    const rows = units(worldH);
    // Derived slot size: the largest square slot that packs `cols × rows` PLUS the
    // one-slot wrap-apron ring (+2 each axis) into the fixed buffer.
    const slotPx = Math.max(1, Math.min(Math.floor(this.fixedCW / (cols + 2)), Math.floor(this.fixedCH / (rows + 2))));
    if (cols === this.cols && rows === this.rows && slotPx === this.slotPx && this.aimed) return;

    // Snapshot the current slot state to reproject FROM + hold as `prev` one frame.
    const old: PrevLayer | null =
      this.aimed && this.cols > 0
        ? {
            cols: this.cols,
            rows: this.rows,
            slotPx: this.slotPx,
            winCol: this.winCol,
            winRow: this.winRow,
            ownerCol: this.slotOwnerCol,
            ownerRow: this.slotOwnerRow,
            baked: this.slotBaked,
          }
        : null;

    // Build into the OTHER buffer of every channel, then swap the shared live pointer.
    this.active = 1 - this.active;
    for (const ch of this.channels) {
      if (old) ch.prevComposite = ch.bufs![1 - this.active]; // the just-vacated buffer
      renderer.render({ container: this.empty, target: ch.bufs![this.active], clear: true, clearColor: [0, 0, 0, 1] }); // opaque (see ensureBuffers)
    }
    this.cols = cols;
    this.rows = rows;
    this.slotPx = slotPx;
    // Scratch is one slot; re-allocate when the slot size changes.
    this.scratchRT?.destroy(true);
    this.scratchTex?.destroy();
    this.scratchRT = RenderTexture.create({ width: slotPx, height: slotPx, resolution: this.res });
    this.scratchTex = new Texture({ source: this.scratchRT.source, dynamic: true });
    this.slotOwnerCol = new Int32Array(cols * rows);
    this.slotOwnerRow = new Int32Array(cols * rows);
    this.slotBaked = new Uint8Array(cols * rows);
    this.dirty.clear();

    if (old) {
      // Aim the new window on the anchor (same rule as recenter, so recenter no-ops),
      // then reproject each channel's old content into it and hold it for display.
      this.winCol = Math.floor((anchorX - worldW / 2) / SQUARE) - OVERSCAN;
      this.winRow = Math.floor((anchorY - worldH / 2) / SQUARE) - OVERSCAN;
      this.aimed = true;
      this.reproject(renderer, old);
      this.prev = old;
    } else {
      // First aim / no prior content — whole-window re-bake on the next recenter.
      this.aimed = false;
    }
  }

  /** Allocate every channel's FIXED ping-pong pair for a `screenW × screenH` viewport (+
   *  the {@link RESERVE_CSS} margin), at {@link WORLD_RES}. Re-allocates only when the
   *  viewport size changes — never per zoom. Invalidates the partition. */
  private ensureBuffers(screenW: number, screenH: number, renderer: Renderer): void {
    const cw = Math.ceil(screenW + RESERVE_CSS);
    const ch = Math.ceil(screenH + RESERVE_CSS);
    if (this.channels[0]?.bufs && cw === this.fixedCW && ch === this.fixedCH) return;
    this.fixedCW = cw;
    this.fixedCH = ch;
    this.res = WORLD_RES;
    for (const channel of this.channels) channel.ensureBuffers(cw, ch, WORLD_RES, renderer, this.empty);
    this.active = 0;
    // Force a re-partition + whole-window re-bake into the fresh buffers.
    this.cols = 0;
    this.rows = 0;
    this.slotPx = 0;
    this.aimed = false;
    this.prev = null;
    this.dirty.clear();
  }

  /** Reproject every channel's old buffer into its new one, SCALED old→new slot size (so
   *  a 64→128 LOD change draws the old tile stretched). The copyable-slot set + the remap
   *  geometry are computed ONCE (shared); each channel then does one remap-mesh draw from
   *  its held `prevComposite`. Copies mark LOW (crisp-up last); squares the old buffer
   *  didn't hold mark HIGH (blank, bake first). */
  private reproject(renderer: Renderer, old: PrevLayer): void {
    // Both ping-pong buffers share the fixed size, so old slots normalise to it.
    const oldCw = this.fixedCW;
    const oldCh = this.fixedCH;
    const pos: number[] = [];
    const uv: number[] = [];
    for (let wc = this.winCol; wc < this.winCol + this.cols; wc++)
      for (let wr = this.winRow; wr < this.winRow + this.rows; wr++) {
        const inOld = wc >= old.winCol && wc < old.winCol + old.cols && wr >= old.winRow && wr < old.winRow + old.rows;
        if (inOld) {
          const oSx = mod(wc, old.cols);
          const oSy = mod(wr, old.rows);
          const oSi = oSy * old.cols + oSx;
          if (old.baked[oSi] === 1 && old.ownerCol[oSi] === wc && old.ownerRow[oSi] === wr) {
            const nSx = mod(wc, this.cols);
            const nSy = mod(wr, this.rows);
            // Dest quad — new interior slot (+1 past the apron) in new-composite CSS px.
            const dx0 = (nSx + 1) * this.slotPx;
            const dy0 = (nSy + 1) * this.slotPx;
            const dx1 = dx0 + this.slotPx;
            const dy1 = dy0 + this.slotPx;
            // Source quad — old interior slot (+1) as normalised UVs of the old composite.
            const su0 = ((oSx + 1) * old.slotPx) / oldCw;
            const sv0 = ((oSy + 1) * old.slotPx) / oldCh;
            const su1 = su0 + old.slotPx / oldCw;
            const sv1 = sv0 + old.slotPx / oldCh;
            pos.push(dx0, dy0, dx1, dy0, dx1, dy1, dx0, dy1);
            uv.push(su0, sv0, su1, sv0, su1, sv1, su0, sv1);
            const nSi = nSy * this.cols + nSx;
            this.slotOwnerCol[nSi] = wc;
            this.slotOwnerRow[nSi] = wr;
            this.slotBaked[nSi] = 1;
            this.markDirty(wc, wr, PRIO_LOW); // placeholder shown — crisp-up last
            continue;
          }
        }
        this.markDirty(wc, wr, PRIO_HIGH); // never held — blank, bake first
      }

    if (pos.length === 0) return;
    const quads = pos.length / 8;
    const idx = new Uint32Array(quads * 6);
    for (let q = 0; q < quads; q++) {
      const v = q * 4;
      const o = q * 6;
      idx[o] = v; idx[o + 1] = v + 1; idx[o + 2] = v + 2;
      idx[o + 3] = v; idx[o + 4] = v + 2; idx[o + 5] = v + 3;
    }
    const geo = new Geometry({
      attributes: {
        aPosition: { buffer: new Buffer({ data: new Float32Array(pos), usage: BufferUsage.VERTEX }), format: "float32x2" },
        aUV: { buffer: new Buffer({ data: new Float32Array(uv), usage: BufferUsage.VERTEX }), format: "float32x2" },
      },
      indexBuffer: new Buffer({ data: idx, usage: BufferUsage.INDEX }),
    });
    // One draw per channel — shared geometry, the channel's own old→new buffers.
    for (const ch of this.channels) {
      if (!ch.prevComposite || !ch.bufs) continue;
      const oldTex = new Texture({ source: ch.prevComposite.source });
      const mesh = new Mesh({ geometry: geo, texture: oldTex });
      mesh.blendMode = "none"; // verbatim copy into the freshly-cleared new composite
      renderer.render({ container: mesh, target: ch.bufs[this.active], clear: false });
      mesh.destroy();
      oldTex.destroy();
    }
    geo.destroy(true); // free the vertex/index buffers (reproject runs often during zoom)
  }

  // ── window (discrete anchor) ──────────────────────────────────────────────────
  /** Move the window so it stays centred on the anchor. When it moves, {@link markStale}
   *  re-dirties every slot that now addresses a different world square than it was last
   *  baked for — derived from slot ownership, not the pan delta, so it self-heals. */
  recenter(anchorWorldX: number, anchorWorldY: number): void {
    if (!this.scratchRT) return;
    const newCol = Math.floor((anchorWorldX - this.viewW / 2) / SQUARE) - OVERSCAN;
    const newRow = Math.floor((anchorWorldY - this.viewH / 2) / SQUARE) - OVERSCAN;
    if (this.aimed && newCol === this.winCol && newRow === this.winRow) return;
    this.winCol = newCol;
    this.winRow = newRow;
    this.aimed = true;
    this.markStale();
  }

  /** Dirty every window slot whose baked owner is not the world square that now maps to
   *  it. One cheap `cols × rows` scan per window move, derived from ground truth. */
  private markStale(): void {
    const { cols, rows } = this;
    const baseC = mod(this.winCol, cols);
    const baseR = mod(this.winRow, rows);
    for (let sr = 0; sr < rows; sr++) {
      const wr = this.winRow + ((sr - baseR + rows) % rows);
      for (let sc = 0; sc < cols; sc++) {
        const wc = this.winCol + ((sc - baseC + cols) % cols);
        const si = sr * cols + sc;
        if (this.slotBaked[si] === 0 || this.slotOwnerCol[si] !== wc || this.slotOwnerRow[si] !== wr) {
          this.markDirty(wc, wr, PRIO_HIGH); // no valid pixels here — bake first
        }
      }
    }
  }

  // ── primitive index ───────────────────────────────────────────────────────────
  /** Register a primitive; returns its id. Links it to every square it overlaps and
   *  dirties those squares. */
  addPrim(spec: PrimitiveSpec): number {
    const id = this.nextId++;
    const prim: Primitive = {
      id,
      texture: spec.texture,
      textureName: spec.textureName,
      geoColor: spec.geoColor,
      x: spec.x,
      y: spec.y,
      width: spec.width,
      height: spec.height,
      tint: spec.tint ?? 0xffffff,
      flipX: spec.flipX,
      cell: spec.cell,
      packed: spec.packed,
      seed: spec.seed,
      zIndex: spec.zIndex ?? 0,
    };
    const range = squaresForAABB(prim.x, prim.y, prim.x + prim.width, prim.y + prim.height);
    this.prims.set(id, { prim, range });
    this.linkRange(id, range);
    this.dirtyRange(range);
    return id;
  }

  /** The mutable primitive for `id` (move it, retint it, swap its texture), or null.
   *  Call {@link refreshPrim} after mutating so the index + dirty set catch up. */
  getPrim(id: number): Primitive | null {
    return this.prims.get(id)?.prim ?? null;
  }

  /** The STANDING prims — `zIndex ≥ 1` (things, not ground tiles) — i.e. the shadow casters.
   *  Insertion order; the caller culls by light radius + resolves each prim's world rect. */
  standingPrims(): Primitive[] {
    const out: Primitive[] = [];
    for (const { prim } of this.prims.values()) if (prim.zIndex >= 1) out.push(prim);
    return out;
  }

  /** Re-evaluate `id`'s footprint after a mutation: relink if it moved/resized, and dirty
   *  both the old and new squares (its art/tint may also have changed). */
  refreshPrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    const range = squaresForAABB(e.prim.x, e.prim.y, e.prim.x + e.prim.width, e.prim.y + e.prim.height);
    if (!sameRange(range, e.range)) {
      this.unlinkRange(id, e.range);
      this.dirtyRange(e.range); // re-bake the vacated squares
      e.range = range;
      this.linkRange(id, range);
    }
    this.dirtyRange(e.range); // always: tint/texture may have changed in place
  }

  /** Drop a primitive and dirty the squares it vacated. */
  removePrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    this.unlinkRange(id, e.range);
    this.dirtyRange(e.range);
    this.prims.delete(id);
  }

  private linkRange(id: number, r: SquareRange): void {
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) {
        const k = sqKey(c, row);
        let s = this.squarePrims.get(k);
        if (!s) this.squarePrims.set(k, (s = new Set()));
        s.add(id);
      }
  }

  private unlinkRange(id: number, r: SquareRange): void {
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) {
        const k = sqKey(c, row);
        const s = this.squarePrims.get(k);
        if (s) {
          s.delete(id);
          if (s.size === 0) this.squarePrims.delete(k);
        }
      }
  }

  private dirtyRange(r: SquareRange): void {
    // A prim moved/landed on already-shown content — STD priority.
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) this.markDirty(c, row, PRIO_STD);
  }

  /** Re-dirty every square that holds a prim, so the next bake re-resolves textures
   *  through each channel's `resolve`. A texture resolver calls this when a tier lands. */
  invalidateAll(): void {
    for (const k of this.squarePrims.keys()) {
      const ci = k.indexOf(",");
      this.markDirty(+k.slice(0, ci), +k.slice(ci + 1), PRIO_STD);
    }
  }

  // ── bake ───────────────────────────────────────────────────────────────────────
  /** Re-bake up to `budget` dirty in-window squares, LOWEST PRIORITY NUMBER FIRST. Each
   *  square bakes into EVERY channel. Off-window dirties are dropped ({@link markStale}
   *  re-derives them from ownership if the window re-addresses the slot). */
  bakeDirty(renderer: Renderer, budget: number): void {
    this.lastBaked = 0;
    if (!this.scratchRT || this.dirty.size === 0) return;
    const order = [...this.dirty.entries()].sort((a, b) => a[1] - b[1]);
    let baked = 0;
    for (const [key] of order) {
      if (baked >= budget) break;
      this.dirty.delete(key);
      const ci = key.indexOf(",");
      const wc = +key.slice(0, ci);
      const wr = +key.slice(ci + 1);
      if (!this.inWindow(wc, wr)) continue; // owned by a different world square now
      this.bakeSquare(renderer, wc, wr);
      baked++;
    }
    this.lastBaked = baked;
  }

  private inWindow(wc: number, wr: number): boolean {
    return wc >= this.winCol && wc < this.winCol + this.cols && wr >= this.winRow && wr < this.winRow + this.rows;
  }

  /** The prims overlapping square `(wc,wr)` — every prim whose AABB reaches it is linked
   *  here, so a straddling prim bakes into each square it crosses (adjacent slots join
   *  seamlessly with no gutter). Shared across channels. */
  private gather(wc: number, wr: number): Primitive[] {
    const ids = this.squarePrims.get(sqKey(wc, wr));
    if (!ids) return [];
    const list: Primitive[] = [];
    for (const id of ids) {
      const e = this.prims.get(id);
      if (e) list.push(e.prim);
    }
    return list;
  }

  /** Bake ONE world square into EVERY channel. Gather + z-sort + slot ownership + the
   *  world→slot transform + apron targets are computed ONCE; then per channel the prims
   *  are re-textured via `resolve`, rendered (scaled) into the shared scratch, and blitted
   *  to that channel's slot (interior + apron border). */
  private bakeSquare(renderer: Renderer, wc: number, wr: number): void {
    const { cols, rows, slotPx, active } = this;
    const sx = mod(wc, cols);
    const sy = mod(wr, rows);
    // The grid is inset by one slot for the wrap-apron ring: interior slot at (sx+1,sy+1).
    const slotX = (sx + 1) * slotPx;
    const slotY = (sy + 1) * slotPx;
    const si = sy * cols + sx;
    this.slotOwnerCol[si] = wc;
    this.slotOwnerRow[si] = wr;
    this.slotBaked[si] = 1;
    const list = this.gather(wc, wr);
    list.sort((a, b) => a.zIndex - b.zIndex || a.id - b.id);

    // Scale world → slot and move the square's world origin to the scratch origin.
    const s = slotPx / SQUARE;
    const m = new Matrix(s, 0, 0, s, -squareWorldX(wc) * s, -squareWorldY(wr) * s);
    // Apron: an interior EDGE slot is also blitted to the OPPOSITE border (a corner hits
    // both + the diagonal), so the edge quad's outward bilinear reads its wrap-neighbour.
    const ax = sx === 0 ? (cols + 1) * slotPx : sx === cols - 1 ? 0 : -1;
    const ay = sy === 0 ? (rows + 1) * slotPx : sy === rows - 1 ? 0 : -1;

    for (const ch of this.channels) {
      const target = ch.bufs![active];
      this.bakeContainer.removeChildren();
      for (let i = 0; i < list.length; i++) {
        const prim = list[i];
        const r = ch.resolve(prim);
        // Material prims (the single real-tier albedo path) draw through the reconstruction
        // mesh; surface prims through the presence-premultiply mesh; depth prims through the
        // tile-depth mesh; normal prims through the soft-α normal mesh; everything else is a
        // flat tinted sprite (geo tier / untextured) as before.
        const node = r.material
          ? this.materialNode(i, prim, r, r.material)
          : r.surface
            ? this.surfaceNode(i, prim, r)
            : r.depth !== undefined
              ? this.depthNode(i, prim, r, r.depth)
              : r.normal
                ? this.normalNode(i, prim, r, r.normal)
                : this.spriteNode(i, prim, r);
        this.bakeContainer.addChild(node);
      }
      renderer.render({ container: this.bakeContainer, target: this.scratchRT!, clear: true, clearColor: [0, 0, 0, 1], transform: m }); // opaque clear — every bake outputs α=1
      this.blit(renderer, this.scratchTex!, slotX, slotY, target);
      if (ax >= 0) this.blit(renderer, this.scratchTex!, ax, slotY, target);
      if (ay >= 0) this.blit(renderer, this.scratchTex!, slotX, ay, target);
      if (ax >= 0 && ay >= 0) this.blit(renderer, this.scratchTex!, ax, ay, target);
    }
  }

  /** Mirror the west facing: shift the origin by the width so the flipped quad still
   *  occupies `[x, x+width]` in world px (the caller has already made `scale.x` negative). */
  private placeFlipped(node: Sprite | Mesh, prim: Primitive): void {
    if (prim.flipX) {
      node.scale.x = -Math.abs(node.scale.x);
      node.position.set(prim.x + prim.width, prim.y);
    } else {
      node.position.set(prim.x, prim.y); // world px; the transform maps → slot-local
    }
  }

  /** The flat-tint path: a pooled sprite showing this channel's resolved texture + tint.
   *  `Sprite.width/height` sets scale relative to the texture size. */
  private spriteNode(i: number, prim: Primitive, r: ResolvedPrim): Sprite {
    let sprite = this.bakePool[i];
    if (!sprite) {
      sprite = new Sprite();
      this.bakePool[i] = sprite;
    }
    sprite.texture = r.texture;
    sprite.tint = r.tint;
    sprite.width = prim.width;
    sprite.height = prim.height;
    this.placeFlipped(sprite, prim);
    return sprite;
  }

  /** The material path: a pooled mesh whose {@link MaterialBakeShader} reconstructs the
   *  albedo with per-material hue/chroma variation. Its `width`/`height` set the unit-quad
   *  scale; the shader reads the prim's world rect for world-space noise. */
  private materialNode(i: number, prim: Primitive, r: ResolvedPrim, mat: MaterialResolve): Mesh {
    if (!this.materialQuad) {
      this.materialQuad = new MeshGeometry({
        positions: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      });
    }
    let mesh = this.materialPool[i];
    if (!mesh) {
      mesh = new Mesh({ geometry: this.materialQuad, shader: makeMaterialBakeShader() });
      this.materialPool[i] = mesh;
    }
    const shader = mesh.shader as MaterialBakeShader;
    shader.residual = mat.residual; // the `albedo` map — the reconstruction base (RGB)
    shader.layers = mat.layers; // weight map, or null → residual only
    shader.surface = mat.surface; // its B channel supplies the visual alpha
    shader.noise = this.noiseTex ?? Texture.EMPTY; // noise optional (identity 0.5 when absent)
    shader.setChannels(mat.chA, mat.chB);
    const [rows, uvTile, worldTile] = this.noiseGlobals;
    shader.setNoiseGlobals(rows, uvTile, worldTile);
    shader.setWorldRect(prim.x, prim.y, prim.width, prim.height);
    shader.setSeed(prim.seed ?? 0);
    // Unit-quad mesh: scale IS the world-px size (no texture-size division). Flip in place.
    mesh.scale.set(prim.width, prim.height);
    this.placeFlipped(mesh, prim);
    return mesh;
  }

  /** The depth path: a pooled mesh whose {@link DepthBakeShader} writes this prim's constant
   *  tile depth into B where the sprite is ≥95% opaque. Shares the unit quad + placement with
   *  {@link materialNode}. */
  private depthNode(i: number, prim: Primitive, r: ResolvedPrim, depth: number): Mesh {
    if (!this.materialQuad) {
      this.materialQuad = new MeshGeometry({
        positions: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      });
    }
    let mesh = this.depthPool[i];
    if (!mesh) {
      mesh = new Mesh({ geometry: this.materialQuad, shader: makeDepthBakeShader() });
      this.depthPool[i] = mesh;
    }
    const shader = mesh.shader as DepthBakeShader;
    shader.texture = r.texture;
    shader.tileDepth = depth;
    mesh.scale.set(prim.width, prim.height);
    this.placeFlipped(mesh, prim);
    return mesh;
  }

  /** The normal path: a pooled mesh whose {@link NormalBakeShader} writes the prim's normal
   *  (real LOD, or flat-up) masked by the silhouette alpha, so its flat background never
   *  clobbers a thing behind. Shares the unit quad + placement with {@link materialNode}. */
  private normalNode(i: number, prim: Primitive, r: ResolvedPrim, nrm: { rgb: Texture | null; alpha: Texture }): Mesh {
    if (!this.materialQuad) {
      this.materialQuad = new MeshGeometry({
        positions: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      });
    }
    let mesh = this.normalPool[i];
    if (!mesh) {
      mesh = new Mesh({ geometry: this.materialQuad, shader: makeNormalBakeShader() });
      this.normalPool[i] = mesh;
    }
    const shader = mesh.shader as NormalBakeShader;
    shader.alpha = nrm.alpha; // surface coverage source (also the mesh's main texture)
    shader.normalTex = nrm.rgb;
    mesh.scale.set(prim.width, prim.height);
    this.placeFlipped(mesh, prim);
    return mesh;
  }

  /** The surface path: a pooled mesh whose {@link SurfaceBakeShader} presence-premultiplies the
   *  surface source (RGB) so the composite carries A = presence. Shares the unit quad +
   *  placement with {@link materialNode}. */
  private surfaceNode(i: number, prim: Primitive, r: ResolvedPrim): Mesh {
    if (!this.materialQuad) {
      this.materialQuad = new MeshGeometry({
        positions: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        uvs: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]),
        indices: new Uint32Array([0, 1, 2, 0, 2, 3]),
      });
    }
    let mesh = this.surfacePool[i];
    if (!mesh) {
      mesh = new Mesh({ geometry: this.materialQuad, shader: makeSurfaceBakeShader() });
      this.surfacePool[i] = mesh;
    }
    const shader = mesh.shader as SurfaceBakeShader;
    shader.texture = r.texture; // the surface source (RGB)
    mesh.scale.set(prim.width, prim.height);
    this.placeFlipped(mesh, prim);
    return mesh;
  }

  /** Copy the full `slotPx` scratch into `target` at `(dx,dy)`, verbatim (blendMode
   *  "none"), so it replaces whatever the slot held. */
  private blit(renderer: Renderer, srcTex: Texture, dx: number, dy: number, target: RenderTexture): void {
    const f = srcTex.frame;
    f.x = 0;
    f.y = 0;
    f.width = this.slotPx;
    f.height = this.slotPx;
    srcTex.updateUvs();
    this.blitSprite.texture = srcTex;
    this.blitSprite.position.set(dx, dy);
    this.blitSprite.width = this.slotPx;
    this.blitSprite.height = this.slotPx;
    renderer.render({ container: this.blitSprite, target, clear: false });
  }

  // ── display ──────────────────────────────────────────────────────────────────
  /**
   * Fill the display mesh: ONE quad per resident world square (`cols × rows`), sampling
   * each slot's atlas region (offset +1 past the apron). Shared across channels — the
   * viewport samples whichever channel's composite through this same geometry. A slot
   * never baked for the square being drawn is a ZERO-AREA quad (draws nothing).
   *
   * `pos`/`uv` are `displayQuadCount·8`-element Float32Arrays (4 verts × xy).
   */
  fillDisplay(panX: number, panY: number, pos: Float32Array, uv: Float32Array): void {
    // Sample the DISPLAYED layer — the held `prev` during a swap (so we draw the ready
    // outgoing grid, not the incoming one still baking), else the live grid.
    const L = this.prev;
    const cols = L ? L.cols : this.cols;
    const rows = L ? L.rows : this.rows;
    const slotPx = L ? L.slotPx : this.slotPx;
    const winCol = L ? L.winCol : this.winCol;
    const winRow = L ? L.winRow : this.winRow;
    const ownerCol = L ? L.ownerCol : this.slotOwnerCol;
    const ownerRow = L ? L.ownerRow : this.slotOwnerRow;
    const baked = L ? L.baked : this.slotBaked;
    // UVs normalise to the FIXED buffer size (every channel's pair shares it).
    const cw = this.fixedCW;
    const chh = this.fixedCH;
    let b = 0;
    for (let wc = winCol; wc < winCol + cols; wc++) {
      const sx = mod(wc, cols);
      const x0 = wc * SQUARE + panX;
      const x1 = x0 + SQUARE;
      const ix = (sx + 1) * slotPx; // +1: the interior grid is inset past the apron
      const u0 = ix / cw;
      const u1 = (ix + slotPx) / cw;
      for (let wr = winRow; wr < winRow + rows; wr++) {
        const sy = mod(wr, rows);
        const si = sy * cols + sx;
        // Skip negative world squares (off the origin-anchored world — never baked/owned)
        // and any slot that doesn't currently hold this world square — draw nothing here.
        if (wc < 0 || wr < 0 || baked[si] === 0 || ownerCol[si] !== wc || ownerRow[si] !== wr) {
          for (let k = 0; k < 8; k++) { pos[b + k] = 0; uv[b + k] = 0; }
          b += 8;
          continue;
        }
        const y0 = wr * SQUARE + panY;
        const y1 = y0 + SQUARE;
        const iy = (sy + 1) * slotPx; // +1: apron inset
        const v0 = iy / chh;
        const v1 = (iy + slotPx) / chh;
        pos[b] = x0; pos[b + 1] = y0; pos[b + 2] = x1; pos[b + 3] = y0;
        pos[b + 4] = x1; pos[b + 5] = y1; pos[b + 6] = x0; pos[b + 7] = y1;
        uv[b] = u0; uv[b + 1] = v0; uv[b + 2] = u1; uv[b + 3] = v0;
        uv[b + 4] = u1; uv[b + 5] = v1; uv[b + 6] = u0; uv[b + 7] = v1;
        b += 8;
      }
    }
  }

  /** Drop every channel's buffers + scratch + index. */
  destroy(): void {
    this.prev = null; // channel buffers are freed below, not here
    for (const ch of this.channels) ch.destroy();
    this.scratchRT?.destroy(true);
    this.scratchTex?.destroy();
    for (const s of this.bakePool) s.destroy();
    this.bakePool.length = 0;
    for (const mesh of this.materialPool) (mesh.shader as MaterialBakeShader).destroy();
    for (const mesh of this.materialPool) mesh.destroy();
    this.materialPool.length = 0;
    for (const mesh of this.depthPool) (mesh.shader as DepthBakeShader).destroy();
    for (const mesh of this.depthPool) mesh.destroy();
    this.depthPool.length = 0;
    for (const mesh of this.normalPool) (mesh.shader as NormalBakeShader).destroy();
    for (const mesh of this.normalPool) mesh.destroy();
    this.normalPool.length = 0;
    for (const mesh of this.surfacePool) (mesh.shader as SurfaceBakeShader).destroy();
    for (const mesh of this.surfacePool) mesh.destroy();
    this.surfacePool.length = 0;
    this.materialQuad?.destroy();
    this.blitSprite.destroy();
    this.bakeContainer.destroy();
    this.empty.destroy();
    this.prims.clear();
    this.squarePrims.clear();
    this.dirty.clear();
  }
}
