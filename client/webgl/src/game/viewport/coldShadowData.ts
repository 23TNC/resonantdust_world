//! ColdShadowData — the ONE unified `RGBA32UI` data texture the shadow gather reads via `texelFetch`
//! (`2026-07-23-unified-data`). 1024×1024, linear index `i → (i & 1023, i >> 10)`, 64-row bands of
//! 65 536 one-texel slots each; bit layouts authoritative in `docs/VARIABLES.md` §Cold shadow data:
//!
//!   rows   0–63    billboard_definition_data  (band base 0)       1 px/def — IMMUTABLE, keyed (stem,cell,lod)
//!   rows  64–127   billboard_data             (band base 65 536)  1 px/placed caster (2/px retired — F1)
//!   rows 128–191   light_data            (band base 131 072) 1 px/light record
//!   rows 192–1022  reserved              (materials era)
//!   row  1023      constants             (window mapping — P3)
//!
//! Transport: dirty ROW-SPAN `texSubImage2D` (P1; the P2 command-buffer scatter replaces it).
//! World unit: `1 unit = SQUARE/16 = 4px`, compile-time (§Unit). Every world field is in units; only the
//! atlas `frame_*` are texture px. Positions pack as `position_anchor_reference` (region|zone|tile|anchor).

import { Renderer, Texture, Program, Geometry, RenderTarget } from "../../gl";
import type { Primitive, PrimitiveLight } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE, UNIT, ZONE_DIM, REGION_DIM } from "./squareMath";

/** Fixed light count (bits in the shadow bitfield — the light BAND holds 65 536 slots of headroom). */
export const N_LIGHTS = 128;
/** The unified data texture: 1024×1024, linear index i → (i & 1023, i >> 10). */
export const DATA_W = 1024;
export const DATA_H = 1024;
/** Band bases (linear indices) — 64-row bands of 65 536 one-texel slots. Mirrored in the GLSL. */
export const DEF_BASE = 0;
export const PRIM_BASE = 65536;            // set 1 — prim_data, the composition node
export const BILLBOARD_DATA_BASE = 6 * 65536; // set 6 — billboard_data, the sprite leaf
/** A prim's `set_a..d` nibble naming the band a carried piece lives in (0 = "no data"). */
const SET_BILLBOARD_DATA = 6;
const SET_LIGHT_DATA = 2;
const SET_PRIM_DATA = 1;   // a prim carrying another prim — what makes the graph a graph
/** How deep the carrier chain may go before the resolve walk gives up (I12). A pawn → hand → tool →
 *  light is 4; 8 is generous headroom and makes a malformed cycle cost a bounded walk, not a hang. */
const MAX_PRIM_DEPTH = 8;
/** The bias-8 signed nibble pair meaning "no offset" — a carried piece sitting exactly on its carrier. */
const OFFSET_ZERO = 0x88;
export const LIGHT_BASE = 131072;
/** Tile-keyed sets — region-torus addressed (presence-in-data). One set = one region's 65 536 tiles. */
export const PRESENCE_BASE = 3 * 65536;    // light_presence_lo (light slots 0–6)
export const BILLBOARD_PRESENCE_BASE = 4 * 65536;
export const PRESENCE_HI_BASE = 5 * 65536; // light_presence_hi (light slots 7–13)
/** The constants row (row 1023) — px0 window mapping, px1 slot/light-count (P3). */
export const CONST_BASE = 1023 * DATA_W;

/** Region-torus **zone-strip fold**: world tile (wc, wr) → its in-set id (0..65535), a zone's 256
 *  tiles contiguous in one row. In-region zone coords carry no region bits (window ≤ region ⟹ no
 *  residue collision on screen). MUST match the GLSL `foldTile`. */
export function foldTile(wc: number, wr: number): number {
  const zx = ((Math.floor(wc / 16) % 16) + 16) % 16;
  const zy = ((Math.floor(wr / 16) % 16) + 16) % 16;
  const tx = ((wc % 16) + 16) % 16;
  const ty = ((wr % 16) + 16) % 16;
  return ((zx >> 2) + zy * 4) * 1024 + (zx & 3) * 256 + ty * 16 + tx;
}
/** Command buffer v3 (user format, work 2026-07-25-primitive-graph): 64×64 RGBA32UI, replay-idempotent
 *  absolute writes. One COMMAND = a fixed 8 px (128 B) targeting ONE set:
 *    px 0 header  R: u8 operation (24–31) | u5 set (19–23) | u3 count (16–18) | u16 id0 (0–15)
 *                 G: id1|id2   B: id3|id4   A: id5|id6            (7 target ids, high|low)
 *    px 1–7       up to 7 payload records, written to (set, id0 .. id_{count-1})
 *  8 commands tile a 64-px row = 56 record-writes/row; 512 commands per buffer. The `operation`
 *  DEFINES the rest of the command, so future 8-px ops (presence writes, bulk clears) take their own
 *  codes. `count` marks the live ids, so a partial command needs no sentinel and `id = 0` stays
 *  writable — required, since the tile-keyed sets address by `foldTile` (range 0..65535 exhaustive,
 *  so fold 0 is a real tile). Replaces v2's per-set count header + self-addressing payloads. */
const CMD_W = 64;
const CMD_H = 64;
/** v3 command geometry: one command is a header px + {@link IDS_PER_CMD} payload px, fixed stride, so
 *  commands tile the row exactly (64 / 8 = 8 per row = 56 record-writes per row). */
const CMD_PX = 8;
const IDS_PER_CMD = CMD_PX - 1;
const CMDS_PER_ROW = CMD_W / CMD_PX;
const MAX_CMDS = (CMD_W * CMD_H) / CMD_PX;
/** v3 header operation codes — the operation DEFINES the rest of the command (0x01 = write-data). */
const OP_WRITE_DATA = 0x01;
/** The data texture's 16 u16-addressable SETS (1024×64 each; set = linear >> 16). u5 in the header. */
const SET_SHIFT = 16;
const SET_COUNT = 16;

/** Scatter v3 (work 2026-07-25-primitive-graph): records no longer self-address — the COMMAND carries
 *  the target ids. One command = ${CMD_PX} px: a header px (operation | set | count | 7× u16 ids) then
 *  up to ${IDS_PER_CMD} payload px. The draw issues ${IDS_PER_CMD} points per command; a point past the
 *  header's `count` goes off-clip (never rasterised), so a partial command needs NO sentinel id — which
 *  matters because the tile-keyed sets address by `foldTile`, whose range covers 0..65535 exhaustively
 *  (fold 0 is a real tile, so `id = 0` must stay writable). No per-set count scan any more. */
