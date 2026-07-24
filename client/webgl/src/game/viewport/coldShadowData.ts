//! ColdShadowData — the ONE unified `RGBA32UI` data texture the shadow gather reads via `texelFetch`
//! (`2026-07-23-unified-data`). 1024×1024, linear index `i → (i & 1023, i >> 10)`, 64-row bands of
//! 65 536 one-texel slots each; bit layouts authoritative in `docs/VARIABLES.md` §Cold shadow data:
//!
//!   rows   0–63    prim_definition_data  (band base 0)       1 px/def — IMMUTABLE, keyed (stem,cell,lod)
//!   rows  64–127   prim_data             (band base 65 536)  1 px/placed caster (2/px retired — F1)
//!   rows 128–191   light_data            (band base 131 072) 1 px/light record
//!   rows 192–1022  reserved              (materials era)
//!   row  1023      constants             (window mapping — P3)
//!
//! Transport: dirty ROW-SPAN `texSubImage2D` (P1; the P2 command-buffer scatter replaces it).
//! World unit: `1 unit = SQUARE/16 = 4px`, compile-time (§Unit). Every world field is in units; only the
//! atlas `frame_*` are texture px. Positions pack as `position_anchor_reference` (region|zone|tile|anchor).

import { Renderer, Texture, Program, Geometry, RenderTarget } from "../../gl";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE, UNIT, ZONE_DIM, REGION_DIM } from "./squareMath";

/** Fixed light count (bits in the shadow bitfield — the light BAND holds 65 536 slots of headroom). */
export const N_LIGHTS = 128;
/** The unified data texture: 1024×1024, linear index i → (i & 1023, i >> 10). */
export const DATA_W = 1024;
export const DATA_H = 1024;
/** Band bases (linear indices) — 64-row bands of 65 536 one-texel slots. Mirrored in the GLSL. */
export const DEF_BASE = 0;
export const PRIM_BASE = 65536;
export const LIGHT_BASE = 131072;
/** Tile-keyed sets — region-torus addressed (presence-in-data). One set = one region's 65 536 tiles. */
export const PRESENCE_BASE = 3 * 65536;    // light_presence_lo (light slots 0–6)
export const CASTER_BASE = 4 * 65536;
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
/** Command buffer v2 (user format): 64×64 RGBA32UI, replay-idempotent absolute writes. One FILL =
 *  a header px + 16 per-SET sections. Header: 16× u6 per-set command counts (5 per RGB lane at
 *  bits 0/6/12/18/24 + the 16th in A) + u8 opcode (A bits 0–7; 0 = write-data — presence/other
 *  maps ride future opcodes) + reserved. A set's section = ceil(n/8) ADDRESS px (8× u16 in-set
 *  addresses) + n PAYLOAD px. u6 caps a set at 63 updates/fill — bursts batch (fills are ~1 KB). */
const CMD_W = 64;
const CMD_H = 64;
/** The data texture's 16 u16-addressable SETS (1024×64 each; set = linear >> 16). */
const SET_SHIFT = 16;
const SET_COUNT = 16;
const MAX_PER_SET = 63; // u6 count per fill

/** Scatter v2: slot index → scan the header's per-set counts to find the owning set + in-section
 *  offset → u16 in-set address → point at the target texel; fragment writes the payload px.
 *  Constant-bound scan (16 sets), break inside — never a body-modified loop condition. */
