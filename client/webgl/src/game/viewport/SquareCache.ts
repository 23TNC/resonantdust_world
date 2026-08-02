//! The square cache (webgl port) — a FIXED-SIZE, toroidal render-texture cache of the world,
//! baked from primitives on the {@link SQUARE} grid. Channel-agnostic: it bakes N sibling
//! CHANNELS (albedo/normal/surface/zdepth) from ONE shared prim index, window, dirty queue and
//! wrap apron via a merged MRT bake. Ported from the pixijs `SquareCache`; the toroidal window /
//! dirty / apron / fillDisplay logic is identical, only the GPU ops change (Pixi RenderTexture/
//! Mesh/Sprite → engine RenderTarget/Program/draw).
//!
//! SIMPLIFICATION vs pixijs (W4c): one buffer per channel (no ping-pong) and NO reproject — a
//! zoom/partition-level change clears + re-bakes the whole window (a brief flash on zoom) instead
//! of reprojecting the old content. Pan is unaffected (leading-edge re-bake only). The ping-pong +
//! reproject smooth path is a deferred follow-up.
//!
//! COORD CONVENTION: composite + scratch pixel coords have y increasing the same way GL texture
//! rows do; the display's uProjection maps world→screen. Any net Y flip is corrected there.
//!
//! MAP MODEL: these channels are the **textile_square maps** (map-model.md) — `SQUARE` textiles/tile,
//! 1 textile = 1 px — on the shared toroidal `cols × rows` TILE window ({@link window}) that the
//! textile_tile maps (presence/buckets/dirty) and the textile_unit shadow map align to tile-for-tile.
//! Only deviation from pure `cols·SQUARE × rows·SQUARE`: each slot carries a {@link PAD} seam gutter,
//! so the atlas pitch is `SLOT_PW = SQUARE + 2·PAD` (sampling uses the interior only).

import { Renderer, RenderTarget, Program, Geometry, Texture, TexFrame } from "../../gl";
import { MrtBakeShader } from "./mrtBakeShader";
import type { PackedChannel } from "./material";
import {
  partitionForZoom,
  mod,
  sameRange,
  SLOTS_X,
  SLOTS_Y,
  SQUARE,
  squaresForAABB,
  squareWorldX,
  squareWorldY,
  type SquareRange,
} from "./squareMath";

/** A renderable occupant of the map. Baked into every square its AABB overlaps. */
export interface Primitive {
  readonly id: number;
  texture: Texture;
  textureName?: string;
  x: number;
  y: number;
  width: number;
  height: number;
  tint: number;
  geoColor?: number;
  flipX?: boolean;
  cell?: number;
  packed?: readonly PackedChannel[];
  seed?: number;
  zIndex: number;
  /** z-positioning P3: world px this prim sits ABOVE the ground plane, 0 for anything standing on
   *  it. `y` is ALREADY shifted north by this much (the draw applies elevation 1:1), so the ground
   *  row is `y + elevation`. That recovery is the whole point: a drawn position alone cannot tell
   *  an elevated part from one genuinely further north, and the shadow system needs to know which. */
  elevation?: number;
  /** pawn-render P2: the prim's TEMPERATURE — true for movers (warm-cache prims). A hot prim's
   *  light/shadow participation lands ONLY in the HOT maps (the ratified tier matrix: any hot
   *  participant → hot); its record carries the same class bit hot lights use. */
  hot?: boolean;
  /** pawn-render P4: the TRUE cardinal rotation (0=s 1=e 2=n 3=w) for the shadow/light record.
   *  Absent ⇒ the legacy e/w derivation (flipX ? 3 : 1). The FRAME already follows the facing
   *  (the def swaps per stem); this code feeds the record's mirror/orientation field. */
  rotation?: number;
  /** tile-lighting F2: a GROUND tile (zIndex 0) that PARTICIPATES in the lighting class —
   *  authored via the `&tile.height` lane. `standingPrims` includes it despite the standing
   *  (zIndex ≥ 1) threshold, so it mints receiver + caster records like a thing. */
  litTile?: boolean;
  /** P5 — the LIGHT presentation of this primitive, if it has one. A primitive presents as a
   *  billboard (the fields above), a light (this), or **both**: a torch is one placed object with a
   *  sprite and a glow. Carried under the SAME carrier prim as the billboard ([F9](../../../../docs/work/2026-07-25-primitive-graph/forks.md)),
   *  so it needs no second delivery list. Absent ⇒ nothing about the records changes. */
  light?: PrimitiveLight;
  /** human-pawns P5 — this billboard is a CARRIED PIECE of another prim's carrier: the value is
   *  the OWNER billboard's prim id (a pawn's head names its body). Its `billboard_data` leaf
   *  parents on that carrier (`prim_data` holds up to 4 pieces — "a pawn = prim{head, body, …}",
   *  VARIABLES.md) instead of minting a degenerate root of its own. Absent ⇒ a root as ever. */
  carrierOf?: number;
  /** human-pawns P5 — the carried piece's LAYER (`billboard_data.G` bits 28–31): the pawn part
   *  slot (body 0, head 1). One object per layer among a prim's pieces. */
  layer?: number;
}

