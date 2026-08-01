//! The record layer (lighting-rework P1) — `prim_data` + `definition_data`.
//!
//! Layouts are owned by `docs/intent/2026-07-31-rework.md`; this file implements them and must not
//! drift. **Everything is a prim**: one flat `u16` index space, no `set` nibble, no billboard /
//! light / tile taxonomy. Three type lanes (`cast_type`, `receive_type`, `emit_type`) say what a
//! prim *does*, and a definition is 16 sequential px indexed by rotation, so `base + rotation`
//! replaces the n/s caster card, the e/w mirror and the autotile cell table with one add.
//!
//! That flatness is the whole point: the previous design addressed records as `u4 set | u16 id`, and
//! 8 of those 20-bit references never fit a 128-bit texel — which is what every failed storage
//! attempt was really about (strip I1). A `u16` reference makes 8-per-texel exact.
//!
//! ```
//! prim_data (1 px)
//!   RED    u16 unit.x            | u16 unit.y
//!   GREEN  u8  unit.z            | u4 fine.x | u4 fine.y | u16 definition_index
//!   BLUE   u2  cast_type         | u2 receive_type | u2 emit_type | u4 layer
//!          u8  seed              | u4 rotation     | u4 reach (biased, tiles) | u6 intensity
//!   ALPHA  u8  color.1 | u8 color.2 | u8 color.3 | u8 color.4
//!
//! definition_data (16 px, one per rotation)
//!   RED    u12 frame.x | u12 frame.y | u4 frame.span | u2 anchor.x | u2 anchor.y
//!   GREEN  u8  subframe.x | u8 subframe.y | u8 subframe.width | u8 subframe.height
//!   BLUE   (as prim BLUE — the defaults a prim copies)
//!   ALPHA  u8  color.1 | u8 color.2 | u8 color.3 | u8 color.4
//! ```
//!
//! `frame.span` and `subframe.width`/`height` are stored **biased**: 0 means 1, 15 means 16. A span
//! of 0 would be no frame, so the natural encoding wastes the one value it cannot use — and gets the
//! maximum wrong by one. `subframe.x`/`y` are genuine 0-based offsets and take **no** bias; the two
//! conventions sit in adjacent lanes of the same channel, which is exactly why it is written here.

import { Texture } from "../../gl";
import { UNITS_PER_TILE } from "./squareMath";

/** Index 0 is the global sentinel in EVERY index space — "no prim", "no definition", "no caster",
 *  "empty slot". One comparison covers them all, and no magic value is carved out of the `u16`
 *  space. Allocation must never return it ([F8](docs/work/2026-07-31-lighting-rework/forks.md#f8)). */
export const INDEX_NONE = 0;

/** Rotations per definition — the allocation stride. 4 for a billboard (n/e/s/w), 16 for a linked
 *  autotile; the block is always 16 so `base + rotation` needs no stride lookup. */
export const ROTATIONS_PER_DEF = 16;

/** Widest texture a definition may span, in tiles. */
export const MAX_TEXTURE_SPAN = 16;

const PRIM_W = 1024, PRIM_H = 64;      // 65 536 prims = the whole u16 space, 1 MiB
const DEF_W = 1024, DEF_H = 16;        // 16 384 px = 1024 definitions x 16 rotations, 256 KiB
/** Per-TILE maps. One region (256x256 tiles) folded toroidally — the window is far under a region,
 *  so two tiles sharing a residue can never be on screen together. 1 MiB each. */
const TILE_DIM = 256;
/** Slots per tile in `presence` and `light`: 8 x u16 = 128 bits = exactly one px. */
export const TILE_SLOTS = 8;

/** Lights per texel in the slot map — 8 px, one per light ([F3](forks.md#f3): the map is 8x wide and
 *  a fragment derives its light from x, so all 8 draw in ONE pass with no MRT). */
export const LIGHT_SLOTS = 8;

/** Intensity is `u6` — 64 levels across the 0..4 overbright range (lighting-correctness P1: the
 *  old `u10` gave four bits to REACH, by user directive — reach in the data beats deriving it). */
