//! ColdShadowData — the GPU data textures the shadow cast reads via `texelFetch` instead of per-frame
//! instance attributes (the `cold-data-textures` stream). Four normalized `RGBA32UI` textures with a
//! light → LUT → definition / instance indirection; written on change, read every frame for free. Bit
//! layouts are authoritative in `docs/VARIABLES.md` §Cold shadow data textures.
//!
//! World unit: `1 unit = SQUARE/16 = 4px`, a compile-time constant (§Unit). Every world field is in units;
//! only the atlas `frame_*` are texture px. Positions pack as `position_anchor_reference` (region | zone |
//! tile | anchor, each `u8 = x:4|y:4`).
//!
//! This slice implements `prim_definition_data` (P1): one px per sprite variant (generic geometry + atlas
//! frame + page), written on first sight of a caster sprite from the resolver's surface frame. The other
//! three textures (cold_prim_data, cold_light_data, cold_light_prim_data) + the shader read land in P2–P4.

import { Renderer, Texture } from "../../gl";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";

/** `1 unit = SQUARE/16 = 4px` — the compile-time world unit everything (bar atlas px) is measured in. */
export const UNIT = SQUARE / 16;

/** `prim_definition_data` texture size — 1 px/variant; `128 × 64` = 8192 slots (far more than the sprite
 *  variants in play). Index `i` → texel `(i & 127, i >> 7)`. */
const DEF_W = 128;
const DEF_H = 64;
/** `cold_prim_data` texture size — 2 entries/px; `256 × 128` px → 65536 caster slots. */
const PRIM_W = 256;
const PRIM_H = 128;
/** `cold_light_data` — 1 px/light. `cold_light_prim_data` (LUT) — 4 entries/px. */
const LIGHT_W = 64;
const LUT_W = 256;
const LUT_H = 256;

const u10 = (v: number): number => Math.min(Math.max(Math.round(v), 0), 0x3ff);
const clamp = (v: number, hi: number): number => Math.min(Math.max(Math.round(v), 0), hi);

/** One light to write into `cold_light_data` (world px + unit-scaled reach; colour 0..1). */
export interface ColdLight {
  x: number;
  y: number;
  z: number; // world px (→ units)
  radius: number; // world px (→ units)
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

  /** `cold_prim_data` — 2 entries/px: a placed caster's `position_anchor_reference` + `z`/`rotation`. */
  private readonly primTex: Texture;
  private readonly primMirror = new Uint32Array(PRIM_W * PRIM_H * 4);
  /** prim.id → allocated prim_data_index. */
  private readonly primIndex = new Map<number, number>();
  private primNext = 0;
  private primDirty = false;

  /** `cold_light_data` (1 px/light) + `cold_light_prim_data` (the LUT, 4 entries/px). Rebuilt by
   *  {@link buildLights}; UNIT for cold data is once-per-change, not per frame (P4). */
  private readonly lightTex: Texture;
  private readonly lightMirror = new Uint32Array(LIGHT_W * 4);
  private readonly lutTex: Texture;
  private readonly lutMirror = new Uint32Array(LUT_W * LUT_H * 4);
  private lightCount = 0;

  constructor(private readonly renderer: Renderer) {
    this.defTex = new Texture(renderer.gl, { width: DEF_W, height: DEF_H, format: "rgba32uint" });
    this.primTex = new Texture(renderer.gl, { width: PRIM_W, height: PRIM_H, format: "rgba32uint" });
    this.lightTex = new Texture(renderer.gl, { width: LIGHT_W, height: 1, format: "rgba32uint" });
    this.lutTex = new Texture(renderer.gl, { width: LUT_W, height: LUT_H, format: "rgba32uint" });
  }

  /** The `prim_definition_data` texture (bound as a `usampler2D` in the shadow shader). */
  get definitionTexture(): Texture {
    return this.defTex;
  }
  get definitionWidth(): number {
    return DEF_W;
  }

