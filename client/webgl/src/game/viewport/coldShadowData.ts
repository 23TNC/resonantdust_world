//! ColdShadowData — the GPU data textures the shadow gather reads via `texelFetch` (the
//! `2026-07-21-shadow-bitfield` stream). Fixed-size, normalized `RGBA32UI` textures; written on change
//! (CPU-side; no ping-pong), read every frame for free. Bit layouts are authoritative in
//! `docs/VARIABLES.md` §Cold shadow data textures.
//!
//! Layout (fixed slots — `N=128` lights, `≤256` casters/light, `u16` indexes):
//!   • `light_data`           128×33 — col = light; row 0 = record, rows 1–32 = 256× `u16` prim indexes
//!                            (the LUT folded in; `lut_index` implicit = column; sentinel `0`-terminated).
//!   • `prim_data`            256×128 — all placed casters, 2/px (`u64`); carries `u16 definition_index`.
//!                            Index 0 is the SENTINEL (empty/end) → usable 1..65535.
//!   • `prim_definition_data` 256×256 — one px/variant (generic geometry + atlas frame), keyed by def_index.
//!
//! World unit: `1 unit = SQUARE/16 = 4px`, compile-time (§Unit). Every world field is in units; only the
//! atlas `frame_*` are texture px. Positions pack as `position_anchor_reference` (region|zone|tile|anchor).

import { Renderer, Texture, type TexFrame } from "../../gl";
import type { Primitive } from "./SquareCache";
import type { TextureResolver } from "../../textures";
import { SQUARE, ZONE_DIM, REGION_DIM } from "./squareMath";

/** `1 unit = SQUARE/16 = 4px` — the compile-time world unit everything (bar atlas px) is measured in. */
export const UNIT = SQUARE / 16;

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
const u8 = (v: number): number => Math.min(Math.max(Math.round(v), 0), 0xff);
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
  /** The atlas page the casters' surface frames live on (F2: single page) — bound for the alpha mask. */
  private surfacePageTex: Texture | null = null;

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

  /** The `definition_index` for a caster's sprite, allocating + writing its def on first sight from the
   *  resolver's surface frame (billboard `width/height` in units, atlas `frame x/y/w/h` in px, `frame_page`).
   *  Returns -1 until the sprite's surface LOD has resolved (the caller falls back). Generic per variant —
   *  every instance of the sprite shares the slot. */
  definitionFor(prim: Primitive, _resolver: TextureResolver | null): number {
    if (!prim.textureName) return -1;
    const key = `${prim.textureName}|${prim.cell ?? 0}`;
    const hit = this.defIndex.get(key);
    if (hit !== undefined) return hit;
    // PURE QUAD: the shadow is a projected billboard quad — it only needs the billboard W/H (units).
    // We deliberately do NOT gate on the surface LOD resolving. That gate made shadows ZOOM-DEPENDENT:
    // a tree whose texture LOD hadn't loaded at the current zoom got def=-1 and cast no shadow. W/H come
    // from the prim geometry, not the atlas, so they're always available. (frame/base-pad were only for
    // the silhouette sample, which the pure-quad gather no longer does.)
    const idx = this.defNext++;
    const base = idx * 4;
    const pw = u10(prim.width / UNIT); // billboard width  (units)
    const ph = u10(prim.height / UNIT); // billboard height (units)
    // R: prim_width(22–31) | prim_height(12–21) | (frame/pad fields unused in pure-quad mode → 0)
    this.defMirror[base] = (((pw << 22) | (ph << 12)) >>> 0);
    this.defMirror[base + 1] = 0;
    this.defMirror[base + 2] = 0;
    this.defMirror[base + 3] = 0;
    this.defIndex.set(key, idx);
    this.defDirty = true;
    return idx;
  }

  /** From the sprite silhouette (one surface-frame B readback, cached with the def): the base spread
   *  `[dA, dB]` (units — the fan's, kept for a fan-return) and **`basePad`** = the transparent rows below
   *  the bottom-most opaque pixel (**atlas px**), used to lift the shadow base to the opaque base. */
  private depthUnits(f: TexFrame, hPx: number): [number, number, number] {
    const gl = this.renderer.gl;
    const w = f.w, h = f.h;
    const fb = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fb);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, f.source.handle, 0);
    const px = new Uint8Array(w * h * 4);
    gl.readPixels(f.x, f.y, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.deleteFramebuffer(fb);
    const present = (x: number, y: number): boolean => px[(y * w + x) * 4 + 2] > 127; // B = coverage
    const avgHeight = (x0: number, x1: number): number => {
      let sum = 0, cols = 0;
      for (let x = x0; x < x1; x++) {
        let top = -1, bot = -1;
        for (let y = 0; y < h; y++) if (present(x, y)) { top = y; break; }
        for (let y = h - 1; y >= 0; y--) if (present(x, y)) { bot = y; break; }
        if (top >= 0) { sum += bot - top + 1; cols++; }
      }
      return cols ? sum / cols : 0;
    };
    const mid = w >> 1;
    // Transparent base padding: rows below the bottom-most opaque pixel (atlas px).
    let bottomOpaque = -1;
    for (let y = h - 1; y >= 0 && bottomOpaque < 0; y--)
      for (let x = 0; x < w; x++) if (present(x, y)) { bottomOpaque = y; break; }
    const basePad = bottomOpaque < 0 ? 0 : h - 1 - bottomOpaque;
    return [(0.5 * (avgHeight(0, mid) / h) * hPx) / UNIT, (0.5 * (avgHeight(mid, w) / h) * hPx) / UNIT, basePad];
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
    if (hit !== undefined) return hit;
    const idx = this.primNext++;
    const ax = prim.x + prim.width * 0.5; // ground anchor (bottom-centre)
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

  // ── DEBUG decoders (verify against the CPU mirrors) ─────────────────────────────────
  debugDef(index: number): { prim_width: number; prim_height: number; frame: [number, number, number, number]; frame_page: number; basePad: number } {
    const b = index * 4;
    const R = this.defMirror[b], G = this.defMirror[b + 1], B = this.defMirror[b + 2];
    return {
      prim_width: (R >>> 22) & 0x3ff,
      prim_height: (R >>> 12) & 0x3ff,
      frame: [(R >>> 2) & 0x3ff, (G >>> 2) & 0x3ff, (G >>> 22) & 0x3ff, (G >>> 12) & 0x3ff], // x,y,w,h
      frame_page: (B >>> 22) & 0x3ff,
      basePad: (B >>> 14) & 0xff,
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