/** The light a primitive carries. Per-KIND in content ([F8](../../../../docs/work/2026-07-25-primitive-graph/forks.md)),
 *  so every torch shares these values; world px / 0..1 colour, converted to units at the record. */
export interface PrimitiveLight {
  color: readonly [number, number, number];
  intensity: number;
  reach: number;          // world px — how far it throws
  emitterRadius: number;  // world px — penumbra softness (0 ⇒ hard shadows)
  height: number;         // world px above the ground plane
  castShadows: boolean;
  hot: boolean;           // animates per frame (flicker/motion) ⇒ the HOT class
  /** lighting-feel P2: emits decay-lightmap flicker particles (`&thing.light.flicker`). */
  flicker?: boolean;
}

/** What a channel's resolve returns — the merged bake gathers albedo material + normal + depth. */
export interface ResolvedPrim {
  texture: Texture;
  tint: number;
  material?: MaterialResolve;
  depth?: number;
  /** The normal map as an atlas sub-frame (or null → flat-up fallback). */
  normal?: { rgb: TexFrame | null };
  surface?: boolean;
}

/** A prim's albedo material — each map is an atlas sub-frame ({@link TexFrame}); the bake reads its
 *  `uvRect()` so a real sprite samples its packed region and the geo tier samples the whole white fill. */
export interface MaterialResolve {
  residual: TexFrame;
  layers: TexFrame | null;
  surface: TexFrame;
  chA: Float32Array;
  chB: Float32Array;
  /** material-system P1: per-channel normal-detail (field row, amp, scale, placement mode). */
  chC: Float32Array;
}

export type PrimitiveSpec = Omit<Primitive, "id" | "tint" | "zIndex"> &
  Partial<Pick<Primitive, "tint" | "zIndex">>;

export interface ChannelSpec {
  key: string;
  resolve: (prim: Primitive) => ResolvedPrim;
}

interface PrimEntry {
  prim: Primitive;
  range: SquareRange;
}

const sqKey = (c: number, r: number): string => `${c},${r}`;

// SIZE_STEP + RESERVE_CSS are RETIRED: the buffers no longer derive from the screen, so there is
// nothing to quantise and nothing to reserve. The grid is SLOTS_X × SLOTS_Y tiles-worth of texels at
// every zoom (work 2026-07-26-textile-slot).
const PRIO_HIGH = 0;

// ── the reproject program (P3) — carry content ACROSS a level change instead of clearing ─────────
// A level change rescales every tile (`slotPx` halves or doubles) AND permutes its slot (the torus
// modulus is `cols`, which also changes). One pass per channel maps DESTINATION texel → source texel
// arithmetically, so the whole grid moves in a single draw instead of a blit per tile (24 576 of them
// at level 3).
//
// NEAREST by construction — `texelFetch`, no filtering. Mandatory, not preference: `zdepth` encodes a
// discrete `0x80 | baseRow` that averaging corrupts silently, and the lightmap must stay exactly
// representable to remain invertible (forks F3).
//
// A destination tile outside the OLD window has no source and returns black; the CPU marks those dirty.
// Both windows are centred on the same anchor, so the old tile range is a contiguous sub-range of the
// new one (zoom out) or a superset of it (zoom in) — a range test is sufficient, no validity texture.
const REPROJ_FRAG = /* glsl */ `#version 300 es
precision highp float;
uniform sampler2D uSrc;
uniform vec2 uSrcSize;        // source buffer px
uniform vec4 uOldWin;         // oldWinCol, oldWinRow, oldCols, oldRows
uniform vec4 uNewWin;         // newWinCol, newWinRow, newCols, newRows
uniform vec2 uSlotPx;         // oldSlotPx, newSlotPx
out vec4 fragColor;

// True modulo — the torus wrap; GLSL mod() on negatives would index outside the composite.
float tmod(float n, float m) { return mod(mod(n, m) + m, m); }

void main() {
  vec2 fc = floor(gl_FragCoord.xy);
  float nSlot = uSlotPx.y, oSlot = uSlotPx.x;
  // Destination slot index (the composite carries a 1-slot apron, hence the -1) + offset within it.
  vec2 dslot = floor(fc / nSlot) - 1.0;
  vec2 doff  = fc - (dslot + 1.0) * nSlot;
  if (dslot.x < 0.0 || dslot.y < 0.0 || dslot.x >= uNewWin.z || dslot.y >= uNewWin.w) { fragColor = vec4(0.0, 0.0, 0.0, 1.0); return; }
  // Slot → WORLD tile, via the new window's own wrap.
  vec2 baseN = vec2(tmod(uNewWin.x, uNewWin.z), tmod(uNewWin.y, uNewWin.w));
  vec2 wt = uNewWin.xy + mod(dslot - baseN + uNewWin.zw, uNewWin.zw);
  // In the OLD window? Its tiles are a contiguous world range; outside it there is nothing to carry.
  vec2 rel = wt - uOldWin.xy;
  if (rel.x < 0.0 || rel.y < 0.0 || rel.x >= uOldWin.z || rel.y >= uOldWin.w) { fragColor = vec4(0.0, 0.0, 0.0, 1.0); return; }
  // WORLD tile → old slot → old texel. The within-tile offset rescales by the slot ratio, which is what
  // performs the up/downscale (replication when growing, decimation when shrinking).
  vec2 sslot = vec2(tmod(wt.x, uOldWin.z), tmod(wt.y, uOldWin.w));
  vec2 soff  = floor(doff * (oSlot / nSlot));
  vec2 src   = (sslot + 1.0) * oSlot + soff;
  fragColor = texelFetch(uSrc, ivec2(clamp(src, vec2(0.0), uSrcSize - 1.0)), 0);
}
`;
const PRIO_STD = 256;
const RING_MAX = 255;