export const INTENSITY_MAX = 63;
/** Reach is STORED — `u4`, biased +1 → 1..16 TILES ([F1](../../../../docs/work/2026-07-31-lighting-correctness/forks.md)).
 *  16 caps the lane deliberately: reach is the measured cost dial, and 16 tiles is the ceiling every
 *  headline number is priced at — the encoding refuses the value the system is known to choke on. */
export const REACH_MAX_TILES = 16;

/** The BLUE-lane light decodes, shared by every shader that reads an emitter — one definition, so
 *  the CPU registration set and the GPU walk bound cannot disagree (the property the retired
 *  `reachFromIntensity` had; STORED reach keeps it by making both read the same lane). */
export const LIGHT_LANES_GLSL = /* glsl */ `
const float INTENSITY_MAX = ${INTENSITY_MAX}.0;
// lighting-correctness P1: reach is STORED (u4 biased +1 = 1..16 tiles, BLUE bits 6-9);
// intensity is u6 (bits 0-5). Reach bounds registration + walks; falloff shapes within it.
uint  intensityFromB(uint B) { return B & 0x3Fu; }
float reachUnitsFromB(uint B) { return float(((B >> 6u) & 0xFu) + 1u) * ${UNITS_PER_TILE}.0; }
// lighting-visual P4: the falloff SCALES WITH THE STORED REACH (the old d0 was pinned at one
// tile, so a reach-16 light died within ~3 tiles: 1/257 of I at its own boundary). d0 = reach/2
// keeps half intensity at mid-pool; a linear feather over the last 15% lands exactly 0 AT reach,
// so the registration boundary and the visible pool edge are the same line. ONE definition,
// consumed by both slot writers — the delta path stays bit-exact by construction.
float lightFalloff(float d, float reach, float I) {
  float d0 = reach * 0.5;
  float at = I / (1.0 + (d / d0) * (d / d0));
  return at * clamp((reach - d) / (0.15 * reach), 0.0, 1.0);
}
// lighting-visual P5: FINE position — the u4 sixteenth lanes (G bits 20-23 / 16-19) refine the
// u16 integer unit, so a gliding mover's light, card and shadow move SMOOTHLY instead of
// stepping a whole unit at a time. ONE decode; every consumer of a record position calls it.
vec2 primPos(uvec4 rec) {
  return vec2(float(rec.x >> 16)     + float((rec.y >> 20) & 0xFu) / 16.0,
              float(rec.x & 0xffffu) + float((rec.y >> 16) & 0xFu) / 16.0);
}
`;

/** The shared occlusion test (lighting-correctness P3) — ONE definition, injected into the
 *  gather AND the refine (they drifted as duplicates before; a drift here is a shadow that
 *  exists at 16/tile and vanishes at 64/tile, or vice versa). Requires `fetchPrim`,
 *  `fetchDef` and `uSurfaceAtlas` in the including shader.
 *
 *  The caster model: a vertical CARD. `cast_type 1` — the card lies along x (the drawn
 *  frame), placed FRAME-ANCHORED (`subX` off the frame's left edge, the frame centred on the
 *  prim), so an off-centre sprite's card sits where its art sits; rotation 3 (west) MIRRORS
 *  the placement and the sample, so the data stays the plain east frame. `cast_type 2` — an
 *  n/s-facing mover: the card is PERPENDICULAR (along y, `[C.y − W, C.y]`), silhouetted by
 *  the block's r1 (side/east) frame; a south facing flips the sample so the head end tracks
 *  the facing (the old D3 contract, now one `base + rotation` fetch). Heights are measured
 *  from the ANCHOR (lighting-visual P1): the bbox is BOTTOM-ALIGNED, the caster occupies
 *  `[0, subH]` from its base — `subY` stays purely the art's atlas address.
 *  `targetH` is the LIT POINT's height (0 = ground; a billboard texel's height up its card),
 *  so shadows land on billboards through the same solve. The def's SEED lane carries the
 *  frame's atlas scale (`pxPerUnit × 8`) — the old hardcoded ×8 was only true at the 128-px
 *  lod. */