  /** The `definition_index` for a caster's sprite, allocating + writing its def on first sight from the
   *  resolver's surface frame (billboard `width/height` in units, atlas `frame x/y/w/h` in px, `frame_page`).
   *  Returns -1 until the sprite's surface LOD has resolved (the caller falls back). Generic per variant —
   *  every instance of the sprite shares the slot. */
  definitionFor(prim: Primitive, resolver: TextureResolver | null): number {
    if (!prim.textureName || !resolver) return -1;
    const key = `${prim.textureName}|${prim.cell ?? 0}`;
    const hit = this.defIndex.get(key);
    if (hit !== undefined) return hit;
    const surf = resolver.resolve(prim.textureName, "surface", prim.cell);
    if (surf.geo || !surf.frame) return -1; // not loaded yet
    const f = surf.frame;

    const idx = this.defNext++;
    const base = idx * 4;
    const pw = u10(prim.width / UNIT); // billboard width  (units)
    const ph = u10(prim.height / UNIT); // billboard height (units)
    const framePage = 0; // single atlas page for now (F2); the layout carries frame_page for later
    // R: prim_width(22–31) | prim_height(12–21) | frame_x(2–11) | rsvd(0–1)
    this.defMirror[base] = (((u10(pw) << 22) | (u10(ph) << 12) | (u10(f.x) << 2)) >>> 0);
    // G: frame_width(22–31) | frame_height(12–21) | frame_y(2–11) | rsvd(0–1)
    this.defMirror[base + 1] = (((u10(f.w) << 22) | (u10(f.h) << 12) | (u10(f.y) << 2)) >>> 0);
    // B: frame_page(22–31) | rsvd(0–21)
    this.defMirror[base + 2] = ((u10(framePage) << 22) >>> 0);
    this.defMirror[base + 3] = 0; // A: reserved (materials later)
    this.defIndex.set(key, idx);
    this.defDirty = true;
    return idx;
  }

  get primTexture(): Texture {
    return this.primTex;
  }
  get primWidth(): number {
    return PRIM_W;
  }

  /** The `prim_data_index` for a PLACED caster (its position + orientation), allocated on first sight + cached
   *  by `prim.id`. Cold things are static, so it's written once. `rotation` from `flipX` (E=1 / W=3 — both the
   *  E/W regime) as a placeholder until the prim carries a real facing (F1/P5); `z` = 0 (ground). */
  primDataFor(prim: Primitive): number {
    const hit = this.primIndex.get(prim.id);
    if (hit !== undefined) return hit;
    const idx = this.primNext++;
    const ax = prim.x + prim.width * 0.5; // ground anchor (bottom-centre)
    const ay = prim.y + prim.height;
    const rotation = prim.flipX ? 3 : 1; // W : E (both E/W regime for now)
    const z = 0;
    // 2 entries/px: even index → RG, odd → BA.
    const base = (idx >> 1) * 4 + (idx & 1) * 2;
    this.primMirror[base] = encodePosition(ax, ay); // position_anchor_reference
    // orient: z(24–31) | rotation(22–23) | reserved(0–21)
    this.primMirror[base + 1] = ((((z & 0xff) << 24) | ((rotation & 0x3) << 22)) >>> 0);
    this.primIndex.set(prim.id, idx);
    this.primDirty = true;
    return idx;
  }

  get lightTexture(): Texture {
    return this.lightTex;
  }
  get lutTexture(): Texture {
    return this.lutTex;
  }
  get lutWidth(): number {
    return LUT_W;
  }
  get lights(): number {
    return this.lightCount;
  }