const SCATTER_VERT = /* glsl */ `#version 300 es
precision highp int;
uniform highp usampler2D uCmd;
uniform int uCmdBase;            // px index of this batch's FIRST command
in uint aIndex;                  // point index within the batch (command * ${IDS_PER_CMD} + slot)
flat out highp int vPayload;     // px index of this point's payload record
uvec4 px(int i) { return texelFetch(uCmd, ivec2(i & 63, i >> 6), 0); }
// header R = u8 operation (24-31) | u5 set (19-23) | u3 count (16-18) | u16 id0 (0-15);
// G/B/A = id1|id2, id3|id4, id5|id6 (high|low). Slot s>0 -> lane (s+1)>>1, odd slot = high half.
uint idOf(uvec4 h, int s) {
  if (s == 0) return h.x & 0xffffu;
  int lane = (s + 1) >> 1;
  uint w = lane == 1 ? h.y : (lane == 2 ? h.z : h.w);
  return (s & 1) == 1 ? (w >> 16) : (w & 0xffffu);
}
void main() {
  int p = int(aIndex);
  int cmd = p / ${IDS_PER_CMD};
  int slot = p - cmd * ${IDS_PER_CMD};
  int base = uCmdBase + cmd * ${CMD_PX};
  uvec4 h = px(base);
  gl_PointSize = 1.0;
  if (slot >= int((h.x >> 16) & 7u)) {            // past this command's live count -> off-clip, no write
    gl_Position = vec4(2.0, 2.0, 0.0, 1.0);
    return;
  }
  vPayload = base + 1 + slot;
  int target = (int((h.x >> 19) & 31u) << 16) + int(idOf(h, slot));   // (set << 16) | in-set id
  float x = (float(target & 1023) + 0.5) / 1024.0 * 2.0 - 1.0;
  float y = (float(target >> 10) + 0.5) / 1024.0 * 2.0 - 1.0;
  gl_Position = vec4(x, y, 0.0, 1.0);
}
`;
const SCATTER_FRAG = /* glsl */ `#version 300 es
precision highp int;
precision highp float;
uniform highp usampler2D uCmd;
flat in highp int vPayload;
out uvec4 fragColor;
void main() { fragColor = texelFetch(uCmd, ivec2(vPayload & 63, vPayload >> 6), 0); }
`;

const clamp = (v: number, hi: number): number => Math.min(Math.max(Math.round(v), 0), hi);
/** Empty slot payload — the eviction clear (length 0 → every slot takes the set sentinel). */
const EMPTY_SLOTS: number[] = [];

/** One light to write into `light_data` (world px + unit-scaled fields; colour 0..1). */
export interface ColdLight {
  x: number;
  y: number;
  z: number; // world px (→ units)
  reach: number; // illumination range, world px (→ units) — how FAR the light throws
  emitterRadius: number; // physical source size, world px (→ units) — penumbra softness (F7/F10)
  color: [number, number, number]; // 0..1
  intensity: number; // 0..1
  castShadows: boolean;
  hot: boolean; // #4: dynamic → the HOT class (baked per frame); static → COLD (baked once)
}

/** World px → `position_anchor_reference` (`region | zone | tile | anchor`, each `u8 = x:4|y:4`); the anchor
 *  is the sub-tile offset in units (`SQUARE/16`). Assumes non-negative world coords. */
export function encodePosition(wx: number, wy: number): number {
  const tileX = Math.floor(wx / SQUARE), tileY = Math.floor(wy / SQUARE);
  const ax = Math.min(15, Math.floor((wx - tileX * SQUARE) / UNIT));
  const ay = Math.min(15, Math.floor((wy - tileY * SQUARE) / UNIT));
  const zoneX = Math.floor(tileX / ZONE_DIM), ltX = tileX % ZONE_DIM;
  const zoneY = Math.floor(tileY / ZONE_DIM), ltY = tileY % ZONE_DIM;
  const regionX = Math.floor(zoneX / REGION_DIM) & 0xf, lzX = zoneX % REGION_DIM;
  const regionY = Math.floor(zoneY / REGION_DIM) & 0xf, lzY = zoneY % REGION_DIM;
  const region = (regionX << 4) | regionY;
  const zone = (lzX << 4) | lzY;
  const tile = (ltX << 4) | ltY;
  const anchor = (ax << 4) | ay;
  return (((region << 24) | (zone << 16) | (tile << 8) | anchor) >>> 0);
}

/** `position_anchor_reference` → world px (inverse of {@link encodePosition}; unit-quantised). Debug/verify. */
export function decodePosition(pos: number): [number, number] {
  const region = (pos >>> 24) & 0xff, zone = (pos >>> 16) & 0xff, tile = (pos >>> 8) & 0xff, anchor = pos & 0xff;
  const wtx = (((region >> 4) * REGION_DIM + (zone >> 4)) * ZONE_DIM + (tile >> 4));
  const wty = (((region & 0xf) * REGION_DIM + (zone & 0xf)) * ZONE_DIM + (tile & 0xf));
  return [wtx * SQUARE + (anchor >> 4) * UNIT, wty * SQUARE + (anchor & 0xf) * UNIT];
}

export class ColdShadowData {
  /** THE unified data texture + its CPU mirror (16 MB — the source of truth for every band). */
  private readonly dataTex: Texture;
  private readonly dataMirror = new Uint32Array(DATA_W * DATA_H * 4);
  /** Changed texels since the last flush (linear indices) — become scatter commands. */
  private readonly dirtySet = new Set<number>();
  /** Command buffer + scatter pass (P2): sequential upload in, random writes out. */
  private readonly cmdTex: Texture;
  private readonly cmdMirror = new Uint32Array(CMD_W * CMD_H * 4);
  private cmdCursorRow = 0; // rotating row cursor — never overwrite just-consumed rows (F3)
  private readonly scatter: Program;
  private readonly scatterGeo: Geometry;
  private readonly dataRT: RenderTarget;
  /** stem+cell+lod → allocated definition_index (defs are IMMUTABLE — one per atlas frame). */
  private readonly defIndex = new Map<string, number>();
  private defNext = 0;
  /** P3 tight bbox per def (WORLD px, relative to the billboard's top-left): `dx,dy` = offset to the
   *  opaque region, `w,h` = its size. Fraction-of-frame × billboard size, so it's LOD-independent. */
  private readonly defTight = new Map<number, { dx: number; dy: number; w: number; h: number }>();
  /** The ONE atlas page the casters' surface frames live on (F2: single page) — bound as `uSurface`
   *  for the P4 silhouette sample. Adopted from the first resolved frame; a frame on any OTHER page
   *  gets no silhouette (solid quad) until multi-page lands (caster-lut C5). */
  private surfacePageTex: Texture | null = null;
  private pageWarned = false;
  private frameSizeWarned = false;

  /** billboard.id → allocated billboard_data_index (≥1; index 0 is the sentinel). */
  private readonly billboardIndex = new Map<number, number>();
  private billboardNext = 1; // 0 reserved as the sentinel
  /** Freed billboard_data indices (P2 free-list) — reused before bumping `billboardNext`, so the id space
   *  survives pan/zone churn (high-water bounded by peak concurrent casters ≪ 65 536). */
  private readonly billboardFreeList: number[] = [];
  /** P3: `prim_data` ids are their OWN space — a prim is what CARRIES a presentation, and both
   *  billboards and lights need one, so they cannot share the leaf counters. */
  private primNext = 1;
  private readonly primFreeList: number[] = [];
  /** billboard.id → the prim carrying it (allocated/freed together with its leaf). */
  private readonly primOfBillboard = new Map<number, number>();
  /** light slot k → the prim carrying it (P3: lights are CARRIED, never placed directly). */
  private readonly primOfLight: number[] = [];
  /** P5 — CONTENT-carried lights: `billboard.id` → its `light_data` id. Their id space starts above
   *  `N_LIGHTS` so it can never collide with the debug array's slots (which `buildLights` zeroes by
   *  index). When `seed()`/`this.lights` finally go, this becomes the only light id space. */
  private readonly lightOfBillboard = new Map<number, number>();
  private carriedLightNext = N_LIGHTS;
  /** What `buildPresence` needs about each carried light: resolved world px + reach. */
  readonly carriedLights = new Map<number, { x: number; y: number; reach: number }>();
  /** Bumped whenever a carried light is added or actually moves — `buildPresence` folds this into its
   *  rebuild signature, so a torch appearing invalidates the per-tile light lists exactly as a debug
   *  light moving does. Without it the gate would hold a stale cull forever. */
  carriedVer = 0;
  private lightCount = 0;