export const OCCLUSION_GLSL = /* glsl */ `
// The def's ATLAS PAGE rides the (otherwise unread) anchor.x lane: frames spread across
// LodPool pages, and sampling a page-1 frame on page 0 reads another stem's silhouette
// (P3 — the residual slab). Two pages bind; a def past them takes offPage — the caller's
// conservative answer (casters: the solid box; receivers: not covered).
bool silhouetteHit(uvec4 d, float fracX, float fracY, bool offPage) {
  int fx = int(d.x >> 20), fy = int((d.x >> 8) & 0xfffu);
  int span = int((d.x >> 4) & 0xfu) + 1;
  int subX = int(d.y >> 24), subY = int((d.y >> 16) & 0xffu);
  int subW = int((d.y >> 8) & 0xffu) + 1, subH = int(d.y & 0xffu) + 1;
  float ppu = float((d.z >> 14) & 0xffu) / 8.0;   // the def SEED lane: atlas px per unit x 8
  if (ppu <= 0.0) { ppu = 8.0; }
  int frameUnits = span * 16;
  // P4: clamp the sampled texel INSIDE the subframe's art rows/cols — frac = 1.0 (the caster's
  // BASE: fracY is measured top-down) otherwise lands one texel PAST the last art row, reads
  // transparent, and the shadow detaches from the feet by a texel band.
  int ox = clamp(int(fracX * float(subW) * ppu), 0, max(0, int(float(subW) * ppu) - 1));
  int oy = clamp(int(fracY * float(subH) * ppu), 0, max(0, int(float(subH) * ppu) - 1));
  int px = int(float(fx + subX) * ppu) + ox;
  int py = int(float(fy + frameUnits + subY) * ppu) + oy;
  uint page = (d.x >> 2) & 3u;
  if (page == 0u) return texelFetch(uSurfaceAtlas, ivec2(px, py), 0).b > 0.35;
  if (page == 1u) return texelFetch(uSurfaceAtlas2, ivec2(px, py), 0).b > 0.35;
  return offPage;
}
bool occludesAt(uint c, vec2 L, float Lz, vec2 P, float targetH) {
  if (c == 0u) return false;
  uvec4 rec = fetchPrim(c);
  uint ct = (rec.z >> 30) & 3u;
  if (ct == 0u) return false;
  vec2 C = primPos(rec);            // P5: fine-refined — the card glides with the mover
  uint block = rec.y & 0xffffu;
  uint prot  = (rec.z >> 10) & 0xfu;
  if (ct == 2u) {
    uvec4 d = fetchDef(block, 1u);                 // the SIDE frame silhouettes the ns card
    int subWi = int((d.y >> 8) & 0xffu) + 1, subHi = int(d.y & 0xffu) + 1;
    float W = float(subWi);
    // lighting-visual P1: the bbox is BOTTOM-ALIGNED to the anchor — the caster occupies
    // [0, subH] from its base; subY stays an ATLAS address only.
    float hTop = float(subHi), hBot = 0.0;
    float dx = P.x - L.x;
    if (abs(dx) < 1e-4) return false;
    float t = (C.x - L.x) / dx;
    if (t <= 0.0 || t >= 1.0) return false;
    float y = L.y + t * (P.y - L.y);
    if (y > C.y || y < C.y - W) return false;
    float h = mix(Lz, targetH, t);
    if (h > hTop || h < hBot) return false;
    float frac = (C.y - y) / W;
    if (prot == 0u) { frac = 1.0 - frac; }         // south-facing: the head end flips
    return silhouetteHit(d, frac, (hTop - h) / float(subHi), true);
  }
  uvec4 d = fetchDef(block, prot);
  int subXi = int(d.y >> 24);
  int subWi = int((d.y >> 8) & 0xffu) + 1, subHi = int(d.y & 0xffu) + 1;
  int spanI = int((d.x >> 4) & 0xfu) + 1;
  float fu = float(spanI * 16);
  // P1: bottom-aligned — the caster occupies [0, subH] from its base (the drawn feet).
  float hTop = float(subHi), hBot = 0.0;
  float dy = P.y - L.y;
  if (abs(dy) < 1e-4) return false;
  float t = (C.y - L.y) / dy;
  if (t <= 0.0 || t >= 1.0) return false;
  float x = L.x + t * (P.x - L.x);
  // FRAME-ANCHORED placement; rotation 3 mirrors placement AND sample about the frame centre.
  float left = prot == 3u
    ? C.x + fu * 0.5 - float(subXi + subWi)
    : C.x - fu * 0.5 + float(subXi);
  if (x < left || x > left + float(subWi)) return false;
  float h = mix(Lz, targetH, t);
  if (h > hTop || h < hBot) return false;
  float frac = (x - left) / float(subWi);
  if (prot == 3u) { frac = 1.0 - frac; }
  return silhouetteHit(d, frac, (hTop - h) / float(subHi), true);
}
bool occludes(uint c, vec2 L, float Lz, vec2 P) { return occludesAt(c, L, Lz, P, 0.0); }
`;

