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
import { SQUARE } from "./squareMath";

/** `1 unit = SQUARE/16 = 4px` — the compile-time world unit everything (bar atlas px) is measured in. */
export const UNIT = SQUARE / 16;

/** `prim_definition_data` texture size — 1 px/variant; `128 × 64` = 8192 slots (far more than the sprite
 *  variants in play). Index `i` → texel `(i & 127, i >> 7)`. */
const DEF_W = 128;
const DEF_H = 64;

const u10 = (v: number): number => Math.min(Math.max(Math.round(v), 0), 0x3ff);

export class ColdShadowData {
  /** `prim_definition_data` — one px/variant: geometry (units) + atlas frame (px) + page. */
  private readonly defTex: Texture;
  private readonly defMirror = new Uint32Array(DEF_W * DEF_H * 4);
  /** stem+cell → allocated definition_index. */
  private readonly defIndex = new Map<string, number>();
  private defNext = 0;
  private defDirty = false;

  constructor(private readonly renderer: Renderer) {
    this.defTex = new Texture(renderer.gl, { width: DEF_W, height: DEF_H, format: "rgba32uint" });
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

  /** Upload any changed textures (call once per frame after populating). Cheap — no-op unless a def landed. */
  flush(): void {
    if (this.defDirty) {
      this.defTex.upload(this.defMirror);
      this.defDirty = false;
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

  destroy(): void {
    this.defTex.destroy();
  }
}