  /** P3: take a `prim_data` id (free-list first, so the space survives churn). */
  private allocPrim(): number {
    return this.primFreeList.pop() ?? this.primNext++;
  }

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.dataTex = new Texture(gl, { width: DATA_W, height: DATA_H, format: "rgba32uint" });
    this.cmdTex = new Texture(gl, { width: CMD_W, height: CMD_H, format: "rgba32uint" });
    this.scatter = new Program(gl, SCATTER_VERT, SCATTER_FRAG, "data-scatter");
    const slots = new Uint32Array(MAX_CMDS * IDS_PER_CMD); // v3: 7 points per command, whole buffer
    for (let i = 0; i < slots.length; i++) slots[i] = i;
    this.scatterGeo = new Geometry(gl, this.scatter, { aIndex: { data: slots, size: 1, integer: true } });
    this.dataRT = new RenderTarget(gl, { width: DATA_W, height: DATA_H, wrap: [this.dataTex] });
  }

  /** THE unified data texture (bound once as `uData` in the shadow shader). */
  get dataTexture(): Texture {
    return this.dataTex;
  }

  /** Record a changed texel (linear index) — becomes one scatter command at flush. */
  private mark(linear: number): void {
    this.dirtySet.add(linear);
  }

  /** P3: the constants row — window mapping + slot/light-count + the WORLD TILT, compare-written so an
   *  unchanged frame costs nothing. Rides the same command path as every other write. */
  private lastConst = [NaN, NaN, NaN, NaN, NaN, NaN, NaN];
  /** `tiltCentiDeg` = the world ground tilt in **centidegrees** (65° → 6500) — lives in the data map's A
   *  reserve so every lighting shader reads the angle without a dedicated uniform (world-space-lighting). */
  setConstants(cols: number, rows: number, winCol: number, winRow: number, slot: number, lights: number, tiltCentiDeg: number): void {
    const c = this.lastConst;
    if (c[0] === cols && c[1] === rows && c[2] === winCol && c[3] === winRow && c[4] === slot && c[5] === lights && c[6] === tiltCentiDeg) return;
    this.lastConst = [cols, rows, winCol, winRow, slot, lights, tiltCentiDeg];
    // v2.1 — ONE self-addressing px: R = u16 id | u16 cols; G = u16 rows | u16 slot;
    // B = i16 winCol | i16 winRow (two's complement halves); A = u16 light_count | u16 tilt_centideg.
    const b0 = CONST_BASE * 4;
    const inSet = CONST_BASE & 0xffff; // its own in-set id (set 15)
    this.dataMirror[b0] = ((inSet << 16) | (cols & 0xffff)) >>> 0;
    this.dataMirror[b0 + 1] = (((rows & 0xffff) << 16) | (slot & 0xffff)) >>> 0;
    this.dataMirror[b0 + 2] = (((winCol & 0xffff) << 16) | (winRow & 0xffff)) >>> 0;
    this.dataMirror[b0 + 3] = (((lights & 0xffff) << 16) | (tiltCentiDeg & 0xffff)) >>> 0;
    this.mark(CONST_BASE);
  }
  /** Write a TILE-keyed set texel (presence / buckets) at the region-torus fold of (wc,wr): the
   *  self-addressing id (the fold) in R's high half, then the 7 slots. Compare-written — an
   *  unchanged tile costs no command. `empty` is the per-set sentinel (0xFFFF presence, 0 buckets).
   *  `slots` holds ≤7 u16 values; missing slots take `empty`. */
  private writeTileSet(base: number, wc: number, wr: number, slots: ArrayLike<number>, empty: number, off = 0): void {
    const id = foldTile(wc, wr);
    const b = (base + id) * 4;
    // v3: 8 slots/px — the u16 self-address is retired (the command carries the id), which is what
    // buys back the 8th slot. R = s0|s1, G = s2|s3, B = s4|s5, A = s6|s7 (high|low).
    const g = (i: number): number => (off + i < slots.length ? slots[off + i] & 0xffff : empty);
    const R = ((g(0) << 16) | g(1)) >>> 0;
    const G = ((g(2) << 16) | g(3)) >>> 0;
    const B = ((g(4) << 16) | g(5)) >>> 0;
    const A = ((g(6) << 16) | g(7)) >>> 0;
    const m = this.dataMirror;
    if (m[b] !== R || m[b + 1] !== G || m[b + 2] !== B || m[b + 3] !== A) {
      m[b] = R; m[b + 1] = G; m[b + 2] = B; m[b + 3] = A;
      this.mark(base + id);
    }
  }
  /** Presence tile: **14** nearest-light u16 indices (0xFFFF empty) across two sets — lo = slots
   *  0–6, hi = slots 7–13. */
  writePresence(wc: number, wr: number, slots: ArrayLike<number>): void {
    this.writeTileSet(PRESENCE_BASE, wc, wr, slots, 0xffff, 0);
    this.writeTileSet(PRESENCE_HI_BASE, wc, wr, slots, 0xffff, 8);
  }
  /** Caster-bucket tile: 7 u16 billboard indices (0 empty — the billboard sentinel). */
  writeBillboardPresence(wc: number, wr: number, slots: ArrayLike<number>): void {
    this.writeTileSet(BILLBOARD_PRESENCE_BASE, wc, wr, slots, 0x0000);
  }
  /** Clear a tile's presence (both sets, all-empty) — eviction. */
  clearPresence(wc: number, wr: number): void {
    this.writeTileSet(PRESENCE_BASE, wc, wr, EMPTY_SLOTS, 0xffff);
    this.writeTileSet(PRESENCE_HI_BASE, wc, wr, EMPTY_SLOTS, 0xffff);
  }
  /** Clear a tile's buckets (all-empty) — eviction. */
  clearCasters(wc: number, wr: number): void {
    this.writeTileSet(BILLBOARD_PRESENCE_BASE, wc, wr, EMPTY_SLOTS, 0x0000);
  }

  /** The shared surface atlas page (or null before any sprite resolved). */
  get surfacePage(): Texture | null {
    return this.surfacePageTex;
  }


  /** The `definition_index` for a caster's sprite at its CURRENTLY-RESOLVED lod. Defs are
   *  **immutable — one def per atlas frame** (keyed `stem|cell|lod`): a new lod landing mints a NEW
   *  def instead of rewriting the old one, and `billboard_data` keeps pointing at whatever def it holds
   *  until {@link billboardDataFor} swaps it — which reports `changed` so the caller runs the billboard dirty
   *  cascade (billboard tiles → reaching lights → their cast regions). The lod-0 def is the LOOSE
   *  fallback (full-footprint box, no frame → solid quad) used until the surface resolves.
   *
   *  The shadow quad = the sprite's minimum bbox (EVEN units) at an unsigned frame-relative offset;
   *  nudges align the sampled window; anchors place the bbox on the billboard's position. */
  definitionFor(billboard: Primitive, resolver: TextureResolver | null): number {
    if (!billboard.textureName) return -1;

    const bbox = resolver ? resolver.opaqueBBox(billboard.textureName) : null;
    const surf = resolver && bbox ? resolver.resolve(billboard.textureName, "surface", billboard.cell).frame : null;

    // Frame world span in TILES (pow2, ≤ ZONE_DIM — u4 stores tiles−1, width = log2(ZONE_DIM)).
    // Footprints are square (aspect plumbing removed); a non-pow2/oversized span is a content bug.
    const stRaw = Math.max(1, Math.round(Math.max(billboard.width, billboard.height) / SQUARE));
    let st = 1;
    while (st < stRaw) st <<= 1;
    if ((st !== stRaw || st > ZONE_DIM) && !this.frameSizeWarned) {
      this.frameSizeWarned = true;
      console.warn(`[cold-shadow] ${billboard.textureName}: footprint ${stRaw} tiles is not pow2 ≤ ${ZONE_DIM} — span rounded`);
    }
    st = Math.min(st, ZONE_DIM);
    const spanU = st * 16; // frame world span in units

    // Determine the resolved lod (0 = loose: no frame / off-page / below the 1 px/unit floor).
    // ppu = 2^lod / spanU is a whole pow2 ≥ 1 by construction when a lod resolves.
    let lod = 0, ppu = 1;
    if (surf) {
      if (!this.surfacePageTex) this.surfacePageTex = surf.source;
      const p = surf.w / spanU;
      const lodF = Math.log2(surf.w);
      if (surf.source !== this.surfacePageTex) {
        if (!this.pageWarned) {
          this.pageWarned = true;
          console.warn("[cold-shadow] surface frame off the shared page — silhouette skipped (solid quad, C5)");
        }
      } else if (Number.isInteger(p) && p >= 1 && Number.isInteger(lodF)) {
        lod = lodF;
        ppu = p;
      }
    }

    // IMMUTABLE defs — ONE def per atlas frame, keyed (stem, cell, lod). An existing def returns
    // as-is; a new lod mints a NEW index. billboard_data keeps whatever def it holds until
    // {@link billboardDataFor} swaps it — which reports `changed`, and the caller runs the billboard dirty
    // cascade. Nothing ever rewrites a def, so def changes ride the billboard-update path for free.
    const key = `${billboard.textureName}|${billboard.cell ?? 0}|${lod}`;
    const hit = this.defIndex.get(key);
    if (hit !== undefined) return hit;
    const idx = this.defNext++;
    this.defIndex.set(key, idx);

    // Whole-px-per-unit fields: minimum bbox in EVEN units at an unsigned frame-relative offset;
    // px nudges align the sampled window (x centered, y bottom — shadows anchor at the base).
    let wu = spanU, hu = spanU, ux0 = 0, uy0 = 0; // LOOSE (lod 0): full-footprint bbox, solid quad
    let fx16 = 0, fy16 = 0, nx = 0, ny = 0;
    if (lod >= 4 && surf) {
      fx16 = Math.round(surf.x / 16);
      fy16 = Math.round(surf.y / 16);
      // COHERENCE (2026-07-24): sample the WHOLE square frame as received — the SAME coordinate system the
      // composite draws EVERY map in (unit quad → the billboard's world rect, full-frame UV). We do NOT re-derive a
      // per-map opaque MINIMUM bbox here anymore: that gave the shadow + normal a DIFFERENT rectangle (even-unit
      // rounded + nudged) than the albedo, which is the whole misalignment (and what RECV_ALIGN/uLightAlign were
      // band-aiding). Transparent area casts no shadow, so the silhouette is identical — only the sampling
      // WINDOW is now the full frame. wu/hu = spanU, ux0/uy0/nx/ny = 0 (the loose defaults, kept).
    }
    // Bucketing box (world px, rel. billboard top-left) — 1 frame unit ≡ 1 world unit by the span model.
    this.defTight.set(idx, { dx: ux0 * UNIT, dy: uy0 * UNIT, w: wu * UNIT, h: hu * UNIT });

    const base = (DEF_BASE + idx) * 4;
    const page = 0;        // ONE bound surface page today — C5 (texture-array pages) assigns real indices
    const ax = 1, ay = 2;  // shadow casters hang the bbox at the billboard's BOTTOM-CENTER anchor
    const nax = 1, nay = 2; // nudge alignment: x centered, y bottom — the default nudging operation
    // v3 (primitive-graph): NO self-address — the command carries the id. Both offsets move into R.
    // R = u4 layer (28-31) | u2 rotation (26-27) | u1 inherit_rotation (25) | u1 reserved (24)
    //   | u10 offset_x (14-23) | u10 offset_y (4-13) | u4 type (0-3);  G = u9 W | u9 H | u4 span | u10 reserved.
    // layer/rotation/inherit_rotation/type are AUTHORED fields the resolver has no source for yet — 0
    // until the DSL supplies them (P5); the geometry fields below are what the gather reads today.
    this.dataMirror[base] = ((((ux0 & 0x3ff) << 14) | ((uy0 & 0x3ff) << 4)) >>> 0);
    this.dataMirror[base + 1] = ((((wu >> 1) & 0x1ff) << 23) | (((hu >> 1) & 0x1ff) << 14) | (((st - 1) & 0xf) << 10)) >>> 0;
    this.dataMirror[base + 2] = (((fx16 & 0x3ff) << 22) | ((fy16 & 0x3ff) << 12) | ((page & 0xf) << 8) | ((lod & 0xf) << 4) | ((ax & 3) << 2) | (ay & 3)) >>> 0;
    this.dataMirror[base + 3] = ((((nx + 2048) & 0xfff) << 20) | (((ny + 2048) & 0xfff) << 8) | ((nax & 3) << 6) | ((nay & 3) << 4)) >>> 0;
    this.mark(DEF_BASE + idx);
    return idx;
  }

  /** The tight bbox (WORLD px, relative to the billboard's top-left) for a placed caster's def, or null. */
  tightBoxOf(defIndex: number): { dx: number; dy: number; w: number; h: number } | null {
    return this.defTight.get(defIndex) ?? null;
  }


  /** The `billboard_data_index` (≥1) for a PLACED caster (position + orientation + its `def_index`),
   *  allocated on first sight + cached by `billboard.id`. Defs are immutable, so a texture/lod change
   *  arrives HERE as a `def_index` swap: the orient word rewrites in place and `changed` reports it —
   *  the caller runs the billboard dirty cascade (billboard tiles → reaching lights → their cast regions).
   *  First sight is `changed` too (the same cascade seeds the new caster's region — no force-all). */
  /** Allocate/patch the pair of records a placed billboard needs under the primitive graph: a ROOT
   *  `prim_data` node (set 1) carrying ONE billboard, and the `billboard_data` leaf (set 6) it points
   *  at. The two are 1:1 in this degenerate form, so ONE index serves as both ids — P3 breaks that
   *  apart when a prim can carry several pieces / nest. Both records compare-write, so an unchanged
   *  billboard still emits no command. */
  billboardDataFor(billboard: Primitive, defIndex: number): { idx: number; changed: boolean } {
    const rotation = billboard.flipX ? 3 : 1; // W : E (both E/W regime for now)
    const z = 0;
    const ax = billboard.x + billboard.width * 0.5; // the TRUE game anchor (full-box base-centre)
    const ay = billboard.y + billboard.height;
    const pos = encodePosition(ax, ay);             // region|zone|tile|unit
    const tile = (pos >>> 8) & 0xff, unit = pos & 0xff;

    const hit = this.billboardIndex.get(billboard.id);
    let idx: number, prim: number;
    if (hit !== undefined) {
      idx = hit;
      prim = this.primOfBillboard.get(billboard.id) ?? this.allocPrim();
      // RETENTION: never swap a standing billboard DOWN to the loose (lod-0) def — if the freshly
      // resolved frame is unusable (off-page) it keeps casting its current silhouette.
      const curDef = (this.dataMirror[(BILLBOARD_DATA_BASE + idx) * 4 + 2] >>> 16) & 0xffff;
      const newLod = (this.dataMirror[(DEF_BASE + defIndex) * 4 + 2] >>> 4) & 0xf;
      const curLod = (this.dataMirror[(DEF_BASE + curDef) * 4 + 2] >>> 4) & 0xf;
      if (newLod < 4 && curLod >= 4) defIndex = curDef;
    } else {
      idx = this.billboardFreeList.pop() ?? this.billboardNext++; // reuse a freed slot first (P2)
      prim = this.allocPrim();
      this.billboardIndex.set(billboard.id, idx);
    }
    this.primOfBillboard.set(billboard.id, prim);
    // FAST PATH: `buildCasters` calls this for EVERY standing prim EVERY frame, and almost none of them
    // change. Three mirror reads decide it — position, rotation, definition — and an unchanged prim skips
    // the resolve walk and both compare-writes. Without this the graph work runs ~1000×/frame to
    // conclude nothing happened (measured ~0.9 ms/frame of pure waste at 1709 prims).
    const pb = (PRIM_BASE + prim) * 4, lb = (BILLBOARD_DATA_BASE + idx) * 4;
    if (hit !== undefined && this.dataMirror[pb] === pos
        && ((this.dataMirror[pb + 1] >>> 26) & 3) === rotation
        && ((this.dataMirror[lb + 2] >>> 16) & 0xffff) === defIndex) {
      return { idx, changed: false };
    }
    // prim_data (set 1): a ROOT (child = 0) at the absolute position, carrying the billboard in slot a.
    // `cast_shadows` MUST be set here: it inherits by AND down the chain, so a carrier that leaves it
    // clear silences everything it carries (caught by mirror readback — the leaf came back cast = 0).
    const primChanged = this.writeRecord(PRIM_BASE + prim, pos,
      ((((rotation & 3) << 26) | (1 << 24) | ((z & 0xff) << 16)
        | ((SET_BILLBOARD_DATA & 0xf) << 12)) >>> 0),
      (((idx & 0xffff) << 16) >>> 0), 0);
    // billboard_data (set 6): parent + the RESOLVED position + definition. The resolve walks this
    // billboard's authored offsets up its carrier chain — a no-op while carriers are roots and the
    // offsets are the bias-8 zero, and correct the moment either stops being true.
    const r = this.resolveCarried(prim, OFFSET_ZERO, OFFSET_ZERO, false, true, 0);
    const leafChanged = this.writeRecord(BILLBOARD_DATA_BASE + idx,
      ((((prim & 0xffff) << 16) | (((r.pos >>> 8) & 0xff) << 8) | (r.pos & 0xff)) >>> 0),
      ((((rotation & 3) << 26) | ((r.hot ? 1 : 0) << 25) | ((r.cast ? 1 : 0) << 24)
        | ((r.z & 0xff) << 16) | (OFFSET_ZERO << 8) | OFFSET_ZERO) >>> 0),
      (((defIndex & 0xffff) << 16) >>> 0), 0);
    return { idx, changed: primChanged || leafChanged };
  }

  /** P3: author a CHILD prim under `parent` — placed by **bias-8 signed** offsets off its carrier
   *  instead of an absolute address (`child = 1` makes RED's top half the `parent_id`). Offsets are
   *  in tiles + units and may be negative, so a piece can sit in any direction from its carrier.
   *  Returns the new prim id. This is what authoring (P5) calls to hang a hand off a pawn or a torch
   *  off a hand; until then it is exercised by {@link debugResolveChain}. */
  childPrimUnder(parent: number, dTileX: number, dTileY: number, dUnitX: number, dUnitY: number,
                 setNib: number, carriedId: number, opts: { hot?: boolean; cast?: boolean; z?: number } = {}): number {
    const prim = this.allocPrim();
    const b8 = (v: number): number => (Math.max(-8, Math.min(7, v)) + 8) & 0xf;
    const R = (((parent & 0xffff) << 16) | (b8(dTileX) << 12) | (b8(dTileY) << 8)
      | (b8(dUnitX) << 4) | b8(dUnitY)) >>> 0;
    const G = (((1 << 28) | ((opts.hot ? 1 : 0) << 25) | ((opts.cast === false ? 0 : 1) << 24)
      | (((opts.z ?? 0) & 0xff) << 16) | ((setNib & 0xf) << 12)) >>> 0);
    this.writeRecord(PRIM_BASE + prim, R, G, (((carriedId & 0xffff) << 16) >>> 0), 0);
    return prim;
  }

  /** DEBUG/self-test for the resolve walk: build `root → child → leaf` with known offsets, resolve,
   *  and compare against hand-computed world arithmetic. Frees its scratch prims, so it can be run
   *  against the live scene without disturbing it. Proves the chain sums offsets AND folds inheritance
   *  (the root is made hot + non-casting, which must force both onto the leaf). */
  debugResolveChain(): Record<string, unknown> {
    const rootX = 40 * SQUARE + 3 * UNIT, rootY = 30 * SQUARE + 5 * UNIT;
    const root = this.allocPrim();
    // root: hot + NON-casting, so inheritance has something to force down the chain.
    this.writeRecord(PRIM_BASE + root, encodePosition(rootX, rootY),
      (((1 << 25) | (0 << 24)) >>> 0), 0, 0);
    const child = this.childPrimUnder(root, -2, 3, 5, -4, SET_BILLBOARD_DATA, 0, { cast: true });
    const leafTileOff = ((8 + 1) << 4) | (8 - 3);   // +1 tile x, −3 tiles y  (bias-8)
    const leafUnitOff = ((8 + 6) << 4) | (8 + 2);   // +6 units x, +2 units y
    const got = this.resolveCarried(child, leafTileOff, leafUnitOff, false, true);
    // Hand-computed: root + child offsets + leaf offsets.
    const wx = rootX + (-2 + 1) * SQUARE + (5 + 6) * UNIT;
    const wy = rootY + (3 - 3) * SQUARE + (-4 + 2) * UNIT;
    const want = encodePosition(wx, wy);
    const [gx, gy] = decodePosition(got.pos);
    this.writeRecord(PRIM_BASE + child, 0, 0, 0, 0);
    this.writeRecord(PRIM_BASE + root, 0, 0, 0, 0);
    this.primFreeList.push(child, root);
    return {
      resolved: { x: gx, y: gy }, expected: { x: wx, y: wy },
      posMatches: got.pos === want,
      hot: got.hot, hotInherited: got.hot === true,      // root hot → leaf hot (OR)
      cast: got.cast, castSilenced: got.cast === false,  // root !cast → leaf !cast (AND)
    };
  }

  /** DEBUG/self-test for the reachability free: build `root{ child{ billboard }, light }`, release the
   *  root, and confirm every record in the subtree is zeroed and every id reclaimed. The nested
   *  billboard is the case the old flat sweep could not reach. */
  debugFreeSubtree(): Record<string, unknown> {
    const primsBefore = this.primFreeList.length, bbBefore = this.billboardFreeList.length;
    const root = this.allocPrim();
    const bb = this.billboardFreeList.pop() ?? this.billboardNext++;
    const light = N_LIGHTS - 1;                       // scratch slot, well past the live lights
    const child = this.childPrimUnder(root, 1, 1, 0, 0, SET_BILLBOARD_DATA, bb);
    // root carries the CHILD PRIM in slot a and a LIGHT in slot b.
    this.writeRecord(PRIM_BASE + root, encodePosition(0, 0),
      ((((SET_PRIM_DATA & 0xf) << 12) | ((SET_LIGHT_DATA & 0xf) << 8)) >>> 0),
      ((((child & 0xffff) << 16) | (light & 0xffff)) >>> 0), 0);
    this.writeRecord(BILLBOARD_DATA_BASE + bb, 0xdead0000, 0, 0, 0);
    this.writeRecord(LIGHT_BASE + light, 0xbeef0000, 0, 0, 0);
    this.freeSubtree(root);
    const zero = (base: number, id: number): boolean => this.dataMirror[(base + id) * 4] === 0;
    return {
      nestedBillboardZeroed: zero(BILLBOARD_DATA_BASE, bb),   // reached through the child prim
      lightZeroed: zero(LIGHT_BASE, light),
      childPrimZeroed: zero(PRIM_BASE, child),
      rootPrimZeroed: zero(PRIM_BASE, root),
      primsReclaimed: this.primFreeList.length - primsBefore,       // expect 2 (root + child)
      billboardsReclaimed: this.billboardFreeList.length - bbBefore, // expect 1
    };
  }

  /** P3 THE RESOLVE WALK — turn a carried piece's *relative* placement into an absolute one by
   *  climbing `carrier → … → root`, summing offsets and folding the inherited flags:
   *    · `hot_cold`     — **topmost hot wins**: any hot ancestor forces the whole subtree hot (OR).
   *    · `cast_shadows` — **topmost !cast wins**: any non-casting ancestor silences the subtree (AND).
   *  Offsets are BIAS-8 signed nibbles (stored − 8 ⇒ −8..+7) so a child can sit in any direction. The
   *  sum is applied in world px and re-encoded, which gets unit→tile→zone carries for free.
   *  Bounded by {@link MAX_PRIM_DEPTH} — a malformed cycle costs a fixed walk, never a hang.
   *  THIS is the single resolve authority ([I12]): the CPU stamps what the GPU reads, and the GPU
   *  never walks the graph in its hot loop. */
  private resolveCarried(prim: number, tileOff: number, unitOff: number, hot: boolean, cast: boolean,
                         zOff = 0): { pos: number; hot: boolean; cast: boolean; z: number } {
    let dtx = ((tileOff >>> 4) & 0xf) - 8, dty = (tileOff & 0xf) - 8;
    let dux = ((unitOff >>> 4) & 0xf) - 8, duy = (unitOff & 0xf) - 8;
    let p = prim, outHot = hot, outCast = cast, rootPos = 0, z = zOff;
    for (let depth = 0; depth < MAX_PRIM_DEPTH; depth++) {
      const b = (PRIM_BASE + p) * 4;
      const R = this.dataMirror[b], G = this.dataMirror[b + 1];
      outHot = outHot || ((G >>> 25) & 1) === 1;
      outCast = outCast && ((G >>> 24) & 1) === 1;
      z += (G >>> 16) & 0xff;                                // heights ADD down the chain
      if (((G >>> 28) & 1) === 0) { rootPos = R; break; }   // ROOT: R is the absolute address
      dtx += ((R >>> 12) & 0xf) - 8; dty += ((R >>> 8) & 0xf) - 8;  // CHILD: accumulate + climb
      dux += ((R >>> 4) & 0xf) - 8;  duy += (R & 0xf) - 8;
      p = (R >>> 16) & 0xffff;
    }
    const [rx, ry] = decodePosition(rootPos);
    return {
      pos: encodePosition(rx + dtx * SQUARE + dux * UNIT, ry + dty * SQUARE + duy * UNIT),
      hot: outHot, cast: outCast,
      // The summed height is a u8 field, so a deep chain of tall carriers SATURATES rather than
      // wrapping — a clamped light sits too low, a wrapped one teleports to the ground (I9).
      z: Math.min(255, z),
    };
  }

  /** The carrier prim allocated for a placed billboard (P5 hangs its light off the same one). */
  primOf(billboardId: number): number {
    return this.primOfBillboard.get(billboardId) ?? 0;
  }

  /** P5 — write the LIGHT leaf a placed primitive carries, under the **same carrier prim** as its
   *  billboard ([F9]). One placed object, two presentations: `set_a` already names the billboard, so
   *  the light takes `set_b`. Returns the light's in-set id. The carrier is written by
   *  {@link billboardDataFor} first, so the resolve walk below reads a populated node. */
  carriedLightFor(billboard: Primitive, L: PrimitiveLight): number {
    const billboardId = billboard.id;
    // ENSURE a carrier. `billboardDataFor` allocates one for a *caster*, but a primitive can carry a
    // light without being one (a bare light source, or a sprite with no resolved silhouette — those
    // `continue` out of the caster loop before ever getting a carrier). So allocate + place one here
    // when it is missing, rather than assuming the billboard path ran.
    let prim = this.primOfBillboard.get(billboardId);
    if (prim === undefined) {
      prim = this.allocPrim();
      this.primOfBillboard.set(billboardId, prim);
      this.writeRecord(PRIM_BASE + prim,
        encodePosition(billboard.x + billboard.width * 0.5, billboard.y + billboard.height),
        (((1 << 24) | ((SET_LIGHT_DATA & 0xf) << 8)) >>> 0), 0, 0);
    }
    return this.writeCarriedLight(billboardId, prim, L);
  }

  private writeCarriedLight(billboardId: number, prim: number, L: PrimitiveLight): number {
    const idx = this.lightOfBillboard.get(billboardId)
      ?? (this.lightOfBillboard.set(billboardId, this.carriedLightNext++), this.carriedLightNext - 1);
    // Point the carrier's slot b at this light (slot a is the billboard, written by billboardDataFor).
    const pb = (PRIM_BASE + prim) * 4, m = this.dataMirror;
    const G = ((m[pb + 1] & ~(0xf << 8)) | ((SET_LIGHT_DATA & 0xf) << 8)) >>> 0;
    const B = ((m[pb + 2] & 0xffff0000) | (idx & 0xffff)) >>> 0;   // id_b, keeping id_a
    this.writeRecord(PRIM_BASE + prim, m[pb], G, B, m[pb + 3]);
    const r = this.resolveCarried(prim, OFFSET_ZERO, OFFSET_ZERO, L.hot, L.castShadows,
                                  clamp(L.height / UNIT, 255));
    const c = (v: number): number => clamp(v * 255, 255);
    this.writeRecord(LIGHT_BASE + idx,
      ((((prim & 0xffff) << 16) | (((r.pos >>> 8) & 0xff) << 8) | (r.pos & 0xff)) >>> 0),
      ((((r.hot ? 1 : 0) << 25) | ((r.cast ? 1 : 0) << 24) | ((r.z & 0xff) << 16)
        | (OFFSET_ZERO << 8) | OFFSET_ZERO) >>> 0),
      (((c(L.color[0]) << 24) | (c(L.color[1]) << 16) | (c(L.color[2]) << 8) | c(L.intensity)) >>> 0),
      (((clamp(L.reach / UNIT, 0xfff) << 20) | (clamp(L.emitterRadius / UNIT, 255) << 12)
        | (((r.pos >>> 16) & 0xff) << 4)) >>> 0));
    const [wx, wy] = decodePosition(r.pos);
    const prev = this.carriedLights.get(idx);
    if (!prev || prev.x !== wx || prev.y !== wy || prev.reach !== L.reach) {
      this.carriedLights.set(idx, { x: wx, y: wy, reach: L.reach });
      this.carriedVer++;                       // the per-tile cull is now stale
    }
    return idx;
  }

  /** Compare-write one whole record; marks it dirty (→ one scatter command) only if it changed. */
  private writeRecord(linear: number, R: number, G: number, B: number, A: number): boolean {
    const b = linear * 4, m = this.dataMirror;
    if (m[b] === R && m[b + 1] === G && m[b + 2] === B && m[b + 3] === A) return false;
    m[b] = R; m[b + 1] = G; m[b + 2] = B; m[b + 3] = A;
    this.mark(linear);
    return true;
  }

  /** Free every allocated billboard whose `billboard.id` is NOT in `seen` (it left the resident/standing set —
   *  zone evicted or destroyed). Returns its slot to the free-list. No slot clear + no bucket cascade
   *  needed: `buildCasters` rebuilds every in-window bucket from the current `standing` (so a freed id
   *  can't be referenced), and caster REMOVAL force-alls the shadow recompute. Reach gap makes it
   *  safe: `standing` bounds who can cast, so a freed billboard is beyond reach of every in-window tile.
   *  Returns the number freed. */
  freeBillboardsExcept(seen: Set<number>): number {
    let dead: number[] | null = null;
    for (const pid of this.billboardIndex.keys()) if (!seen.has(pid)) (dead ??= []).push(pid);
    if (!dead) return 0;
    for (const pid of dead) {
      this.billboardFreeList.push(this.billboardIndex.get(pid)!);
      this.billboardIndex.delete(pid);
      // P3 subtree lifetime: a carried leaf never outlives its carrier — releasing the billboard
      // releases the prim holding it AND everything that prim carries, recursively.
      const prim = this.primOfBillboard.get(pid);
      if (prim !== undefined) {
        this.freeSubtree(prim);
        this.primOfBillboard.delete(pid);
      }
    }
    return dead.length;
  }

  /** P3 REACHABILITY FREE ([I9]) — release `prim` and everything it carries, depth-first. Replaces the
   *  flat "was it seen this frame" sweep, which only knew about top-level billboards: once a prim can
   *  carry other prims, dropping the root alone strands its whole subtree in the id space (records the
   *  buckets no longer reference, ids the free-list never reclaims). Walks `set_a..d`, recurses into
   *  carried PRIMS, and returns freed leaves to their own lists. `MAX_PRIM_DEPTH`-bounded and
   *  cycle-safe via `seen`, so malformed data costs a bounded walk instead of a hang. */
  private freeSubtree(prim: number, depth = 0, seen = new Set<number>()): void {
    if (depth >= MAX_PRIM_DEPTH || prim === 0 || seen.has(prim)) return;
    seen.add(prim);
    const b = (PRIM_BASE + prim) * 4;
    const G = this.dataMirror[b + 1], B = this.dataMirror[b + 2], A = this.dataMirror[b + 3];
    const ids = [(B >>> 16) & 0xffff, B & 0xffff, (A >>> 16) & 0xffff, A & 0xffff];
    for (let slot = 0; slot < 4; slot++) {
      const set = (G >>> (12 - slot * 4)) & 0xf;              // set_a..d, high nibble first
      const id = ids[slot];
      if (set === 0) continue;                                 // set 0 = "no data carried"
      if (set === SET_PRIM_DATA) { this.freeSubtree(id, depth + 1, seen); continue; }
      if (set === SET_BILLBOARD_DATA) {
        this.writeRecord(BILLBOARD_DATA_BASE + id, 0, 0, 0, 0);
        this.billboardFreeList.push(id);
      } else if (set === SET_LIGHT_DATA) {
        this.writeRecord(LIGHT_BASE + id, 0, 0, 0, 0);
      }
    }
    this.writeRecord(PRIM_BASE + prim, 0, 0, 0, 0);
    this.primFreeList.push(prim);
  }

  get lights(): number {
    return this.lightCount;
  }

  /** Build `light_data` (the record row) from the lights, and upload it. **Records only** — casters
   *  are found by the per-tile buckets + corridor sweep now, so there's no LUT to build here (`standing`
   *  is bucketed by `ShadowGather.buildCasters`, which also allocates defs/billboards). Call on change. */
  buildLights(lights: ColdLight[]): void {
    const n = Math.min(lights.length, N_LIGHTS);
    const m = this.dataMirror;
    for (let k = 0; k < Math.max(n, this.lightCount); k++) {
      const base = (LIGHT_BASE + k) * 4;
      let R = 0, G = 0, B = 0, A = 0; // k >= n → a removed light zeroes its record
      if (k < n) {
        const L = lights[k];
        // v3 light_data (primitive-graph):
        //   R = u16 parent_id (16–31) | u8 resolved_tile (8–15) | u8 resolved_unit (0–7)
        //   G = u4 layer (28–31) | u2 rotation (26–27) | u1 hot_cold (25) | u1 cast_shadows (24)
        //     | u8 z_offset (16–23) | u8 tile_offset (8–15) | u8 unit_offset (0–7)   (offsets BIAS-8)
        //   B = u8 r | u8 g | u8 b | u8 intensity
        //   A = u12 reach (20–31) | u8 emitter_radius (12–19) | u8 resolved_zone (4–11) | u4 reserved
        // P3: a light is CARRIED, never placed — it gets a ROOT prim holding the absolute position,
        // and the leaf points back at it. The carrier is written FIRST (below is too late: the resolve
        // walk reads it), then the leaf's position comes from walking that chain.
        const z = clamp(L.z / UNIT, 255), reach = clamp(L.reach / UNIT, 0xfff), em = clamp(L.emitterRadius / UNIT, 255);
        const prim = this.primOfLight[k] ?? (this.primOfLight[k] = this.allocPrim());
        this.writeRecord(PRIM_BASE + prim, encodePosition(L.x, L.y),
          ((((L.hot ? 1 : 0) << 25) | ((L.castShadows ? 1 : 0) << 24) | ((z & 0xff) << 16)
            | ((SET_LIGHT_DATA & 0xf) << 12)) >>> 0),
          (((k & 0xffff) << 16) >>> 0), 0);
        // The carrier holds the AUTHORED height; the leaf's z field is what the GPU reads, so it takes
        // the RESOLVED sum (carrier chain + own offset) — same split as position. Passing 0 as the
        // leaf's own offset keeps today's single-level case at exactly the light's height.
        const res = this.resolveCarried(prim, OFFSET_ZERO, OFFSET_ZERO, false, true, 0);
        const zone = (res.pos >>> 16) & 0xff, tile = (res.pos >>> 8) & 0xff, unit = res.pos & 0xff;
        R = (((prim & 0xffff) << 16) | (tile << 8) | unit) >>> 0;
        G = ((((res.hot ? 1 : 0) << 25) | ((res.cast ? 1 : 0) << 24) | ((res.z & 0xff) << 16)
          | (OFFSET_ZERO << 8) | OFFSET_ZERO) >>> 0);
        const r = clamp(L.color[0] * 255, 255), g = clamp(L.color[1] * 255, 255), b = clamp(L.color[2] * 255, 255);
        B = (((r << 24) | (g << 16) | (b << 8) | clamp(L.intensity * 255, 255)) >>> 0);
        A = (((reach << 20) | (em << 12) | (zone << 4)) >>> 0);
      }
      // Compare-write: a STATIC light's record is unchanged → no command (only movers re-scatter).
      if (m[base] !== R || m[base + 1] !== G || m[base + 2] !== B || m[base + 3] !== A) {
        m[base] = R; m[base + 1] = G; m[base + 2] = B; m[base + 3] = A;
        this.mark(LIGHT_BASE + k);
      }
      // A REMOVED light (k >= n) releases its carrier to the free-list and zeroes the node, so no
      // stale carrier stays reachable. (The live carrier is written above, before the resolve.)
      const prim = this.primOfLight[k];
      if (k >= n && prim !== undefined) {
        this.writeRecord(PRIM_BASE + prim, 0, 0, 0, 0);
        this.primFreeList.push(prim);
        delete this.primOfLight[k];
      }
    }
    this.lightCount = n;
  }

  /** Upload any changed textures (call once per frame after populating). Cheap — no-op unless a slot landed. */
  /** Returns true if anything was uploaded (def/billboard changed) — the caller re-dirties the shadow. */
  /** DEBUG: commands scattered by the last flush (per-set breakdown). */
  debugLastFlush: Record<number, number> = {};
  flush(): boolean {
    // DEBUG per-set command tally (which sets got writes this flush).
    this.debugLastFlush = {};
    for (const t of this.dirtySet) { const s = t >> SET_SHIFT; this.debugLastFlush[s] = (this.debugLastFlush[s] ?? 0) + 1; }
    if (this.dirtySet.size === 0) return false;
    // v3 commands: bucket the changed texels by SET (linear >> 16), then cut each set's ids into
    // chunks of <= IDS_PER_CMD. One command = header px (operation | set | count | 7× u16 ids) +
    // IDS_PER_CMD payload px, fixed stride. The header's `count` marks the live ids, so a partial
    // command needs no sentinel — `id = 0` stays writable (tile-keyed sets address by foldTile,
    // whose range covers 0..65535 exhaustively, so fold 0 is a real tile).
    const buckets: number[][] = Array.from({ length: SET_COUNT }, () => []);
    for (const t of this.dirtySet) buckets[t >> SET_SHIFT].push(t & 0xffff);
    this.dirtySet.clear();
    const cmds: { set: number; ids: number[] }[] = [];
    for (let b = 0; b < SET_COUNT; b++) {
      const ids = buckets[b];
      for (let i = 0; i < ids.length; i += IDS_PER_CMD) cmds.push({ set: b, ids: ids.slice(i, i + IDS_PER_CMD) });
    }
    const m = this.cmdMirror;
    for (let c0 = 0; c0 < cmds.length; ) {
      if (this.cmdCursorRow >= CMD_H) this.cmdCursorRow = 0;  // rotate (F3) — never look back
      const batch = Math.min(cmds.length - c0, (CMD_H - this.cmdCursorRow) * CMDS_PER_ROW);
      const rowsNeeded = Math.ceil(batch / CMDS_PER_ROW);
      const basePx = this.cmdCursorRow * CMD_W;
      for (let k = 0; k < batch; k++) {
        const { set, ids } = cmds[c0 + k];
        const cmdPx = basePx + k * CMD_PX;
        const h = cmdPx * 4;
        // R = u8 operation (24–31) | u5 set (19–23) | u3 count (16–18) | u16 id0; G/B/A = id1..id6.
        m[h] = (((OP_WRITE_DATA << 24) | (set << 19) | (ids.length << 16) | (ids[0] ?? 0)) >>> 0);
        m[h + 1] = ((((ids[1] ?? 0) << 16) | (ids[2] ?? 0)) >>> 0);
        m[h + 2] = ((((ids[3] ?? 0) << 16) | (ids[4] ?? 0)) >>> 0);
        m[h + 3] = ((((ids[5] ?? 0) << 16) | (ids[6] ?? 0)) >>> 0);
        for (let i = 0; i < ids.length; i++) {
          const src = ((set << SET_SHIFT) + ids[i]) * 4;
          const dst = (cmdPx + 1 + i) * 4;
          m[dst] = this.dataMirror[src];
          m[dst + 1] = this.dataMirror[src + 1];
          m[dst + 2] = this.dataMirror[src + 2];
          m[dst + 3] = this.dataMirror[src + 3];
        }
      }
      this.cmdTex.uploadRows(this.cmdCursorRow, rowsNeeded, this.cmdMirror, 4);
      this.renderer.draw({
        program: this.scatter,
        geometry: this.scatterGeo,
        target: this.dataRT,
        blend: "none",
        mode: this.renderer.gl.POINTS,
        count: batch * IDS_PER_CMD,   // 7 points/command; those past `count` go off-clip
        textures: { uCmd: this.cmdTex },
        uniforms: (prog) => prog.uInt("uCmdBase", basePx),
      });
      this.cmdCursorRow += rowsNeeded;
      c0 += batch;
    }
    return true;
  }


  // ── DEBUG decoders (verify against the CPU mirrors) ─────────────────────────────────
  debugDef(index: number): {
    billboard_width: number; billboard_height: number; frame_span: number; offset: [number, number];
    frame_xy: [number, number]; frame_page: number; frame_lod: number; anchor: [number, number];
    nudge: [number, number]; nudge_anchor: [number, number];
  } {
    const b = (DEF_BASE + index) * 4;
    const R = this.dataMirror[b], G = this.dataMirror[b + 1], B = this.dataMirror[b + 2], A = this.dataMirror[b + 3];
    return {
      billboard_width: ((G >>> 23) & 0x1ff) * 2,  // units (stored /2 — even bbox)
      billboard_height: ((G >>> 14) & 0x1ff) * 2,
      frame_span: ((G >>> 10) & 0xf) + 1,    // tiles
      offset: [(R >>> 6) & 0x3ff, G & 0x3ff], // units, frame-relative (unsigned)
      frame_xy: [((B >>> 22) & 0x3ff) * 16, ((B >>> 12) & 0x3ff) * 16], // frame origin (px)
      frame_page: (B >>> 8) & 0xf,
      frame_lod: (B >>> 4) & 0xf,            // side = 2^lod; < 4 = no silhouette (solid quad)
      anchor: [(B >>> 2) & 0x3, B & 0x3],
      nudge: [((A >>> 20) & 0xfff) - 2048, ((A >>> 8) & 0xfff) - 2048], // px, signed (+2048 bias)
      nudge_anchor: [(A >>> 6) & 0x3, (A >>> 4) & 0x3], // the alignment the nudge encodes (1,2 = center/bottom)
    };
  }
  get debugDefCount(): number {
    return this.defNext;
  }
  /** DEBUG: decode a billboard_data slot — position back to px + z + rotation + def_index. */
  debugBillboard(index: number): { pos: [number, number]; z: number; rotation: number; def: number } {
    const base = (BILLBOARD_DATA_BASE + index) * 4;
    const pos = this.dataMirror[base + 1], orient = this.dataMirror[base + 2]; // v2.1: G/B
    return { pos: decodePosition(pos), z: (orient >>> 24) & 0xff, rotation: (orient >>> 22) & 0x3, def: (orient >>> 6) & 0xffff };
  }

  get debugBillboardCount(): number {
    return this.billboardNext - 1; // index 0 is the sentinel
  }
  /** DEBUG (P2): free-list health — `next` high-water, `freed` reusable slots, `live` allocated. */
  get debugBillboardStats(): { next: number; freed: number; live: number } {
    return { next: this.billboardNext - 1, freed: this.billboardFreeList.length, live: this.billboardIndex.size };
  }
  /** DEBUG: decode a light record (the record row). */
  debugLight(k: number): { pos: [number, number]; rgb: [number, number, number]; intensity: number; z: number; reach: number; emitterRadius: number; castShadows: boolean } {
    const b = (LIGHT_BASE + k) * 4;
    const G = this.dataMirror[b + 1], B = this.dataMirror[b + 2], A = this.dataMirror[b + 3]; // v2.1
    return {
      pos: decodePosition(G),
      rgb: [(B >>> 24) & 0xff, (B >>> 16) & 0xff, (B >>> 8) & 0xff],
      intensity: B & 0xff,
      z: (A >>> 24) & 0xff,
      reach: (A >>> 12) & 0xfff,
      emitterRadius: (A >>> 4) & 0xff,
      castShadows: (A & 1) === 1,
    };
  }

  destroy(): void {
    this.dataRT.destroy(); // wrapped — does not destroy dataTex
    this.scatterGeo.destroy();
    this.scatter.destroy();
    this.cmdTex.destroy();
    this.dataTex.destroy();
  }
}