// ── the blit program (scratch attachment → composite slot; verbatim copy) ───────────
const BLIT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;             // unit quad 0..1
uniform vec4 uDst;             // dest NDC rect: x0, y0, w, h
uniform vec4 uSrcRect;         // source UV rect: u0, v0, w, h
out vec2 vUV;
void main() {
  vUV = uSrcRect.xy + aPosition * uSrcRect.zw;
  vec2 ndc = uDst.xy + aPosition * uDst.zw;
  gl_Position = vec4(ndc, 0.0, 1.0);
}
`;
const BLIT_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uSrc;
out vec4 fragColor;
void main() { fragColor = texture(uSrc, vUV); }
`;

/** One bake target: a fixed-size buffer + the resolve hook. */
class Channel {
  readonly key: string;
  readonly resolve: (prim: Primitive) => ResolvedPrim;
  buf: RenderTarget | null = null;

  constructor(spec: ChannelSpec) {
    this.key = spec.key;
    this.resolve = spec.resolve;
  }

  ensureBuffer(cw: number, ch: number, gl: WebGL2RenderingContext): void {
    this.buf?.destroy();
    this.buf = new RenderTarget(gl, { width: cw, height: ch, formats: ["rgba8unorm"] });
    this.buf.clear(0, 0, 0, 1); // opaque (see the stale-slot fix note in pixijs)
  }

  destroy(): void {
    this.buf?.destroy();
    this.buf = null;
  }
}

export class SquareCache {
  private readonly channels: Channel[];
  private fixedCW = 0;
  private fixedCH = 0;
  private mrtScratch: RenderTarget | null = null;

  // role refs (found from keys) — the merged bake gathers these; blit order = [albedo,surface,normal,depth]
  private albedoCh: Channel | undefined;
  private surfaceCh: Channel | undefined;
  private normalCh: Channel | undefined;
  private depthCh: Channel | undefined;

  private cols = 0;
  private rows = 0;
  private slotPx = SQUARE;
  /** Current level (0..3). `-1` = unpartitioned. `cols`/`rows`/`slotPx` all derive from it. */
  private level = -1;

  private winCol = 0;
  private winRow = 0;
  private aimed = false;

  private slotOwnerCol = new Int32Array(0);
  private slotOwnerRow = new Int32Array(0);
  private slotBaked = new Uint8Array(0);

  private nextId = 1;
  private readonly prims = new Map<number, PrimEntry>();
  private readonly squarePrims = new Map<string, Set<number>>();
  private readonly dirty = new Map<string, number>();

  // GPU scratch
  private readonly gl: WebGL2RenderingContext;
  private readonly renderer: Renderer;
  private readonly empty: Texture;
  private readonly blit: Program;
  private readonly reproj: Program;
  private readonly unitQuad: Geometry;
  private readonly bake: MrtBakeShader;

  private noiseTex: Texture | null = null;
  private noiseGlobals: readonly [number, number, number] = [1, 1, 128];

  /** Diagnostics: squares baked on the last {@link bakeDirty}. */
  lastBaked = 0;
  /** DEBUG (textile-slot P6): pending dirty squares + the grid state, for asserting a zoom sweep
   *  without screenshots — a reproject that works dirties ~nothing on zoom-IN. */
  get debugState(): { level: number; cols: number; rows: number; slotPx: number; dirty: number; empty: number; stale: number; baked: number; bufW: number; bufH: number } {
    let baked = 0;
    for (let i = 0; i < this.slotBaked.length; i++) if (this.slotBaked[i] === 1) baked++;
    // P5 evidence: the dirty queue's priority histogram. A priority is `band + ring` (see `prio`), so the
    // BAND is what classifies and the ring only orders within it — EMPTY tiles (never baked / wrong owner)
    // occupy PRIO_HIGH..+RING_MAX, tiles carried across a level change occupy PRIO_STD..+RING_MAX. Split on
    // the band boundary, NOT on the band value. `bakeDirty` sorts ascending, so every empty tile drains
    // before any stale one, and within each class the centre of the screen fills first.
    let empty = 0, stale = 0;
    for (const p of this.dirty.values()) { if (p < PRIO_STD) empty++; else stale++; }
    return { level: this.level, cols: this.cols, rows: this.rows, slotPx: this.slotPx,
             dirty: this.dirty.size, empty, stale, baked, bufW: this.fixedCW, bufH: this.fixedCH };
  }