/** Per-light contributions are stored DIVIDED BY 4 ([F2](forks.md#f2)).
 *
 *  `RGB10_A2` is normalised 0..1, but the blit this replaces clamped at `vec3(4.0)` — **4x overbright
 *  is shipped behaviour**, not an accident. Storing `c/4` keeps that ceiling; ten bits over a 0..4
 *  range still gives 256 levels per unit interval, which is exactly the precision the old 8-bit-per-
 *  channel accumulator delivered. Same 4 bytes as RGBA8, and fixed-point so blending is core ES 3.0. */
export const LIGHT_SCALE = 4;

/** Encode a linear contribution into a `RGB10_A2` channel value (0..1023). */
export function encodeLightChannel(c: number): number {
  return Math.round(Math.max(0, Math.min(1, c / LIGHT_SCALE)) * 1023);
}

/** Decode it back. `encode -> decode` is lossy only by the 10-bit quantum. */
export function decodeLightChannel(v: number): number {
  return (v / 1023) * LIGHT_SCALE;
}

/** Dev-mode lane assertion. A value too wide for its lane would otherwise truncate SILENTLY and
 *  surface as a wrong sprite or a phantom shadow — the failure class that costs a day on the GPU
 *  because nothing on the CPU side ever complained. */
function fit(value: number, bits: number, lane: string): number {
  const max = bits === 32 ? 0xffffffff : (1 << bits) - 1;
  if (!Number.isInteger(value) || value < 0 || value > max) {
    throw new Error(`[records] ${lane}: ${value} does not fit u${bits} (0..${max})`);
  }
  return value;
}

/** Bias a 1-based extent into its lane: 1 stores as 0, `2^bits` stores as `2^bits - 1`. */
function biased(value: number, bits: number, lane: string): number {
  return fit(value - 1, bits, `${lane} (biased, 1..${1 << bits})`);
}

export interface PrimFields {
  unitX: number; unitY: number; unitZ?: number;
  fineX?: number; fineY?: number;
  definition: number;
  rotation?: number;
  castType?: number; receiveType?: number; emitType?: number;
  layer?: number; seed?: number;
  /** 0..[`INTENSITY_MAX`] (u6). */
  intensity?: number;
  /** Emitter reach in TILES, 1..[`REACH_MAX_TILES`] — stored biased (default 1). */
  reach?: number;
  colors?: [number, number, number, number];
}

export interface DefinitionFields {
  frameX: number; frameY: number;
  /** Span in TILES, 1..16 — stored biased. */
  frameSpan: number;
  anchorX?: number; anchorY?: number;
  subX?: number; subY?: number;
  /** Extents in UNITS, 1..256 — stored biased. */
  subW?: number; subH?: number;
  castType?: number; receiveType?: number; emitType?: number;
  layer?: number; seed?: number; intensity?: number; reach?: number;
  colors?: [number, number, number, number];
}

/** `prim_data` + `definition_data`, their CPU mirrors, and the allocators.
 *
 *  Two separate textures rather than banded regions of one: with no `set` nibble there is nothing to
 *  gain from packing them together, and separate textures mean an index is just an index. */
export class Records {
  readonly primTex: Texture;
  readonly defTex: Texture;
  readonly primMirror = new Uint32Array(PRIM_W * PRIM_H * 4);
  readonly defMirror = new Uint32Array(DEF_W * DEF_H * 4);

  readonly presenceTex: Texture;
  readonly lightTex: Texture;
  readonly presenceMirror = new Uint32Array(TILE_DIM * TILE_DIM * 4);
  readonly lightMirror = new Uint32Array(TILE_DIM * TILE_DIM * 4);
  private tileDirty = false;