  /** Build `cold_light_data` + the `cold_light_prim_data` LUT from the lights + resident casters, and upload
   *  both. Each `cast_shadows` light gets a contiguous LUT run of `(definition_index, prim_data_index)` for its
   *  in-range casters (Chebyshev cull; def skipped until its surface resolves). Cold data → call on change, not
   *  per frame (P4). */
  buildLights(lights: ColdLight[], standing: Primitive[], resolver: TextureResolver | null): void {
    const n = Math.min(lights.length, LIGHT_W);
    let cursor = 0;
    for (let k = 0; k < n; k++) {
      const L = lights[k];
      const lutIndex = cursor;
      if (L.castShadows) {
        const rPx = L.radius;
        for (const p of standing) {
          const ax = p.x + p.width * 0.5, ay = p.y + p.height;
          if (Math.abs(ax - L.x) > rPx || Math.abs(ay - L.y) > rPx) continue; // Chebyshev cull (F5)
          const def = this.definitionFor(p, resolver);
          if (def < 0) continue; // surface not resolved yet
          const inst = this.primDataFor(p);
          if (cursor >= LUT_W * LUT_H * 4) break;
          this.lutMirror[cursor++] = (((def & 0xffff) << 16) | (inst & 0xffff)) >>> 0;
        }
      }
      const lutCount = cursor - lutIndex;
      const base = k * 4;
      this.lightMirror[base] = encodePosition(L.x, L.y); // R: position
      // G: r|g|b|intensity (u8 each)
      const r = clamp(L.color[0] * 255, 255), g = clamp(L.color[1] * 255, 255), b = clamp(L.color[2] * 255, 255);
      this.lightMirror[base + 1] = (((r << 24) | (g << 16) | (b << 8) | clamp(L.intensity * 255, 255)) >>> 0);
      // B: z(24–31) | radius(12–23) | reserved(1–11) | cast_shadows(0)
      const z = clamp(L.z / UNIT, 255), radius = clamp(L.radius / UNIT, 0xfff);
      this.lightMirror[base + 2] = (((z << 24) | (radius << 12) | (L.castShadows ? 1 : 0)) >>> 0);
      // A: lut_index(16–31) | lut_count(0–15)
      this.lightMirror[base + 3] = ((((lutIndex & 0xffff) << 16) | (lutCount & 0xffff)) >>> 0);
    }
    this.lightCount = n;
    this.lightTex.upload(this.lightMirror);
    this.lutTex.upload(this.lutMirror);
    this.flush(); // ensure any new defs/prims from the cull are uploaded too
  }

  /** Upload any changed textures (call once per frame after populating). Cheap — no-op unless a slot landed. */
  flush(): void {
    if (this.defDirty) {
      this.defTex.upload(this.defMirror);
      this.defDirty = false;
    }
    if (this.primDirty) {
      this.primTex.upload(this.primMirror);
      this.primDirty = false;
    }
  }

  /** DEBUG (P1 verify): decode a definition slot from the CPU mirror. */
  debugDef(index: number): { prim_width: number; prim_height: number; frame: [number, number, number, number]; frame_page: number } {
    const b = index * 4;
    const R = this.defMirror[b], G = this.defMirror[b + 1], B = this.defMirror[b + 2];
    return {
      prim_width: (R >>> 22) & 0x3ff,
      prim_height: (R >>> 12) & 0x3ff,
      frame: [(R >>> 2) & 0x3ff, (G >>> 2) & 0x3ff, (G >>> 22) & 0x3ff, (G >>> 12) & 0x3ff], // x,y,w,h
      frame_page: (B >>> 22) & 0x3ff,
    };
  }
  get debugDefCount(): number {
    return this.defNext;
  }
  /** DEBUG (P2 verify): decode a cold_prim_data slot — position back to px + z + rotation. */
  debugPrim(index: number): { pos: [number, number]; z: number; rotation: number } {
    const base = (index >> 1) * 4 + (index & 1) * 2;
    const pos = this.primMirror[base], orient = this.primMirror[base + 1];
    return { pos: decodePosition(pos), z: (orient >>> 24) & 0xff, rotation: (orient >>> 22) & 0x3 };
  }
  get debugPrimCount(): number {
    return this.primNext;
  }
  /** DEBUG (P3 verify): decode a cold_light_data slot. */
  debugLight(k: number): { pos: [number, number]; rgb: [number, number, number]; intensity: number; z: number; radius: number; castShadows: boolean; lutIndex: number; lutCount: number } {
    const b = k * 4, R = this.lightMirror[b], G = this.lightMirror[b + 1], B = this.lightMirror[b + 2], A = this.lightMirror[b + 3];
    return {
      pos: decodePosition(R),
      rgb: [(G >>> 24) & 0xff, (G >>> 16) & 0xff, (G >>> 8) & 0xff],
      intensity: G & 0xff,
      z: (B >>> 24) & 0xff,
      radius: (B >>> 12) & 0xfff,
      castShadows: (B & 1) === 1,
      lutIndex: (A >>> 16) & 0xffff,
      lutCount: A & 0xffff,
    };
  }
  /** DEBUG (P3 verify): decode a LUT entry → (definition_index, prim_data_index). */
  debugLut(i: number): [number, number] {
    const e = this.lutMirror[i];
    return [(e >>> 16) & 0xffff, e & 0xffff];
  }

  destroy(): void {
    this.defTex.destroy();
    this.primTex.destroy();
    this.lightTex.destroy();
    this.lutTex.destroy();
  }
}