  constructor(renderer: Renderer, empty: Texture, channels: ChannelSpec[]) {
    this.renderer = renderer;
    this.gl = renderer.gl;
    this.empty = empty;
    this.channels = channels.map((c) => new Channel(c));
    this.albedoCh = this.channels.find((c) => c.key.startsWith("albedo"));
    this.surfaceCh = this.channels.find((c) => c.key.startsWith("surface"));
    this.normalCh = this.channels.find((c) => c.key.startsWith("normal"));
    this.depthCh = this.channels.find((c) => c.key.startsWith("zdepth"));
    this.blit = new Program(this.gl, BLIT_VERT, BLIT_FRAG, "square-blit");
    this.reproj = new Program(this.gl, BLIT_VERT, REPROJ_FRAG, "square-reproject");
    this.unitQuad = new Geometry(this.gl, this.blit, {
      aPosition: { data: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), size: 2 },
    }, new Uint32Array([0, 1, 2, 0, 2, 3]));
    this.bake = new MrtBakeShader(this.gl);
  }

  setNoise(texture: Texture | null, rows: number, uvTile: number, worldTile: number): void {
    this.noiseTex = texture;
    this.noiseGlobals = [rows, uvTile, worldTile];
  }

  /** material-system P4: the global colour-placement override (-1 = per-material). */
  setPlaceMode(m: number): void {
    this.bake.placeMode = m;
  }

  displayComposite(key: string): Texture | null {
    const ch = this.channels.find((c) => c.key === key);
    return ch?.buf?.textures[0] ?? null;
  }

  private prio(wc: number, wr: number, band: number): number {
    const cc = this.winCol + (this.cols >> 1);
    const cr = this.winRow + (this.rows >> 1);
    const ring = Math.min(Math.max(Math.abs(wc - cc), Math.abs(wr - cr)), RING_MAX);
    return band + ring;
  }

  private markDirty(wc: number, wr: number, band: number): void {
    if (wc < 0 || wr < 0) return;
    const k = sqKey(wc, wr);
    const p = this.prio(wc, wr, band);
    const cur = this.dirty.get(k);
    if (cur === undefined || p < cur) this.dirty.set(k, p);
  }

  get ready(): boolean {
    return this.cols > 0 && this.channels.length > 0 && this.channels[0].buf != null;
  }

  get displayQuadCount(): number {
    return this.cols * this.rows;
  }

  /** The toroidal tile window geometry — for a sibling world-space toroidal map (shadow-cold) that must
   *  align tile-for-tile with this cache: `cols`/`rows` tiles (`SLOTS << level`, incl. overscan), the window
   *  origin (`winCol`/`winRow`, in world tiles), the per-tile texel size (`SQUARE >> level`) and the `level`
   *  itself — siblings need the level to derive THEIR own per-tile size from their own texels-per-slot. */
  get window(): { winCol: number; winRow: number; cols: number; rows: number; slotPx: number; level: number } {
    return { winCol: this.winCol, winRow: this.winRow, cols: this.cols, rows: this.rows, slotPx: this.slotPx, level: Math.max(0, this.level) };
  }

  // ── sizing ───────────────────────────────────────────────────────────────────
  resize(screenW: number, screenH: number, zoom: number, anchorX: number, anchorY: number): void {
    if (screenW <= 0 || screenH <= 0) return;
    this.ensureBuffers();

    // FIXED SLOT GRID (work 2026-07-26-textile-slot). The texture never changes size; what varies with
    // zoom is TILES PER SLOT. A slot is SQUARE texels and holds `2^level` tiles per axis, so the tile grid
    // is `SLOTS << level` tiles at `SQUARE >> level` texels each — and those multiply back to the SAME
    // `SLOTS · SQUARE` texels at every level. That identity is why the rest of this class is unchanged:
    // it is still "a uniform grid of `cols × rows` tiles at `slotPx` texels", just with both derived
    // from the level ladder instead of from the screen.
    const level = partitionForZoom(zoom);
    const cols = SLOTS_X << level;
    const rows = SLOTS_Y << level;
    const slotPx = SQUARE >> level;
    if (level === this.level && this.aimed) return;

    // P3: REPROJECT, don't clear. The old content is still valid world data — it just needs rescaling
    // into the new slot layout. Clearing here is what made zoom flash ([I5]).
    const prev = this.level >= 0 && this.aimed
      ? { winCol: this.winCol, winRow: this.winRow, cols: this.cols, rows: this.rows, slotPx: this.slotPx }
      : null;
    this.level = level;
    this.cols = cols;
    this.rows = rows;
    this.slotPx = slotPx;
    this.mrtScratch?.destroy();
    this.mrtScratch = new RenderTarget(this.gl, {
      width: slotPx, height: slotPx,
      formats: ["rgba8unorm", "rgba8unorm", "rgba8unorm", "rgba8unorm"],
    });
    const prevOwnerCol = this.slotOwnerCol, prevOwnerRow = this.slotOwnerRow, prevBaked = this.slotBaked;
    this.slotOwnerCol = new Int32Array(cols * rows);
    this.slotOwnerRow = new Int32Array(cols * rows);
    this.slotBaked = new Uint8Array(cols * rows);
    this.dirty.clear();
    // Aim the window on the anchor. The window ALREADY carries its overscan (SLOTS = VISIBLE +
    // 2·OVERSCAN), so centring the whole grid on the anchor is what puts the slack on every edge.
    this.winCol = Math.floor(anchorX / SQUARE) - (cols >> 1);
    this.winRow = Math.floor(anchorY / SQUARE) - (rows >> 1);
    this.aimed = true;

    let carried = 0;
    if (!prev) {
      // lighting-visual P5: the FIRST partition re-marks every resident square — any
      // invalidateAll that fired before the grid existed (fast IndexedDB texture packs
      // emitting at boot) marked a dirty set this method just cleared. Without this a
      // boot-race leaves the world baked geo forever.
      this.invalidateAll();
    }
    if (prev) {
      this.reproject(prev);
      // Carry each tile's BAKED flag across: a tile that was baked and is still in range keeps its
      // reprojected content (rescaled, so visually correct until re-baked at the new level). Everything
      // else has no source and must bake. This is what turns a whole-grid re-bake into a partial one:
      // zooming IN dirties nothing, zooming OUT dirties only the newly exposed ring.
      for (let sr = 0; sr < rows; sr++) {
        for (let sc = 0; sc < cols; sc++) {
          const wc = this.winCol + ((sc - mod(this.winCol, cols) + cols) % cols);
          const wr = this.winRow + ((sr - mod(this.winRow, rows) + rows) % rows);
          const rc = wc - prev.winCol, rr = wr - prev.winRow;
          if (rc < 0 || rr < 0 || rc >= prev.cols || rr >= prev.rows) continue;
          const pi = mod(wr, prev.rows) * prev.cols + mod(wc, prev.cols);
          if (prevBaked[pi] !== 1 || prevOwnerCol[pi] !== wc || prevOwnerRow[pi] !== wr) continue;
          const si = sr * cols + sc;
          this.slotOwnerCol[si] = wc;
          this.slotOwnerRow[si] = wr;
          this.slotBaked[si] = 1;
          carried++;
          // STALE, not clean: the content is a rescale of the other level's bake, so it must be re-baked
          // at the new level eventually — just at lower priority than a tile with nothing at all (P5).
          this.markDirty(wc, wr, PRIO_STD);
        }
      }
    }
    this.markStale(); // anything still unowned/unbaked → PRIO_HIGH
    // DEBUG (P6): the reproject's own accounting, captured BEFORE bakeDirty drains the queue — which is
    // the only moment the carry-over is observable from outside.
    this.debugReproj = {
      from: prev ? { cols: prev.cols, rows: prev.rows, slotPx: prev.slotPx } : null,
      to: { level, cols, rows, slotPx },
      carried, total: cols * rows, fresh: cols * rows - carried, dirty: this.dirty.size,
      queue: this.debugState, // priority split at the moment of the switch (P5 evidence)
    };
  }

  /** DEBUG (P6): what the last level change carried across vs had to bake fresh. */
  debugReproj: unknown = null;

  /** P3: carry every channel's content across a level change. One full-target pass per channel through a
   *  scratch (the slot permutation makes source and destination overlap arbitrarily, so in-place is
   *  undefined — [I4]), then swap the scratch in as the channel's buffer. */
  private reproject(prev: { winCol: number; winRow: number; cols: number; rows: number; slotPx: number }): void {
    const cw = this.fixedCW, ch = this.fixedCH;
    for (const channel of this.channels) {
      const src = channel.buf;
      if (!src) continue;
      const dst = new RenderTarget(this.gl, { width: cw, height: ch, formats: ["rgba8unorm"] });
      this.renderer.draw({
        program: this.reproj,
        geometry: this.unitQuad,
        target: dst,
        blend: "none",
        textures: { uSrc: src.textures[0] },
        uniforms: (p) => {
          p.uVec4("uDst", -1, -1, 2, 2);
          p.uVec4("uSrcRect", 0, 0, 1, 1);
          p.uVec2("uSrcSize", cw, ch);
          p.uVec4("uOldWin", prev.winCol, prev.winRow, prev.cols, prev.rows);
          p.uVec4("uNewWin", this.winCol, this.winRow, this.cols, this.rows);
          p.uVec2("uSlotPx", prev.slotPx, this.slotPx);
        },
      });
      src.destroy();
      channel.buf = dst;
    }
  }

  /** The channel buffers are FIXED — sized in TILES, not screen px, so they never resize and never track
   *  the player's monitor.
   *
   *  **MODULUS vs STRIDE — these are different numbers and only one of them is pow2 by design.**
   *  The toroidal wrap is `sx = mod(wc, cols)` with `cols = SLOTS_X << level`, i.e. **32 · 2^level** — a
   *  power of two at every level, so the wrap is a bitmask, not an integer division. That is the whole
   *  reason `SLOTS` is 32×16 ([F5](../../../../docs/work/2026-07-26-textile-slot/forks.md#f5)).
   *
   *  The TEXTURE is `SLOTS + 2` slots per axis (34×18). The extra ring is the **WRAP-APRON**, applied
   *  as a `(sx + 1)` offset AFTER the modulus, so it never touches the wrap arithmetic. It exists
   *  because the toroidal window straddles the texture edge: `bakeSquare` mirrors an edge slot to the
   *  opposite border so a square adjacent across the wrap has physically adjacent texels. Sized for the
   *  LARGEST apron (level 0) — the apron is `2 · (SQUARE >> level)`, so higher levels leave margin unused
   *  rather than resizing.
   *
   *  So: a non-pow2 4352×2304 texture costs nothing (WebGL2 handles NPOT fine at NEAREST/CLAMP) while
   *  the address math stays pow2. Do NOT "simplify" the +2 away to make the texture pow2 — that trades
   *  a free physical stride for the wrap correctness the apron buys. */
  private ensureBuffers(): void {
    const cw = (SLOTS_X + 2) * SQUARE;
    const ch = (SLOTS_Y + 2) * SQUARE;
    if (this.channels[0]?.buf && cw === this.fixedCW && ch === this.fixedCH) return;
    this.fixedCW = cw;
    this.fixedCH = ch;
    for (const channel of this.channels) channel.ensureBuffer(cw, ch, this.gl);
    this.level = -1;
    this.cols = 0;
    this.rows = 0;
    this.slotPx = 0;
    this.aimed = false;
    this.dirty.clear();
  }

  // ── window ────────────────────────────────────────────────────────────────────
  recenter(anchorWorldX: number, anchorWorldY: number): void {
    if (!this.mrtScratch) return;
    // Centre the whole tile grid on the anchor — the grid already carries its overscan.
    const newCol = Math.floor(anchorWorldX / SQUARE) - (this.cols >> 1);
    const newRow = Math.floor(anchorWorldY / SQUARE) - (this.rows >> 1);
    if (this.aimed && newCol === this.winCol && newRow === this.winRow) return;
    this.winCol = newCol;
    this.winRow = newRow;
    this.aimed = true;
    this.markStale();
  }

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
          this.markDirty(wc, wr, PRIO_HIGH);
        }
      }
    }
  }

  // ── primitive index ─────────────────────────────────────────────────────────────
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
      light: spec.light,   // P5: the LIGHT presentation — dropped here, a torch never lights
      hot: spec.hot,       // pawn-render P2: the temperature — dropped here, a mover dirties COLD
      rotation: spec.rotation, // pawn-render P4: the true cardinal — dropped here, a RESTING n/s
                               // mover falls back to the e/w derivation (ns-shadows I1)
      litTile: spec.litTile,   // tile-lighting F2: the participation flag — dropped here, a
                               // wall never joins the lighting class (the addPrim gotcha)
      carrierOf: spec.carrierOf, // human-pawns P5: the piece link — dropped here, a head mints
      layer: spec.layer,         // its own root + loses its layer (the addPrim gotcha again)
      elevation: spec.elevation, // z-positioning P3: the HEIGHT — dropped here, every elevated
                                 // part reverts to standing on the floor and casts from the wrong
                                 // footprint, which is the exact bug this stream exists to fix
    };
    const range = squaresForAABB(prim.x, prim.y, prim.x + prim.width, prim.y + prim.height);
    this.prims.set(id, { prim, range });
    this.linkRange(id, range);
    this.dirtyRange(range);
    return id;
  }

  getPrim(id: number): Primitive | null {
    return this.prims.get(id)?.prim ?? null;
  }

  standingPrims(): Primitive[] {
    const out: Primitive[] = [];
    // The zIndex ≥ 1 threshold IS the standing/ground split (tile-lighting P0). The litTile
    // participation gate is PARKED (stream paused 2026-07-29): tiles will enter lighting via
    // `prim_presence` slot 0 carrying their DEFINITION id directly — the texture-
    // generalization redesign — not by joining the standing list as pseudo-billboards.
    for (const { prim } of this.prims.values()) if (prim.zIndex >= 1) out.push(prim);
    return out;
  }

  refreshPrim(id: number): void {
    const e = this.prims.get(id);
    if (!e) return;
    const range = squaresForAABB(e.prim.x, e.prim.y, e.prim.x + e.prim.width, e.prim.y + e.prim.height);
    if (!sameRange(range, e.range)) {
      this.unlinkRange(id, e.range);
      this.dirtyRange(e.range);
      e.range = range;
      this.linkRange(id, range);
    }
    this.dirtyRange(e.range);
  }

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
    for (let c = r.col0; c <= r.col1; c++)
      for (let row = r.row0; row <= r.row1; row++) this.markDirty(c, row, PRIO_STD);
  }

  invalidateAll(): void {
    for (const k of this.squarePrims.keys()) {
      const ci = k.indexOf(",");
      this.markDirty(+k.slice(0, ci), +k.slice(ci + 1), PRIO_STD);
    }
  }

  /** Dirty only the squares holding a prim that draws `stem` — the targeted form of
   *  {@link invalidateAll}, for "one texture landed" rather than "the whole atlas moved".
   *
   *  **Why this exists** (render-performance I11): a texture arrival used to `invalidateAll()`, so
   *  ONE stem landing dirtied all 2048–8192 squares. `bakeDirty` is budgeted, so during load the
   *  queue never drained — the composites lagged permanently behind the lighting, which reads them
   *  every frame. The preview tier made it acute by roughly doubling the arrival rate (geo →
   *  preview → master, plus emit-on-failure). A stem occupies a handful of squares; dirtying the
   *  other few thousand is pure waste that also starves the ones that changed. */
  invalidateStem(stem: string): void {
    for (const [k, ids] of this.squarePrims) {
      let hit = false;
      for (const id of ids) {
        if (this.prims.get(id)?.prim.textureName === stem) { hit = true; break; }
      }
      if (!hit) continue;
      const ci = k.indexOf(",");
      this.markDirty(+k.slice(0, ci), +k.slice(ci + 1), PRIO_STD);
    }
  }

  // ── bake ─────────────────────────────────────────────────────────────────────────
  bakeDirty(budget: number): void {
    this.lastBaked = 0;
    if (!this.mrtScratch || this.dirty.size === 0) return;
    const order = [...this.dirty.entries()].sort((a, b) => a[1] - b[1]);
    let baked = 0;
    for (const [key] of order) {
      if (baked >= budget) break;
      this.dirty.delete(key);
      const ci = key.indexOf(",");
      const wc = +key.slice(0, ci);
      const wr = +key.slice(ci + 1);
      if (!this.inWindow(wc, wr)) continue;
      this.bakeSquare(wc, wr);
      baked++;
    }
    this.lastBaked = baked;
  }

  private inWindow(wc: number, wr: number): boolean {
    return wc >= this.winCol && wc < this.winCol + this.cols && wr >= this.winRow && wr < this.winRow + this.rows;
  }

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

  /** Bake ONE world square into every channel: render all its prims into the 4-attachment MRT
   *  scratch (one draw per prim, placed by uModel), then blit each attachment to the channel's
   *  slot (interior + wrap-apron border). */
  private bakeSquare(wc: number, wr: number): void {
    const { cols, rows, slotPx } = this;
    const sx = mod(wc, cols);
    const sy = mod(wr, rows);
    const slotX = (sx + 1) * slotPx;
    const slotY = (sy + 1) * slotPx;
    const si = sy * cols + sx;
    this.slotOwnerCol[si] = wc;
    this.slotOwnerRow[si] = wr;
    this.slotBaked[si] = 1;
    const list = this.gather(wc, wr);
    list.sort((a, b) => a.zIndex - b.zIndex || a.id - b.id);

    // Wrap-apron: an interior EDGE slot is also written to the OPPOSITE border.
    const ax = sx === 0 ? (cols + 1) * slotPx : sx === cols - 1 ? 0 : -1;
    const ay = sy === 0 ? (rows + 1) * slotPx : sy === rows - 1 ? 0 : -1;

    // One MRT pass: clear the scratch, then draw each prim placed by uModel.
    this.mrtScratch!.clear(0, 0, 0, 1);
    const wcOrigin = squareWorldX(wc);
    const wrOrigin = squareWorldY(wr);
    for (let i = 0; i < list.length; i++) {
      const prim = list[i];
      const albedoR = this.albedoCh!.resolve(prim);
      const normalR = this.normalCh!.resolve(prim);
      const depthR = this.depthCh!.resolve(prim);
      const mat = albedoR.material!; // B2: albedo always resolves to a material
      this.bake.setTint(albedoR.tint);
      // Each material map is an atlas sub-frame — pass its page texture + UV rect so a real sprite
      // samples only its packed region (geo maps are the whole white fill → identity rect).
      this.bake.setResidual(mat.residual.source, mat.residual.uvRect());
      this.bake.setLayers(mat.layers?.source ?? null, mat.layers?.uvRect());
      this.bake.setSurface(mat.surface.source, mat.surface.uvRect());
      this.bake.setNoise(this.noiseTex);
      this.bake.setChannels(mat.chA, mat.chB, mat.chC);
      const [nrows, uvTile, worldTile] = this.noiseGlobals;
      this.bake.setNoiseGlobals(nrows, uvTile, worldTile);
      this.bake.setWorldRect(prim.x, prim.y, prim.width, prim.height);
      // material-system P2: the bake consumes the SAME u8-quantised seed the billboard_data record
      // stamps (VARIABLES B bits 0–7) — one source, bit-agreeing on both sides.
      this.bake.setSeed(Math.floor((prim.seed ?? 0) * 255) / 255);
      this.bake.setNormal(normalR.normal?.rgb?.source ?? null, normalR.normal?.rgb?.uvRect());
      this.bake.setTileDepth(depthR.depth ?? -1);
      const model = this.primModel(prim, wcOrigin, wrOrigin);
      this.renderer.draw({
        program: this.bake.program,
        geometry: this.unitQuad,
        target: this.mrtScratch!,
        blend: "normal",
        textures: this.bake.textures(this.empty),
        uniforms: (p) => {
          p.uMat3("uModel", model);
          this.bake.apply(p);
        },
      });
    }
    // Blit each attachment to its channel's slot (interior + apron).
    const order = [this.albedoCh!, this.surfaceCh!, this.normalCh!, this.depthCh!];
    for (let k = 0; k < 4; k++) {
      const tex = this.mrtScratch!.textures[k];
      const target = order[k].buf!;
      this.blitSlot(tex, slotX, slotY, target);
      if (ax >= 0) this.blitSlot(tex, ax, slotY, target);
      if (ay >= 0) this.blitSlot(tex, slotX, ay, target);
      if (ax >= 0 && ay >= 0) this.blitSlot(tex, ax, ay, target);
    }
  }

  /** unit quad (0..1) → the prim's world rect → this square's scratch NDC (world square fills
   *  the slotPx scratch, so NDC = (worldLocal/SQUARE)*2 - 1). flipX mirrors about the prim x. */
  private primModel(prim: Primitive, wcOrigin: number, wrOrigin: number): Float32Array {
    const k = 2 / SQUARE;
    let a = prim.width * k;
    let tx = prim.x * k - (wcOrigin * k + 1);
    if (prim.flipX) {
      a = -a;
      tx = (prim.x + prim.width) * k - (wcOrigin * k + 1);
    }
    const d = prim.height * k;
    const ty = prim.y * k - (wrOrigin * k + 1);
    // column-major mat3: col0=(a,0,0) col1=(0,d,0) col2=(tx,ty,1)
    return new Float32Array([a, 0, 0, 0, d, 0, tx, ty, 1]);
  }

  /** Copy the whole slotPx scratch into `target` at composite px (dx,dy), verbatim. */
  private blitSlot(srcTex: Texture, dx: number, dy: number, target: RenderTarget): void {
    const w = this.slotPx / this.fixedCW;
    const h = this.slotPx / this.fixedCH;
    const x0 = (dx / this.fixedCW) * 2 - 1;
    const y0 = (dy / this.fixedCH) * 2 - 1;
    this.renderer.draw({
      program: this.blit,
      geometry: this.unitQuad,
      target,
      blend: "none",
      textures: { uSrc: srcTex },
      uniforms: (p) => {
        p.uVec4("uDst", x0, y0, w * 2, h * 2);
        p.uVec4("uSrcRect", 0, 0, 1, 1);
      },
    });
  }

  // ── display ────────────────────────────────────────────────────────────────────
  /** Fill the display mesh: ONE quad per resident world square, sampling each slot's atlas
   *  region (offset +1 past the apron). A slot never baked for the square is a zero-area quad. */
  fillDisplay(panX: number, panY: number, pos: Float32Array, uv: Float32Array): void {
    const { cols, rows, slotPx, winCol, winRow } = this;
    const ownerCol = this.slotOwnerCol;
    const ownerRow = this.slotOwnerRow;
    const baked = this.slotBaked;
    const cw = this.fixedCW;
    const chh = this.fixedCH;
    let b = 0;
    for (let wc = winCol; wc < winCol + cols; wc++) {
      const sx = mod(wc, cols);
      const x0 = wc * SQUARE + panX;
      const x1 = x0 + SQUARE;
      const ix = (sx + 1) * slotPx;
      const u0 = ix / cw;
      const u1 = (ix + slotPx) / cw;
      for (let wr = winRow; wr < winRow + rows; wr++) {
        const sy = mod(wr, rows);
        const si = sy * cols + sx;
        if (wc < 0 || wr < 0 || baked[si] === 0 || ownerCol[si] !== wc || ownerRow[si] !== wr) {
          for (let kk = 0; kk < 8; kk++) { pos[b + kk] = 0; uv[b + kk] = 0; }
          b += 8;
          continue;
        }
        const y0 = wr * SQUARE + panY;
        const y1 = y0 + SQUARE;
        const iy = (sy + 1) * slotPx;
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

  destroy(): void {
    for (const ch of this.channels) ch.destroy();
    this.mrtScratch?.destroy();
    this.blit.destroy();
    this.unitQuad.destroy();
    this.bake.destroy();
    this.prims.clear();
    this.squarePrims.clear();
    this.dirty.clear();
  }
}