  /** [F10](forks.md#f10): caps EVICT, and evictions are COUNTED. The old system dropped in silence,
   *  and the failure mode is nasty — a light simply is not there, in one tile, reading as a shader
   *  bug rather than a capacity limit. A counter turns a debugging session into a glance. */
  droppedReceivers = 0;
  droppedLights = 0;

  /** Toroidal fold: world tile → its px in a per-tile map. */
  private static foldTile(tx: number, ty: number): number {
    const x = ((tx % TILE_DIM) + TILE_DIM) % TILE_DIM;
    const y = ((ty % TILE_DIM) + TILE_DIM) % TILE_DIM;
    return y * TILE_DIM + x;
  }

  /** Pack 8 `u16` slots into one px: slot i in lane i>>1, high half when (i&1)==0. */
  private static packSlots(slots: Uint16Array, mirror: Uint32Array, base: number): boolean {
    let changed = false;
    for (let c = 0; c < 4; c++) {
      const v = (((slots[c * 2] & 0xffff) << 16) | (slots[c * 2 + 1] & 0xffff)) >>> 0;
      if (mirror[base + c] !== v) { mirror[base + c] = v; changed = true; }
    }
    return changed;
  }

  /** `presence` — the receivers ON a tile, **layer-sorted, topmost LAST**, slot 0 = the tile itself.
   *
   *  The sort is not cosmetic: the per-pixel pass walks `presence[7..1]` and takes the first receiver
   *  covering the pixel, so "topmost last" IS the resolution order. Nothing else enforces it, which
   *  is why it is asserted here rather than assumed by the shader. */
  writePresence(tileX: number, tileY: number, tile: number, receivers: { index: number; layer: number }[]): void {
    // P4 (the one z contract): the sort key IS the draw's zIndex — the same number the
    // painter orders sprites with. A non-finite key would sort arbitrarily and the
    // per-pixel resolution would disagree with the drawn image, silently.
    for (const r of receivers) {
      if (!Number.isFinite(r.layer)) throw new Error(`[records] presence layer not finite: ${r.layer}`);
    }
    const sorted = [...receivers].sort((a, b) => a.layer - b.layer);
    if (sorted.length > TILE_SLOTS - 1) {
      this.droppedReceivers += sorted.length - (TILE_SLOTS - 1);
      sorted.splice(0, sorted.length - (TILE_SLOTS - 1));   // keep the TOPMOST when over cap
    }
    const slots = new Uint16Array(TILE_SLOTS);
    slots[0] = tile & 0xffff;
    for (let i = 0; i < sorted.length; i++) slots[i + 1] = sorted[i].index & 0xffff;
    const base = Records.foldTile(tileX, tileY) * 4;
    if (Records.packSlots(slots, this.presenceMirror, base)) this.tileDirty = true;
  }

  /** `light` — the 8 NEAREST light prims reaching a tile. Reach is the light's STORED reach in
   *  TILES (lighting-correctness P1): the same lane the GPU walk bound reads, so the registered
   *  set and the walked set cannot disagree — the property F6 derived, now held by construction. */
  buildLights(lights: { index: number; tileX: number; tileY: number; reach: number }[]): void {
    const acc = new Map<number, { index: number; d2: number }[]>();
    for (const L of lights) {
      const reach = Math.max(0, Math.min(REACH_MAX_TILES, Math.ceil(L.reach)));
      for (let dy = -reach; dy <= reach; dy++) {
        for (let dx = -reach; dx <= reach; dx++) {
          const d2 = dx * dx + dy * dy;
          if (d2 > reach * reach) continue;                 // circular reach, not a square box
          const key = Records.foldTile(L.tileX + dx, L.tileY + dy);
          let list = acc.get(key);
          if (!list) acc.set(key, (list = []));
          list.push({ index: L.index, d2 });
        }
      }
    }
    const slots = new Uint16Array(TILE_SLOTS);
    for (const [key, list] of acc) {
      list.sort((a, b) => a.d2 - b.d2);                     // nearest-N eviction
      if (list.length > TILE_SLOTS) this.droppedLights += list.length - TILE_SLOTS;
      slots.fill(0);
      for (let i = 0; i < Math.min(TILE_SLOTS, list.length); i++) slots[i] = list[i].index & 0xffff;
      if (Records.packSlots(slots, this.lightMirror, key * 4)) this.tileDirty = true;
    }
  }

