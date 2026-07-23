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
//!   • `prim_definition_data` 256×256 — one px/variant: opaque bbox (units) + anchor offset + the P4
//!                            silhouette frame (opaque surface sub-rect, atlas px), keyed by def_index.
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

const u10 = (v: number): number => Math.min(Math.max(Math.round(v), 0), 0x3ff);
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
  private defDirty = false;
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
  private primDirty = false;

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

  /** The `definition_index` for a caster's sprite, allocating on first sight and (re)writing the def
   *  whenever its inputs change (compare-write — a LOD upgrade moves the frame and lands here):
   *
   *  The shadow quad = the sprite's OPAQUE bbox. Its size (W/H, units) + its OFFSET from the prim's game
   *  anchor live in the DEF (per-sprite, shared); `prim_data` keeps the true game anchor untouched, and
   *  the gather shifts it by this offset. The bbox is pre-computed on the CPU at decode (frame fractions,
   *  LOD-safe). P4: the def also carries the **silhouette frame** — the opaque sub-rect of the sprite's
   *  SURFACE frame in atlas px on the shared page (F2) — sampled inside the proven quad; `frame_w = 0`
   *  (not yet resolved / other page) → solid quad. No shadow is gated on any of it: a LOOSE fallback
   *  (full box, zero offset, no frame) casts until the surface decodes, then the def UPGRADES in place —
   *  which re-dirties the gather, no per-prim rewrite needed. */
  definitionFor(prim: Primitive, resolver: TextureResolver | null): number {
    if (!prim.textureName) return -1;
    const key = `${prim.textureName}|${prim.cell ?? 0}`;
    let idx = this.defIndex.get(key);
    if (idx === undefined) { idx = this.defNext++; this.defIndex.set(key, idx); }

    const bbox = resolver ? resolver.opaqueBBox(prim.textureName) : null;
    const dx = bbox ? bbox.fx * prim.width : 0, dy = bbox ? bbox.fy * prim.height : 0;      // opaque top-left (px)
    const ww = bbox ? bbox.fw * prim.width : prim.width, hh = bbox ? bbox.fh * prim.height : prim.height;
    this.defTight.set(idx, { dx, dy, w: ww, h: hh }); // opaque box (px, rel. prim top-left) — for bucketing
    // Offset (units) from the game anchor (full-box base-centre) to the opaque bbox base-centre.
    const ox = Math.round((dx + ww / 2 - prim.width / 2) / UNIT), oy = Math.round((dy + hh - prim.height) / UNIT);

    // P4 silhouette frame: the opaque sub-rect of the surface frame (atlas px) on the shared page.
    // frame_w/h are u10 — REQUIREMENT: no prim texture exceeds 1024×1024 (at SQUARE=64 that is
    // 16×16 tiles = one full zone). A larger frame is a content bug: warn once + clamp.
    let fx = 0, fy = 0, fw = 0, fh = 0;
    const surf = resolver && bbox ? resolver.resolve(prim.textureName, "surface", prim.cell).frame : null;
    if (surf) {
      if (!this.surfacePageTex) this.surfacePageTex = surf.source;
      if (surf.source === this.surfacePageTex) {
        fx = Math.round(surf.x + bbox!.fx * surf.w);
        fy = Math.round(surf.y + bbox!.fy * surf.h);
        fw = Math.max(1, Math.round(bbox!.fw * surf.w));
        fh = Math.max(1, Math.round(bbox!.fh * surf.h));
        if ((fw > 0x3ff || fh > 0x3ff) && !this.frameSizeWarned) {
          this.frameSizeWarned = true;
          console.warn(`[cold-shadow] ${prim.textureName}: silhouette frame ${fw}×${fh} exceeds the 1024² per-prim texture ceiling — clamped (u10)`);
        }
      } else if (!this.pageWarned) {
        this.pageWarned = true;
        console.warn("[cold-shadow] surface frame off the shared page — silhouette skipped (solid quad, C5)");
      }
    }

    const base = idx * 4;
    // R: W(22–31) | H(12–21) opaque size (units). G: (ox+512)(12–21) | (oy+512)(2–11) signed offset (units).
    // B: frame_x(18–31) | frame_y(4–17) (atlas px — PAGE coords, u14 = the 16384 GL max texture size).
    // A: frame_w(22–31) | frame_h(12–21) (u10 — ≤1024² per-prim ceiling; w = 0 → solid quad)
    //    | frame_page(0–7) (u8 — ≤256 pages, the WebGL2 MAX_ARRAY_TEXTURE_LAYERS floor; 0 until C5).
    const page = 0; // ONE bound surface page today — C5 (texture-array pages) assigns real indices
    const R = (((u10(ww / UNIT) << 22) | (u10(hh / UNIT) << 12)) >>> 0);
    const G = (((((ox + 512) & 0x3ff) << 12) | (((oy + 512) & 0x3ff) << 2)) >>> 0);
    const B = ((((fx & 0x3fff) << 18) | ((fy & 0x3fff) << 4)) >>> 0);
    const A = (((u10(fw) << 22) | (u10(fh) << 12) | (page & 0xff)) >>> 0);
    const m = this.defMirror;
    if (m[base] !== R || m[base + 1] !== G || m[base + 2] !== B || m[base + 3] !== A) {
      m[base] = R; m[base + 1] = G; m[base + 2] = B; m[base + 3] = A;
      this.defDirty = true;
    }
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

  /** The `prim_data_index` (≥1) for a PLACED caster (position + orientation + its `def_index`), allocated on
   *  first sight + cached by `prim.id`. Cold things are static, so it's written once. `rotation` from `flipX`
   *  (E=1 / W=3 — both the E/W regime) as a placeholder until the prim carries a real facing; `z` = 0. */
  primDataFor(prim: Primitive, defIndex: number): number {
    const hit = this.primIndex.get(prim.id);
    if (hit !== undefined) return hit; // static caster → written once; the opaque offset lives in the def
    const idx = this.primNext++;
    const ax = prim.x + prim.width * 0.5; // the TRUE game anchor (full-box base-centre) — unchanged
    const ay = prim.y + prim.height;
    const rotation = prim.flipX ? 3 : 1; // W : E (both E/W regime for now)
    const z = 0;
    // 2 entries/px: even index → RG, odd → BA.
    const base = (idx >> 1) * 4 + (idx & 1) * 2;
    this.primMirror[base] = encodePosition(ax, ay); // position_anchor_reference
    // orient: z(24–31) | rotation(22–23) | definition_index(6–21) | reserved(0–5)
    this.primMirror[base + 1] = ((((z & 0xff) << 24) | ((rotation & 0x3) << 22) | ((defIndex & 0xffff) << 6)) >>> 0);
    this.primIndex.set(prim.id, idx);
    this.primDirty = true;
    return idx;
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
    const changed = this.defDirty || this.primDirty;
    if (this.defDirty) {
      this.defTex.upload(this.defMirror);
      this.defDirty = false;
    }
    if (this.primDirty) {
      this.primTex.upload(this.primMirror);
      this.primDirty = false;
    }
    return changed;
  }

  // ── DEBUG decoders (verify against the CPU mirrors) ─────────────────────────────────
  debugDef(index: number): { prim_width: number; prim_height: number; offset: [number, number]; frame: [number, number, number, number]; frame_page: number } {
    const b = index * 4;
    const R = this.defMirror[b], G = this.defMirror[b + 1], B = this.defMirror[b + 2], A = this.defMirror[b + 3];
    return {
      prim_width: (R >>> 22) & 0x3ff,
      prim_height: (R >>> 12) & 0x3ff,
      offset: [((G >>> 12) & 0x3ff) - 512, ((G >>> 2) & 0x3ff) - 512], // units, opaque-base-centre − anchor
      frame: [(B >>> 18) & 0x3fff, (B >>> 4) & 0x3fff, (A >>> 22) & 0x3ff, (A >>> 12) & 0x3ff], // x,y (u14) w,h (u10, atlas px)
      frame_page: A & 0xff,
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