const SCATTER_VERT = /* glsl */ `#version 300 es
precision highp int;
uniform highp usampler2D uCmd;
uniform int uCmdBase;            // px index of this fill's header
in uint aIndex;                  // command slot within the fill (0..count-1)
flat out highp int vPayload;     // px index of this command's payload
uvec4 px(int i) { return texelFetch(uCmd, ivec2(i & 63, i >> 6), 0); }
uint laneOf(uvec4 v, int lane) { return lane == 0 ? v.x : (lane == 1 ? v.y : (lane == 2 ? v.z : v.w)); }
int countOf(uvec4 hdr, int b) {  // 16× u6: 5 per RGB lane (bits 0/6/12/18/24), 16th in A bits 8–13
  if (b >= 15) return int((hdr.w >> 8) & 63u);
  int lane = b / 5;
  int sh = (b - lane * 5) * 6;
  return int((laneOf(hdr, lane) >> uint(sh)) & 63u);
}
void main() {
  uvec4 hdr = px(uCmdBase);
  int rem = int(aIndex);
  int sec = uCmdBase + 1;        // pure-payload sections start after the header px (v2.1: NO
  int band = 15;                 // address blocks — every record carries its u16 id in R's high half)
  for (int b = 0; b < 16; b++) { // constant bound; break inside
    int c = countOf(hdr, b);
    if (rem < c) { band = b; break; }
    rem -= c;
    sec += c;
  }
  vPayload = sec + rem;
  uint id = px(vPayload).x >> 16;                  // the record's own in-set id (self-addressing)
  int target = (band << 16) + int(id);
  float x = (float(target & 1023) + 0.5) / 1024.0 * 2.0 - 1.0;
  float y = (float(target >> 10) + 0.5) / 1024.0 * 2.0 - 1.0;
  gl_Position = vec4(x, y, 0.0, 1.0);
  gl_PointSize = 1.0;
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
/** Empty 7-slot payload — the eviction clear (length 0 → every slot takes the set sentinel). */
const EMPTY7: number[] = [];

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
  /** P3 tight bbox per def (WORLD px, relative to the prim's top-left): `dx,dy` = offset to the
   *  opaque region, `w,h` = its size. Fraction-of-frame × prim size, so it's LOD-independent. */
  private readonly defTight = new Map<number, { dx: number; dy: number; w: number; h: number }>();
  /** The ONE atlas page the casters' surface frames live on (F2: single page) — bound as `uSurface`
   *  for the P4 silhouette sample. Adopted from the first resolved frame; a frame on any OTHER page
   *  gets no silhouette (solid quad) until multi-page lands (caster-lut C5). */
  private surfacePageTex: Texture | null = null;
  private pageWarned = false;
  private frameSizeWarned = false;

  /** prim.id → allocated prim_data_index (≥1; index 0 is the sentinel). */
  private readonly primIndex = new Map<number, number>();
  private primNext = 1; // 0 reserved as the sentinel
  /** Freed prim_data indices (P2 free-list) — reused before bumping `primNext`, so the id space
   *  survives pan/zone churn (high-water bounded by peak concurrent casters ≪ 65 536). */
  private readonly primFreeList: number[] = [];
  private lightCount = 0;

  constructor(private readonly renderer: Renderer) {
    const gl = renderer.gl;
    this.dataTex = new Texture(gl, { width: DATA_W, height: DATA_H, format: "rgba32uint" });
    this.cmdTex = new Texture(gl, { width: CMD_W, height: CMD_H, format: "rgba32uint" });
    this.scatter = new Program(gl, SCATTER_VERT, SCATTER_FRAG, "data-scatter");
    const slots = new Uint32Array(SET_COUNT * MAX_PER_SET); // max commands per fill (16 × 63)
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
    const g = (i: number): number => (off + i < slots.length ? slots[off + i] & 0xffff : empty);
    const R = (((id & 0xffff) << 16) | g(0)) >>> 0;
    const G = ((g(1) << 16) | g(2)) >>> 0;
    const B = ((g(3) << 16) | g(4)) >>> 0;
    const A = ((g(5) << 16) | g(6)) >>> 0;
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
    this.writeTileSet(PRESENCE_HI_BASE, wc, wr, slots, 0xffff, 7);
  }
  /** Caster-bucket tile: 7 u16 prim indices (0 empty — the prim sentinel). */
  writeCasters(wc: number, wr: number, slots: ArrayLike<number>): void {
    this.writeTileSet(CASTER_BASE, wc, wr, slots, 0x0000);
  }
  /** Clear a tile's presence (both sets, all-empty) — eviction. */
  clearPresence(wc: number, wr: number): void {
    this.writeTileSet(PRESENCE_BASE, wc, wr, EMPTY7, 0xffff);
    this.writeTileSet(PRESENCE_HI_BASE, wc, wr, EMPTY7, 0xffff);
  }
  /** Clear a tile's buckets (all-empty) — eviction. */
  clearCasters(wc: number, wr: number): void {
    this.writeTileSet(CASTER_BASE, wc, wr, EMPTY7, 0x0000);
  }

  /** The shared surface atlas page (or null before any sprite resolved). */
  get surfacePage(): Texture | null {
    return this.surfacePageTex;
  }


  /** The `definition_index` for a caster's sprite at its CURRENTLY-RESOLVED lod. Defs are
   *  **immutable — one def per atlas frame** (keyed `stem|cell|lod`): a new lod landing mints a NEW
   *  def instead of rewriting the old one, and `prim_data` keeps pointing at whatever def it holds
   *  until {@link primDataFor} swaps it — which reports `changed` so the caller runs the prim dirty
   *  cascade (prim tiles → reaching lights → their cast regions). The lod-0 def is the LOOSE
   *  fallback (full-footprint box, no frame → solid quad) used until the surface resolves.
   *
   *  The shadow quad = the sprite's minimum bbox (EVEN units) at an unsigned frame-relative offset;
   *  nudges align the sampled window; anchors place the bbox on the prim's position. */
  definitionFor(prim: Primitive, resolver: TextureResolver | null): number {
    if (!prim.textureName) return -1;

    const bbox = resolver ? resolver.opaqueBBox(prim.textureName) : null;
    const surf = resolver && bbox ? resolver.resolve(prim.textureName, "surface", prim.cell).frame : null;

    // Frame world span in TILES (pow2, ≤ ZONE_DIM — u4 stores tiles−1, width = log2(ZONE_DIM)).
    // Footprints are square (aspect plumbing removed); a non-pow2/oversized span is a content bug.
    const stRaw = Math.max(1, Math.round(Math.max(prim.width, prim.height) / SQUARE));
    let st = 1;
    while (st < stRaw) st <<= 1;
    if ((st !== stRaw || st > ZONE_DIM) && !this.frameSizeWarned) {
      this.frameSizeWarned = true;
      console.warn(`[cold-shadow] ${prim.textureName}: footprint ${stRaw} tiles is not pow2 ≤ ${ZONE_DIM} — span rounded`);
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
    // as-is; a new lod mints a NEW index. prim_data keeps whatever def it holds until
    // {@link primDataFor} swaps it — which reports `changed`, and the caller runs the prim dirty
    // cascade. Nothing ever rewrites a def, so def changes ride the prim-update path for free.
    const key = `${prim.textureName}|${prim.cell ?? 0}|${lod}`;
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
      // Opaque run, frame-relative px.
      const rx0 = bbox!.fx * surf.w, ry0 = bbox!.fy * surf.h;
      const rw = Math.max(1, bbox!.fw * surf.w), rh = Math.max(1, bbox!.fh * surf.h);
      // Minimum bbox on the unit grid, top-left biased, rounded out to EVEN units.
      ux0 = Math.floor(rx0 / ppu);
      uy0 = Math.floor(ry0 / ppu);
      wu = Math.ceil((rx0 + rw) / ppu) - ux0;
      hu = Math.ceil((ry0 + rh) / ppu) - uy0;
      if (wu & 1) wu++;
      if (hu & 1) hu++;
      wu = Math.min(wu, spanU); hu = Math.min(hu, spanU);
      if (ux0 + wu > spanU) ux0 = Math.max(0, spanU - wu);
      if (uy0 + hu > spanU) uy0 = Math.max(0, spanU - hu);
      // Nudges (px at this def's lod — per-lod defs make them immutable too). Both SIGNED u12
      // (+2048 bias, F4): full either-direction range; `nudge_anchor` records the alignment
      // (default x centered = 1, y bottom = 2).
      let sx0 = rx0 - (wu * ppu - rw) / 2;
      let sy0 = ry0 + rh - hu * ppu;
      sx0 = Math.min(Math.max(sx0, 0), surf.w - wu * ppu); // window stays inside the frame
      sy0 = Math.min(Math.max(sy0, 0), surf.h - hu * ppu);
      nx = Math.min(Math.max(Math.round(ux0 * ppu - sx0), -2048), 2047);
      ny = Math.min(Math.max(Math.round(uy0 * ppu - sy0), -2048), 2047);
    }
    // Bucketing box (world px, rel. prim top-left) — 1 frame unit ≡ 1 world unit by the span model.
    this.defTight.set(idx, { dx: ux0 * UNIT, dy: uy0 * UNIT, w: wu * UNIT, h: hu * UNIT });

    const base = (DEF_BASE + idx) * 4;
    const page = 0;        // ONE bound surface page today — C5 (texture-array pages) assigns real indices
    const ax = 1, ay = 2;  // shadow casters hang the bbox at the prim's BOTTOM-CENTER anchor
    const nax = 1, nay = 2; // nudge alignment: x centered, y bottom — the default nudging operation
    // v2.1 self-addressing: R = u16 id | u10 offset_x | u6 reserved; G = u9 W | u9 H | u4 span | u10 offset_y.
    this.dataMirror[base] = (((idx & 0xffff) << 16) | ((ux0 & 0x3ff) << 6)) >>> 0;
    this.dataMirror[base + 1] = ((((wu >> 1) & 0x1ff) << 23) | (((hu >> 1) & 0x1ff) << 14) | (((st - 1) & 0xf) << 10) | (uy0 & 0x3ff)) >>> 0;
    this.dataMirror[base + 2] = (((fx16 & 0x3ff) << 22) | ((fy16 & 0x3ff) << 12) | ((page & 0xf) << 8) | ((lod & 0xf) << 4) | ((ax & 3) << 2) | (ay & 3)) >>> 0;
    this.dataMirror[base + 3] = ((((nx + 2048) & 0xfff) << 20) | (((ny + 2048) & 0xfff) << 8) | ((nax & 3) << 6) | ((nay & 3) << 4)) >>> 0;
    this.mark(DEF_BASE + idx);
    return idx;
  }

  /** The tight bbox (WORLD px, relative to the prim's top-left) for a placed caster's def, or null. */
  tightBoxOf(defIndex: number): { dx: number; dy: number; w: number; h: number } | null {
    return this.defTight.get(defIndex) ?? null;
  }


  /** The `prim_data_index` (≥1) for a PLACED caster (position + orientation + its `def_index`),
   *  allocated on first sight + cached by `prim.id`. Defs are immutable, so a texture/lod change
   *  arrives HERE as a `def_index` swap: the orient word rewrites in place and `changed` reports it —
   *  the caller runs the prim dirty cascade (prim tiles → reaching lights → their cast regions).
   *  First sight is `changed` too (the same cascade seeds the new caster's region — no force-all). */
  primDataFor(prim: Primitive, defIndex: number): { idx: number; changed: boolean } {
    const rotation = prim.flipX ? 3 : 1; // W : E (both E/W regime for now)
    const z = 0;
    const orient = ((((z & 0xff) << 24) | ((rotation & 0x3) << 22) | ((defIndex & 0xffff) << 6)) >>> 0);
    const hit = this.primIndex.get(prim.id);
    if (hit !== undefined) {
      const base = (PRIM_BASE + hit) * 4; // v2.1: R = id|reserved, G = position, B = orient, A reserved
      if (this.dataMirror[base + 2] === orient) return { idx: hit, changed: false };
      // RETENTION: "retain whatever the lod was until we get a NEW one" — a new one means a new
      // USABLE one. Never swap a standing prim DOWN to the loose (lod-0) def: if the freshly
      // resolved frame is unusable (off-page — e.g. a pool spill) the prim keeps casting its
      // current silhouette instead of degrading to a solid quad.
      const curDef = (this.dataMirror[base + 2] >>> 6) & 0xffff;
      const newLod = (this.dataMirror[(DEF_BASE + defIndex) * 4 + 2] >>> 4) & 0xf;
      const curLod = (this.dataMirror[(DEF_BASE + curDef) * 4 + 2] >>> 4) & 0xf;
      if (newLod < 4 && curLod >= 4) return { idx: hit, changed: false };
      this.dataMirror[base + 2] = orient; // def swap (new lod) / orientation change — position untouched
      this.mark(PRIM_BASE + hit);
      return { idx: hit, changed: true };
    }
    const idx = this.primFreeList.pop() ?? this.primNext++; // reuse a freed slot before bumping (P2)
    const ax = prim.x + prim.width * 0.5; // the TRUE game anchor (full-box base-centre) — unchanged
    const ay = prim.y + prim.height;
    const base = (PRIM_BASE + idx) * 4; // v2.1: R = u16 id | u16 reserved, G = position, B = orient
    this.dataMirror[base] = ((idx & 0xffff) << 16) >>> 0;
    this.dataMirror[base + 1] = encodePosition(ax, ay); // position_anchor_reference
    // orient: z(24–31) | rotation(22–23) | definition_index(6–21) | reserved(0–5)
    this.dataMirror[base + 2] = orient;
    this.primIndex.set(prim.id, idx);
    this.mark(PRIM_BASE + idx);
    return { idx, changed: true };
  }

  /** Free every allocated prim whose `prim.id` is NOT in `seen` (it left the resident/standing set —
   *  zone evicted or destroyed). Returns its slot to the free-list. No slot clear + no bucket cascade
   *  needed: `buildCasters` rebuilds every in-window bucket from the current `standing` (so a freed id
   *  can't be referenced), and caster REMOVAL force-alls the shadow recompute. Reach gap makes it
   *  safe: `standing` bounds who can cast, so a freed prim is beyond reach of every in-window tile.
   *  Returns the number freed. */
  freePrimsExcept(seen: Set<number>): number {
    let dead: number[] | null = null;
    for (const pid of this.primIndex.keys()) if (!seen.has(pid)) (dead ??= []).push(pid);
    if (!dead) return 0;
    for (const pid of dead) {
      this.primFreeList.push(this.primIndex.get(pid)!);
      this.primIndex.delete(pid);
    }
    return dead.length;
  }

  get lights(): number {
    return this.lightCount;
  }

  /** Build `light_data` (the record row) from the lights, and upload it. **Records only** — casters
   *  are found by the per-tile buckets + corridor sweep now, so there's no LUT to build here (`standing`
   *  is bucketed by `ShadowGather.buildCasters`, which also allocates defs/prims). Call on change. */
  buildLights(lights: ColdLight[]): void {
    const n = Math.min(lights.length, N_LIGHTS);
    const m = this.dataMirror;
    for (let k = 0; k < Math.max(n, this.lightCount); k++) {
      const base = (LIGHT_BASE + k) * 4;
      let R = 0, G = 0, B = 0, A = 0; // k >= n → a removed light zeroes its record
      if (k < n) {
        const L = lights[k];
        // v2.1 self-addressing: R = u16 id | u16 reserved; G = position; B = colour;
        // A = u8 z (24–31) | u12 reach (12–23) | u8 emitter (4–11) | u2 reserved (2–3) | u1 hot (1) | u1 cast_shadows (0).
        R = ((k & 0xffff) << 16) >>> 0;
        G = encodePosition(L.x, L.y);
        const r = clamp(L.color[0] * 255, 255), g = clamp(L.color[1] * 255, 255), b = clamp(L.color[2] * 255, 255);
        B = (((r << 24) | (g << 16) | (b << 8) | clamp(L.intensity * 255, 255)) >>> 0);
        const z = clamp(L.z / UNIT, 255), reach = clamp(L.reach / UNIT, 0xfff), em = clamp(L.emitterRadius / UNIT, 255);
        A = (((z << 24) | (reach << 12) | (em << 4) | (L.hot ? 2 : 0) | (L.castShadows ? 1 : 0)) >>> 0);
      }
      // Compare-write: a STATIC light's record is unchanged → no command (only movers re-scatter).
      if (m[base] !== R || m[base + 1] !== G || m[base + 2] !== B || m[base + 3] !== A) {
        m[base] = R; m[base + 1] = G; m[base + 2] = B; m[base + 3] = A;
        this.mark(LIGHT_BASE + k);
      }
    }
    this.lightCount = n;
  }

  /** Upload any changed textures (call once per frame after populating). Cheap — no-op unless a slot landed. */
  /** Returns true if anything was uploaded (def/prim changed) — the caller re-dirties the shadow. */
  /** DEBUG: commands scattered by the last flush (per-set breakdown). */
  debugLastFlush: Record<number, number> = {};
  flush(): boolean {
    // DEBUG per-set command tally (which sets got writes this flush).
    this.debugLastFlush = {};
    for (const t of this.dirtySet) { const s = t >> SET_SHIFT; this.debugLastFlush[s] = (this.debugLastFlush[s] ?? 0) + 1; }
    if (this.dirtySet.size === 0) return false;
    // v2 fills: bucket the changed texels by SET (linear >> 16), then emit fills until drained.
    // Each fill = header px (16× u6 per-set counts + u8 opcode 0) + per-set sections (address px
    // of 8× u16 in-set addresses + payload pxs from the mirror). u6 caps 63/set/fill — bursts
    // batch; a fill maxes at 1 + 16·(8+63) = 1137 px, always inside the 64×64 buffer.
    const buckets: number[][] = Array.from({ length: SET_COUNT }, () => []);
    for (const t of this.dirtySet) buckets[t >> SET_SHIFT].push(t & 0xffff);
    this.dirtySet.clear();
    const cursors = new Array(SET_COUNT).fill(0) as number[];
    const m = this.cmdMirror;
    for (;;) {
      let total = 0;
      const counts = new Array(SET_COUNT).fill(0) as number[];
      for (let b = 0; b < SET_COUNT; b++) {
        counts[b] = Math.min(MAX_PER_SET, buckets[b].length - cursors[b]);
        total += counts[b];
      }
      if (total === 0) break;
      let pxNeeded = 1;
      for (let b = 0; b < SET_COUNT; b++) pxNeeded += counts[b]; // v2.1: payload-only sections
      const rowsNeeded = Math.ceil(pxNeeded / CMD_W);
      if (this.cmdCursorRow + rowsNeeded > CMD_H) this.cmdCursorRow = 0; // rotate (F3) — never look back
      const basePx = this.cmdCursorRow * CMD_W;
      // Header: 5 counts per RGB lane (bits 0/6/12/18/24), the 16th in A bits 8–13; opcode 0 in A bits 0–7.
      const h = basePx * 4;
      m[h] = 0; m[h + 1] = 0; m[h + 2] = 0;
      for (let b = 0; b < 15; b++) m[h + Math.floor(b / 5)] = (m[h + Math.floor(b / 5)] | (counts[b] << ((b % 5) * 6))) >>> 0;
      m[h + 3] = (((counts[15] & 63) << 8) | 0) >>> 0;
      // Sections, set order.
      let p = basePx + 1;
      for (let b = 0; b < SET_COUNT; b++) {
        const n = counts[b];
        if (n === 0) continue;
        // v2.1 pure payloads — every record self-addresses (u16 id in R's high half).
        for (let i = 0; i < n; i++) {
          const src = ((b << SET_SHIFT) + buckets[b][cursors[b] + i]) * 4;
          const dst = (p + i) * 4;
          m[dst] = this.dataMirror[src];
          m[dst + 1] = this.dataMirror[src + 1];
          m[dst + 2] = this.dataMirror[src + 2];
          m[dst + 3] = this.dataMirror[src + 3];
        }
        cursors[b] += n;
        p += n;
      }
      this.cmdTex.uploadRows(this.cmdCursorRow, rowsNeeded, this.cmdMirror, 4);
      this.renderer.draw({
        program: this.scatter,
        geometry: this.scatterGeo,
        target: this.dataRT,
        blend: "none",
        mode: this.renderer.gl.POINTS,
        count: total,
        textures: { uCmd: this.cmdTex },
        uniforms: (prog) => prog.uInt("uCmdBase", basePx),
      });
      this.cmdCursorRow += rowsNeeded;
    }
    return true;
  }


  // ── DEBUG decoders (verify against the CPU mirrors) ─────────────────────────────────
  debugDef(index: number): {
    prim_width: number; prim_height: number; frame_span: number; offset: [number, number];
    frame_xy: [number, number]; frame_page: number; frame_lod: number; anchor: [number, number];
    nudge: [number, number]; nudge_anchor: [number, number];
  } {
    const b = (DEF_BASE + index) * 4;
    const R = this.dataMirror[b], G = this.dataMirror[b + 1], B = this.dataMirror[b + 2], A = this.dataMirror[b + 3];
    return {
      prim_width: ((G >>> 23) & 0x1ff) * 2,  // units (stored /2 — even bbox)
      prim_height: ((G >>> 14) & 0x1ff) * 2,
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
  /** DEBUG: decode a prim_data slot — position back to px + z + rotation + def_index. */
  debugPrim(index: number): { pos: [number, number]; z: number; rotation: number; def: number } {
    const base = (PRIM_BASE + index) * 4;
    const pos = this.dataMirror[base + 1], orient = this.dataMirror[base + 2]; // v2.1: G/B
    return { pos: decodePosition(pos), z: (orient >>> 24) & 0xff, rotation: (orient >>> 22) & 0x3, def: (orient >>> 6) & 0xffff };
  }

  get debugPrimCount(): number {
    return this.primNext - 1; // index 0 is the sentinel
  }
  /** DEBUG (P2): free-list health — `next` high-water, `freed` reusable slots, `live` allocated. */
  get debugPrimStats(): { next: number; freed: number; live: number } {
    return { next: this.primNext - 1, freed: this.primFreeList.length, live: this.primIndex.size };
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