  /** Clear the WHOLE light map (lighting-correctness P1b) — `buildLights` writes only the
   *  tiles its lights currently cover, so a mover's OLD coverage would otherwise linger as
   *  stale registrations (pinned in I1). The reconciler clears + rebuilds on any emitter
   *  change; 256 KiB of fill is nothing against a silent ghost light. */
  clearLights(): void {
    let changed = false;
    for (let i = 0; i < this.lightMirror.length; i++) {
      if (this.lightMirror[i] !== 0) { this.lightMirror[i] = 0; changed = true; }
    }
    if (changed) this.tileDirty = true;
  }

  /** DEBUG: the 8 slots of a per-tile map at a world tile. */
  slotsAt(map: "presence" | "light", tileX: number, tileY: number): number[] {
    const m = map === "presence" ? this.presenceMirror : this.lightMirror;
    const b = Records.foldTile(tileX, tileY) * 4;
    const out: number[] = [];
    for (let c = 0; c < 4; c++) { out.push(m[b + c] >>> 16, m[b + c] & 0xffff); }
    return out;
  }

  /** One past the highest prim index ever handed out. **Readable** so debug probes can walk the
   *  live set — z-positioning P0 needed exactly this to answer "is anything elevated at all", and
   *  the answer was twice assumed rather than checked. Allocation stays internal. */
  primNext = 1;                         // 0 is the sentinel and is never handed out
  private defNext = 1;                  // definition BLOCK index; px base = block * ROTATIONS_PER_DEF
  private readonly primFree: number[] = [];
  /** Rotations actually allocated per definition block — the clamp bound for [F7](forks.md#f7). */
  private readonly defRotations = new Map<number, number>();
  private primDirty = false;
  private defDirty = false;

  constructor(gl: WebGL2RenderingContext) {
    this.primTex = new Texture(gl, { width: PRIM_W, height: PRIM_H, format: "rgba32uint" });
    this.defTex = new Texture(gl, { width: DEF_W, height: DEF_H, format: "rgba32uint" });
    this.presenceTex = new Texture(gl, { width: TILE_DIM, height: TILE_DIM, format: "rgba32uint" });
    this.lightTex = new Texture(gl, { width: TILE_DIM, height: TILE_DIM, format: "rgba32uint" });
  }

  /** A fresh prim index. Never 0 ([F8](forks.md#f8)); reuses a freed slot before growing. */
  allocPrim(): number {
    const idx = this.primFree.pop() ?? this.primNext++;
    if (idx === INDEX_NONE) throw new Error("[records] allocPrim returned the sentinel");
    fit(idx, 16, "prim index");
    return idx;
  }

  freePrim(idx: number): void {
    if (idx === INDEX_NONE) return;
    this.writePrimRaw(idx, 0, 0, 0, 0);
    this.primFree.push(idx);
  }

  /** A definition BLOCK. `rotations` is how many of the 16 px are real; the rest stay zero and the
   *  writer clamps against this count, so `base + rotation` can never read the next definition. */
  allocDefinition(rotations: number): number {
    if (rotations < 1 || rotations > ROTATIONS_PER_DEF) {
      throw new Error(`[records] rotations ${rotations} outside 1..${ROTATIONS_PER_DEF}`);
    }
    const block = this.defNext++;
    if (block * ROTATIONS_PER_DEF + ROTATIONS_PER_DEF > DEF_W * DEF_H) {
      throw new Error("[records] definition_data full");
    }
    this.defRotations.set(block, rotations);
    fit(block, 16, "definition index");
    return block;
  }

  /** How many rotations a definition block really has (0 if never allocated). */
  rotationsOf(block: number): number {
    return this.defRotations.get(block) ?? 0;
  }

  /** Clamp a desired rotation into a definition's real allocation.
   *
   *  [F7](forks.md#f7): validate on WRITE, not on read. There are no spare bits for a
   *  `rotation_count` lane, and the GPU should not pay a bounds check per fragment for something the
   *  writer already knows — so the writer earns the trust the hot loop extends. */
  clampRotation(block: number, rotation: number): number {
    const n = this.rotationsOf(block);
    if (n === 0) return 0;
    if (rotation < 0 || rotation >= n) {
      if (import.meta.env?.DEV) {
        console.warn(`[records] rotation ${rotation} outside definition ${block}'s ${n} — clamped`);
      }
      return Math.max(0, Math.min(n - 1, rotation));
    }
    return rotation;
  }

