//! ColdShadowData — the GPU data textures the shadow gather reads via `texelFetch` (the
//! `2026-07-21-shadow-bitfield` stream). Fixed-size, normalized `RGBA32UI` textures; written on change
//! (CPU-side; no ping-pong), read every frame for free. Bit layouts are authoritative in
//! `docs/VARIABLES.md` §Cold shadow data textures.
//!
//! Layout (fixed slots — `N=128` lights, `u16` indexes):
//!   • `light_data`           128×1 — col = light record (position/colour/reach). The per-light caster
//!                            LUT is retired — casters come from the per-tile buckets (`shadowGather`).
//!   • `prim_data`            256×128 — all placed casters, 2/px (`u64`); carries `u16 definition_index`.
//!                            Index 0 is the SENTINEL (empty/end) → usable 1..65535.
//!   • `prim_definition_data` 256×256 — one px/variant: minimum bbox (EVEN units) at a frame-relative
//!                            offset, pow2-square frame by lod exponent + span + px nudges + 3×3
//!                            anchors (the def-frame-anchors model), keyed by def_index.
//!
//! World unit: `1 unit = SQUARE/16 = 4px`, compile-time (§Unit). Every world field is in units; only the
//! atlas `frame_*` are texture px. Positions pack as `position_anchor_reference` (region|zone|tile|anchor).

import { Renderer, Texture } from "../../gl";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE, UNIT, ZONE_DIM, REGION_DIM } from "./squareMath";

/** Fixed light count (bits in the shadow bitfield). */
export const N_LIGHTS = 128;
/** `light_data` — 128 wide (col = light) × 1 tall (the record row). The per-light caster LUT is
 *  retired: casters are found by the per-tile buckets + corridor sweep (`shadowGather`), not a LUT. */
const LIGHT_W = 128;
const LIGHT_H = 1;
/** `prim_data` — 2 entries/px; `256 × 128` = 65536 slots (index 0 = sentinel). */
const PRIM_W = 256;
const PRIM_H = 128;
/** `prim_definition_data` — 1 px/variant; `256 × 256` = 65536 defs. */
const DEF_W = 256;
const DEF_H = 256;

