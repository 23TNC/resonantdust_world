//! The projected-silhouette SHADOW SCATTER (lighting P3) — ported from the old game's
//! `shadowMaskShader`. For each dynamic shadow-casting light, every nearby caster's earcut
//! silhouette (the `OutlineCache` triangulation) is projected through the light onto the ground (a
//! billboard shear: each vertex slides away from the light by `hUp/(lightZ−hUp)·dist`, growing with
//! its height up the sprite) and rasterized as flat filled triangles into ONE channel of an RGBA
//! scatter map. The display samples the map and subtracts `mask[light]` from that light's term —
//! per-light, so B lighting A's shadow can't un-shadow it.
//!
//! The per-light lane is a `uChannel` vec4 UNIFORM output directly — NOT the premultiplied tint
//! (tint couples rgb↔alpha, which is why the alpha lane was unusable + the cap was 3; `uChannel`
//! recovers the 4th → 4/map × SHADOW_MAPS). Casters for a light are one container rendered ONCE per
//! map with `max` blend (a second render into the same RT no-ops — the depth-saga trap).
//!
//! GOTCHA (as the bake shaders): a GLSL compile error draws BLACK with only a console.error; a
//! backtick inside a GLSL comment closes the template literal.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  Matrix,
  Geometry,
  Buffer,
  BufferUsage,
  UniformGroup,
} from "pixi.js";

/** RGBA scatter maps (4 lights each). Two → **8** fresh dynamic-light shadows/frame. */
export const SHADOW_MAPS = 2;
/** Dynamic lights that fit the scatter maps this frame (`4 × SHADOW_MAPS`). Past this, the warm
 *  round-robin cycles them (a cached, ≤3-frame-stale bit). */
export const MAX_SHADOW_LIGHTS = 4 * SHADOW_MAPS;

/** Which map (0..SHADOW_MAPS-1) light `i` writes. */
export const shadowMapOf = (i: number): number => i >> 2;
/** The channel-select vec4 for light `i`: a 1 in lane `i & 3` (R/G/B/A). Shader output directly. */
export function channelForLight(i: number): [number, number, number, number] {
  const c = i & 3;
  return [c === 0 ? 1 : 0, c === 1 ? 1 : 0, c === 2 ? 1 : 0, c === 3 ? 1 : 0];
}

/** Runaway guard on casters projected per light (nearest-this-many); the radius is the real bound. */
export const MAX_SHADOW_CASTERS = 256;
/** Cap on base→light distance in the projection length (px) — keeps shadow length stable as lights roam. */
export const SHADOW_DBL_CAP = 240;
/** Absolute cap on a projected shadow's length (px) — clamps a low light's `hUp/(lightZ−hUp)` blowup. */
export const SHADOW_MAX_LEN = 280;
/** Floor on the height a light PROJECTS from (px) — a low light casts the same short shadows a high one
 *  does (shadows only; the light's REAL height still drives N·L). */
export const SHADOW_MIN_Z = 90;
/** Initial projected-vertex capacity per light (3/tri). Doubles on demand. */
export const MAX_SHADOW_VERTS = 8192;

/** Occluder modelled height as a fraction of sprite px (base; taller casters diminish). */
export const SHADOW_OCC_HEIGHT_SCALE = 0.5;
export const SHADOW_HEIGHT_REF = 100;
export const SHADOW_HEIGHT_FALLOFF = 0.1;
export const SHADOW_HEIGHT_MIN = 0.15;
/** The occluder height scale for a caster of sprite height `h`: full ≤ ref, then `−FALLOFF`/doubling. */
export function shadowHeightScale(h: number): number {
  const f = SHADOW_OCC_HEIGHT_SCALE - SHADOW_HEIGHT_FALLOFF * Math.log2(Math.max(h, 1) / SHADOW_HEIGHT_REF);
  return Math.max(SHADOW_HEIGHT_MIN, Math.min(SHADOW_OCC_HEIGHT_SCALE, f));
}
/** Stretch north-going (screen −Y) shadow length so it reads as long as southward (ground recede). */
export const SHADOW_NORTH_STRETCH = 1.8;

// Solid-fill shadow: overwrite outColor with the per-light lane mask. `max` blend accumulates coverage.
const shadowMaskBitGl = {
  name: "shadow-mask-bit",
  fragment: {
    header: /* glsl */ `uniform vec4 uChannel;`,
    main: /* glsl */ `outColor = uChannel;`,
  },
};

let program: GlProgram | null = null;
function shadowProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "shadow-mask",
      bits: [localUniformBitGl, textureBitGl, shadowMaskBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** Flat-fill shadow shader: outputs `uChannel` (the per-light lane). The silhouette is the geometry. */
export class ShadowMaskShader extends Shader {
  /** Point this mesh's coverage at one lane (a 1 in r/g/b/a). */
  setChannel(rgba: readonly [number, number, number, number]): void {
    const u = this.resources.channelUniforms.uniforms;
    (u.uChannel as Float32Array).set(rgba);
    this.resources.channelUniforms.update();
  }
}

export function makeShadowMaskShader(): ShadowMaskShader {
  const white = Texture.WHITE;
  return new ShadowMaskShader({
    glProgram: shadowProgram(),
    resources: {
      uTexture: white.source,
      uSampler: white.source.style,
      channelUniforms: new UniformGroup({ uChannel: { value: new Float32Array([1, 0, 0, 0]), type: "vec4<f32>" } }),
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
    },
  });
}

/** Geometry for one light's projected-silhouette triangles (`cap` verts). Non-indexed tri list drawn
 *  via a sequential index buffer (fixed draw count); the zeroed tail is degenerate → draws nothing. */
export function makeShadowGeometry(cap: number = MAX_SHADOW_VERTS): { geometry: Geometry; pos: Buffer } {
  const pos = new Buffer({ data: new Float32Array(cap * 2), usage: BufferUsage.VERTEX | BufferUsage.COPY_DST });
  const uv = new Buffer({ data: new Float32Array(cap * 2), usage: BufferUsage.VERTEX });
  const indices = new Uint32Array(cap);
  for (let i = 0; i < cap; i++) indices[i] = i;
  const geometry = new Geometry({
    attributes: {
      aPosition: { buffer: pos, format: "float32x2" },
      aUV: { buffer: uv, format: "float32x2" },
    },
    indexBuffer: new Buffer({ data: indices, usage: BufferUsage.INDEX | BufferUsage.COPY_DST }),
  });
  return { geometry, pos };
}