  writePrim(idx: number, f: PrimFields): void {
    fit(idx, 16, "prim index");
    const rot = this.clampRotation(f.definition, f.rotation ?? 0);
    const c = f.colors ?? [0, 0, 0, 0];
    const R = (fit(f.unitX, 16, "unit.x") * 0x10000 + fit(f.unitY, 16, "unit.y")) >>> 0;
    const G = ((fit(f.unitZ ?? 0, 8, "unit.z") << 24)
             | (fit(f.fineX ?? 0, 4, "fine.x") << 20)
             | (fit(f.fineY ?? 0, 4, "fine.y") << 16)
             | fit(f.definition, 16, "definition_index")) >>> 0;
    const B = this.packTypeLanes(f, rot);
    const A = ((fit(c[0], 8, "color.1") << 24) | (fit(c[1], 8, "color.2") << 16)
             | (fit(c[2], 8, "color.3") << 8) | fit(c[3], 8, "color.4")) >>> 0;
    this.writePrimRaw(idx, R, G, B, A);
  }

  /** BLUE, shared by both records — a prim COPIES the definition's lanes and may override.
   *  Bits 0–9 (lighting-correctness P1): `u4 reach (6–9, biased +1 = 1..16 tiles) | u6 intensity`. */
  private packTypeLanes(f: PrimFields | DefinitionFields, rot: number): number {
    return ((fit(f.castType ?? 0, 2, "cast_type") << 30)
          | (fit(f.receiveType ?? 0, 2, "receive_type") << 28)
          | (fit(f.emitType ?? 0, 2, "emit_type") << 26)
          | (fit(f.layer ?? 0, 4, "layer") << 22)
          | (fit(f.seed ?? 0, 8, "seed") << 14)
          | (fit(rot, 4, "rotation") << 10)
          | (biased(f.reach ?? 1, 4, "reach") << 6)
          | fit(f.intensity ?? 0, 6, "intensity")) >>> 0;
  }

  writeDefinition(block: number, rotation: number, f: DefinitionFields): void {
    const n = this.rotationsOf(block);
    if (rotation < 0 || rotation >= n) {
      throw new Error(`[records] definition ${block} has ${n} rotation(s); cannot write ${rotation}`);
    }
    // lighting-correctness P2: the subframe must sit INSIDE the frame — a bbox past the frame
    // edge samples a NEIGHBOUR definition's art in the refine, silently (the GPU never checks).
    const frameUnits = f.frameSpan * 16;
    if ((f.subX ?? 0) + (f.subW ?? 1) > frameUnits || (f.subY ?? 0) + (f.subH ?? 1) > frameUnits) {
      throw new Error(
        `[records] definition ${block} r${rotation}: subframe (${f.subX ?? 0},${f.subY ?? 0} ` +
        `${f.subW ?? 1}x${f.subH ?? 1}) exceeds the ${frameUnits}-unit frame`,
      );
    }
    const px = block * ROTATIONS_PER_DEF + rotation;
    const c = f.colors ?? [0, 0, 0, 0];
    const R = ((fit(f.frameX, 12, "frame.x") << 20)
             | (fit(f.frameY, 12, "frame.y") << 8)
             | (biased(f.frameSpan, 4, "frame.span") << 4)
             | (fit(f.anchorX ?? 0, 2, "anchor.x") << 2)
             | fit(f.anchorY ?? 0, 2, "anchor.y")) >>> 0;
    const G = ((fit(f.subX ?? 0, 8, "subframe.x") << 24)
             | (fit(f.subY ?? 0, 8, "subframe.y") << 16)
             | (biased(f.subW ?? 1, 8, "subframe.width") << 8)
             | biased(f.subH ?? 1, 8, "subframe.height")) >>> 0;
    const B = this.packTypeLanes(f, rotation);
    const A = ((fit(c[0], 8, "color.1") << 24) | (fit(c[1], 8, "color.2") << 16)
             | (fit(c[2], 8, "color.3") << 8) | fit(c[3], 8, "color.4")) >>> 0;
    const b = px * 4;
    const m = this.defMirror;
    if (m[b] === R && m[b + 1] === G && m[b + 2] === B && m[b + 3] === A) return;
    m[b] = R; m[b + 1] = G; m[b + 2] = B; m[b + 3] = A;
    this.defDirty = true;
  }