const clamp = (v: number, hi: number): number => Math.min(Math.max(Math.round(v), 0), hi);

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
  /** `prim_definition_data` — one px/variant: geometry (units) + atlas frame (px) + page. */
  private readonly defTex: Texture;
  private readonly defMirror = new Uint32Array(DEF_W * DEF_H * 4);
  /** stem+cell → allocated definition_index. */
  private readonly defIndex = new Map<string, number>();
  private defNext = 0;
  /** Dirty ROW span of the def mirror (rows = def_index >> 8) — flushed as ONE texSubImage2D. */
  private defRowLo = DEF_H;
  private defRowHi = -1;
  /** P3 tight bbox per def (WORLD px, relative to the prim's top-left): `dx,dy` = offset to the
   *  opaque region, `w,h` = its size. Fraction-of-frame × prim size, so it's LOD-independent. */
  private readonly defTight = new Map<number, { dx: number; dy: number; w: number; h: number }>();
  /** The ONE atlas page the casters' surface frames live on (F2: single page) — bound as `uSurface`
   *  for the P4 silhouette sample. Adopted from the first resolved frame; a frame on any OTHER page
   *  gets no silhouette (solid quad) until multi-page lands (caster-lut C5). */
  private surfacePageTex: Texture | null = null;
  private pageWarned = false;
  private frameSizeWarned = false;

  /** `prim_data` — 2 entries/px: a placed caster's `position_anchor_reference` + `z`/`rotation`/`def_index`. */
  private readonly primTex: Texture;
  private readonly primMirror = new Uint32Array(PRIM_W * PRIM_H * 4);
  /** prim.id → allocated prim_data_index (≥1; index 0 is the sentinel). */
  private readonly primIndex = new Map<number, number>();
  private primNext = 1; // 0 reserved as the LUT sentinel
  /** Dirty ROW span of the prim mirror (rows = prim_index >> 9; 2 prims/px) — one upload per flush. */
  private primRowLo = PRIM_H;
  private primRowHi = -1;

  /** `light_data` (128×33): row 0 = record, rows 1–32 = the LUT. Rebuilt by {@link buildLights}. */
  private readonly lightTex: Texture;
  private readonly lightMirror = new Uint32Array(LIGHT_W * LIGHT_H * 4);
  private lightCount = 0;

  constructor(private readonly renderer: Renderer) {
    this.defTex = new Texture(renderer.gl, { width: DEF_W, height: DEF_H, format: "rgba32uint" });
    this.primTex = new Texture(renderer.gl, { width: PRIM_W, height: PRIM_H, format: "rgba32uint" });
    this.lightTex = new Texture(renderer.gl, { width: LIGHT_W, height: LIGHT_H, format: "rgba32uint" });
  }

  /** The `prim_definition_data` texture (bound as a `usampler2D` in the shadow shader). */
  get definitionTexture(): Texture {
    return this.defTex;
  }
  get definitionWidth(): number {
    return DEF_W;
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

    const base = idx * 4;
    const page = 0;        // ONE bound surface page today — C5 (texture-array pages) assigns real indices
    const ax = 1, ay = 2;  // shadow casters hang the bbox at the prim's BOTTOM-CENTER anchor
    const nax = 1, nay = 2; // nudge alignment: x centered, y bottom — the default nudging operation
    this.defMirror[base] = ((((wu >> 1) & 0x1ff) << 23) | (((hu >> 1) & 0x1ff) << 14) | (((st - 1) & 0xf) << 10)) >>> 0;
    this.defMirror[base + 1] = (((ux0 & 0x3ff) << 22) | ((uy0 & 0x3ff) << 12)) >>> 0;
    this.defMirror[base + 2] = (((fx16 & 0x3ff) << 22) | ((fy16 & 0x3ff) << 12) | ((page & 0xf) << 8) | ((lod & 0xf) << 4) | ((ax & 3) << 2) | (ay & 3)) >>> 0;
    this.defMirror[base + 3] = ((((nx + 2048) & 0xfff) << 20) | (((ny + 2048) & 0xfff) << 8) | ((nax & 3) << 6) | ((nay & 3) << 4)) >>> 0;
    const row = idx >> 8; // DEF_W = 256 px/row, 1 def/px
    if (row < this.defRowLo) this.defRowLo = row;
    if (row > this.defRowHi) this.defRowHi = row;
    return idx;
  }

  /** The tight bbox (WORLD px, relative to the prim's top-left) for a placed caster's def, or null. */
  tightBoxOf(defIndex: number): { dx: number; dy: number; w: number; h: number } | null {
    return this.defTight.get(defIndex) ?? null;
  }

  get primTexture(): Texture {
    return this.primTex;
  }
  get primWidth(): number {
    return PRIM_W;
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
      const base = (hit >> 1) * 4 + (hit & 1) * 2;
      if (this.primMirror[base + 1] === orient) return { idx: hit, changed: false };
      // RETENTION: "retain whatever the lod was until we get a NEW one" — a new one means a new
      // USABLE one. Never swap a standing prim DOWN to the loose (lod-0) def: if the freshly
      // resolved frame is unusable (off-page — e.g. a pool spill) the prim keeps casting its
      // current silhouette instead of degrading to a solid quad.
      const curDef = (this.primMirror[base + 1] >>> 6) & 0xffff;
      const newLod = (this.defMirror[defIndex * 4 + 2] >>> 4) & 0xf;
      const curLod = (this.defMirror[curDef * 4 + 2] >>> 4) & 0xf;
      if (newLod < 4 && curLod >= 4) return { idx: hit, changed: false };
      this.primMirror[base + 1] = orient; // def swap (new lod) / orientation change — position untouched
      this.markPrimRow(hit);
      return { idx: hit, changed: true };
    }
    const idx = this.primNext++;
    const ax = prim.x + prim.width * 0.5; // the TRUE game anchor (full-box base-centre) — unchanged
    const ay = prim.y + prim.height;
    // 2 entries/px: even index → RG, odd → BA.
    const base = (idx >> 1) * 4 + (idx & 1) * 2;
    this.primMirror[base] = encodePosition(ax, ay); // position_anchor_reference
    // orient: z(24–31) | rotation(22–23) | definition_index(6–21) | reserved(0–5)
    this.primMirror[base + 1] = orient;
    this.primIndex.set(prim.id, idx);
    this.markPrimRow(idx);
    return { idx, changed: true };
  }

  get lightTexture(): Texture {
    return this.lightTex;
  }
  get lights(): number {
    return this.lightCount;
  }
  /** Texture widths `[lightW, defW, primW]` for the shader's index→texel maths. */
  get widths(): [number, number, number] {
    return [LIGHT_W, DEF_W, PRIM_W];
  }

  /** Build `light_data` (the record row) from the lights, and upload it. **Records only** — casters
   *  are found by the per-tile buckets + corridor sweep now, so there's no LUT to build here (`standing`
   *  is bucketed by `ShadowGather.buildCasters`, which also allocates defs/prims). Call on change. */
  buildLights(lights: ColdLight[]): void {
    const n = Math.min(lights.length, N_LIGHTS);
    this.lightMirror.fill(0);
    for (let k = 0; k < n; k++) {
      const L = lights[k];
      const base = k * 4;
      this.lightMirror[base] = encodePosition(L.x, L.y); // R: position
      const r = clamp(L.color[0] * 255, 255), g = clamp(L.color[1] * 255, 255), b = clamp(L.color[2] * 255, 255);
      this.lightMirror[base + 1] = (((r << 24) | (g << 16) | (b << 8) | clamp(L.intensity * 255, 255)) >>> 0); // G
      // B: z(24–31, units) | reach(12–23, units) | reserved(1–11) | cast_shadows(0)
      const z = clamp(L.z / UNIT, 255), reach = clamp(L.reach / UNIT, 0xfff);
      this.lightMirror[base + 2] = (((z << 24) | (reach << 12) | (L.castShadows ? 1 : 0)) >>> 0);
      // A: emitter_radius(24–31, units) — penumbra softness; rest reserved.
      this.lightMirror[base + 3] = ((clamp(L.emitterRadius / UNIT, 255) << 24) >>> 0);
    }
    this.lightCount = n;
    this.lightTex.upload(this.lightMirror);
  }

  /** Upload any changed textures (call once per frame after populating). Cheap — no-op unless a slot landed. */
  /** Returns true if anything was uploaded (def/prim changed) — the caller re-dirties the shadow. */
  flush(): boolean {
    const changed = this.defRowHi >= this.defRowLo || this.primRowHi >= this.primRowLo;
    // ONE full-width row-span texSubImage2D per texture per flush, straight out of the mirror —
    // per-call overhead dominates at these sizes, so fewer/larger rectangles win. Appends
    // (immutable defs) and clustered prim swaps keep the bounding span naturally tight.
    if (this.defRowHi >= this.defRowLo) {
      this.defTex.uploadRows(this.defRowLo, this.defRowHi - this.defRowLo + 1, this.defMirror, 4);
      this.defRowLo = DEF_H; this.defRowHi = -1;
    }
    if (this.primRowHi >= this.primRowLo) {
      this.primTex.uploadRows(this.primRowLo, this.primRowHi - this.primRowLo + 1, this.primMirror, 4);
      this.primRowLo = PRIM_H; this.primRowHi = -1;
    }
    return changed;
  }

  /** Widen the prim mirror's dirty row span to cover `prim_index` (2 prims/px → row = idx >> 9). */
  private markPrimRow(idx: number): void {
    const row = idx >> 9;
    if (row < this.primRowLo) this.primRowLo = row;
    if (row > this.primRowHi) this.primRowHi = row;
  }

  // ── DEBUG decoders (verify against the CPU mirrors) ─────────────────────────────────
  debugDef(index: number): {
    prim_width: number; prim_height: number; frame_span: number; offset: [number, number];
    frame_xy: [number, number]; frame_page: number; frame_lod: number; anchor: [number, number];
    nudge: [number, number]; nudge_anchor: [number, number];
  } {
    const b = index * 4;
    const R = this.defMirror[b], G = this.defMirror[b + 1], B = this.defMirror[b + 2], A = this.defMirror[b + 3];
    return {
      prim_width: ((R >>> 23) & 0x1ff) * 2,  // units (stored /2 — even bbox)
      prim_height: ((R >>> 14) & 0x1ff) * 2,
      frame_span: ((R >>> 10) & 0xf) + 1,    // tiles
      offset: [(G >>> 22) & 0x3ff, (G >>> 12) & 0x3ff], // units, frame-relative (unsigned)
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
    const base = (index >> 1) * 4 + (index & 1) * 2;
    const pos = this.primMirror[base], orient = this.primMirror[base + 1];
    return { pos: decodePosition(pos), z: (orient >>> 24) & 0xff, rotation: (orient >>> 22) & 0x3, def: (orient >>> 6) & 0xffff };
  }

  get debugPrimCount(): number {
    return this.primNext - 1; // index 0 is the sentinel
  }
  /** DEBUG: decode a light record (the record row). */
  debugLight(k: number): { pos: [number, number]; rgb: [number, number, number]; intensity: number; z: number; reach: number; emitterRadius: number; castShadows: boolean } {
    const b = k * 4, R = this.lightMirror[b], G = this.lightMirror[b + 1], B = this.lightMirror[b + 2], A = this.lightMirror[b + 3];
    return {
      pos: decodePosition(R),
      rgb: [(G >>> 24) & 0xff, (G >>> 16) & 0xff, (G >>> 8) & 0xff],
      intensity: G & 0xff,
      z: (B >>> 24) & 0xff,
      reach: (B >>> 12) & 0xfff,
      emitterRadius: (A >>> 24) & 0xff,
      castShadows: (B & 1) === 1,
    };
  }

  destroy(): void {
    this.defTex.destroy();
    this.primTex.destroy();
    this.lightTex.destroy();
  }
}