  private writePrimRaw(idx: number, R: number, G: number, B: number, A: number): void {
    const b = idx * 4;
    const m = this.primMirror;
    if (m[b] === R && m[b + 1] === G && m[b + 2] === B && m[b + 3] === A) return;
    m[b] = R; m[b + 1] = G; m[b + 2] = B; m[b + 3] = A;
    this.primDirty = true;
  }

  /** Push whichever mirrors changed. Whole-texture for now — 1.25 MiB total, and a dirty-rect upload
   *  is an optimisation to make against a measurement, not on principle. */
  upload(gl: WebGL2RenderingContext): void {
    if (this.primDirty) {
      gl.bindTexture(gl.TEXTURE_2D, this.primTex.handle);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, PRIM_W, PRIM_H, gl.RGBA_INTEGER, gl.UNSIGNED_INT, this.primMirror);
      this.primDirty = false;
    }
    if (this.defDirty) {
      gl.bindTexture(gl.TEXTURE_2D, this.defTex.handle);
      gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, DEF_W, DEF_H, gl.RGBA_INTEGER, gl.UNSIGNED_INT, this.defMirror);
      this.defDirty = false;
    }
    if (this.tileDirty) {
      for (const [tex, data] of [[this.presenceTex, this.presenceMirror], [this.lightTex, this.lightMirror]] as const) {
        gl.bindTexture(gl.TEXTURE_2D, tex.handle);
        gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, TILE_DIM, TILE_DIM, gl.RGBA_INTEGER, gl.UNSIGNED_INT, data);
      }
      this.tileDirty = false;
    }
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  /** DEBUG: decode a prim record back to fields, for asserting what was actually written. */
  debugPrim(idx: number): Record<string, number> {
    const b = idx * 4, m = this.primMirror;
    return {
      unitX: m[b] >>> 16, unitY: m[b] & 0xffff,
      unitZ: m[b + 1] >>> 24, fineX: (m[b + 1] >>> 20) & 0xf, fineY: (m[b + 1] >>> 16) & 0xf,
      definition: m[b + 1] & 0xffff,
      castType: (m[b + 2] >>> 30) & 3, receiveType: (m[b + 2] >>> 28) & 3,
      emitType: (m[b + 2] >>> 26) & 3, layer: (m[b + 2] >>> 22) & 0xf,
      seed: (m[b + 2] >>> 14) & 0xff, rotation: (m[b + 2] >>> 10) & 0xf,
      reach: ((m[b + 2] >>> 6) & 0xf) + 1,                       // un-bias → tiles
      intensity: m[b + 2] & 0x3f,
      tileX: Math.floor((m[b] >>> 16) / UNITS_PER_TILE), tileY: Math.floor((m[b] & 0xffff) / UNITS_PER_TILE),
    };
  }

  /** DEBUG: decode a definition px (block + rotation). */
  debugDefinition(block: number, rotation = 0): Record<string, number> {
    const b = (block * ROTATIONS_PER_DEF + rotation) * 4, m = this.defMirror;
    return {
      frameX: m[b] >>> 20, frameY: (m[b] >>> 8) & 0xfff,
      frameSpan: ((m[b] >>> 4) & 0xf) + 1,                       // un-bias
      anchorX: (m[b] >>> 2) & 3, anchorY: m[b] & 3,
      subX: m[b + 1] >>> 24, subY: (m[b + 1] >>> 16) & 0xff,
      subW: ((m[b + 1] >>> 8) & 0xff) + 1, subH: (m[b + 1] & 0xff) + 1,
      rotation: (m[b + 2] >>> 10) & 0xf,
    };
  }

  get stats(): Record<string, number> {
    return { prims: this.primNext - 1, freed: this.primFree.length, definitions: this.defNext - 1,
             droppedReceivers: this.droppedReceivers, droppedLights: this.droppedLights };
  }
}
