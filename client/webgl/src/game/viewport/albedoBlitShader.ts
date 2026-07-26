//! The viewport's display shader (webgl port) — draws the baked ALBEDO G-buffer lit by the baked LIGHTMAP,
//! warm (movers) composited over cold by warm coverage:
//!
//!   out = albedo.rgb × (ambient + coldLight + hotLight) × alpha       (alpha = surface.B, the visual coverage)
//!
//! lightmap P1: the lightmap is a FINE (TEXTILE_SQUARE/tile) single map holding the fully accumulated per-light
//! irradiance — Σ colour·falloff·(1−shadow)·max(0, N·L) — baked in {@link ShadowGather} (the N·L is done per
//! light in the bake against the prim's world-frame normal, so there is no blit-side relief/direction/unshadowed
//! path anymore, and no bilinear smear: the detail lives in the lighting, sampled NEAREST). GOTCHA: a backtick
//! inside the GLSL closes the `/* glsl */` literal.

import { Program, type Texture } from "../../gl";
import { SQUARE } from "./squareMath";

const SQF = SQUARE.toFixed(1);

const BLIT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // display-quad world px
in vec2 aUV;                    // composite UV (identity: the bake stores upright)
uniform mat3 uProjection;       // world px → clip (pan + zoom + screen→clip)
out vec2 vUV;
out vec2 vWorld;                // world px — for sampling the lightmap by world position
void main() {
  vUV = aUV;
  vWorld = aPosition;
  vec3 p = uProjection * vec3(aPosition, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const BLIT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
in vec2 vUV;
in vec2 vWorld;
uniform sampler2D uAlbedo;       // COLD albedo composite (the mesh's main texture)
uniform sampler2D uSurface;      // COLD surface: B = alpha (visual coverage)
uniform sampler2D uAlbedoWarm;   // WARM tier: mover albedo (over cold by warm coverage)
uniform sampler2D uSurfaceWarm;  // WARM tier: mover surface (its B = composite coverage)
uniform float uLightQuant;      // additive-lightmap quantisation step (F11b) — de-quantise on read
uniform sampler2D uColdLight;    // #4 COLD lightmap irradiance (static lights, baked once) — per-light N·L baked in
uniform sampler2D uHotLight;     // #4 HOT lightmap irradiance (dynamic lights, per frame)
uniform int uLightEnable;        // 0 = UNLIT (albedo only) — the fallback when the lightmap isn't ready
uniform float uAmbient;          // #4 ambient floor — added ONCE over cold+hot (not baked into either map)
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot; // lightmap window mapping (uLSlot = TEXTILE_SQUARE, fine)
out vec4 fragColor;
const float SQ = ${SQF};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// World → the toroidal FINE lightmap texel (matches the bake's fc→world), or (-1,-1) if outside the window.
ivec2 lightTexel(vec2 world) {
  int tx = int(floor(world.x / SQ)), ty = int(floor(world.y / SQ));
  if (tx < uLWinCol || tx >= uLWinCol + uLCols || ty < uLWinRow || ty >= uLWinRow + uLRows) return ivec2(-1);
  int sx = pmod(tx, uLCols), sy = pmod(ty, uLRows);
  float lx = fract(world.x / SQ), ly = fract(world.y / SQ);
  return ivec2(sx * uLSlot + int(lx * float(uLSlot)), sy * uLSlot + int(ly * float(uLSlot)));
}
void main() {
  vec4 outColor = texture(uAlbedo, vUV);
  // WARM-over-COLD: warm is slot-aligned with cold, sampled at the SAME vUV. Where no mover sits, warm
  // coverage is 0 -> pure cold.
  float wcov = texture(uSurfaceWarm, vUV).b;
  vec4 alb = mix(outColor, texture(uAlbedoWarm, vUV), wcov);
  vec4 surf = mix(texture(uSurface, vUV), texture(uSurfaceWarm, vUV), wcov);
  float alpha = surf.b;          // visual coverage -> output alpha
  // LIGHTING: material (albedo) × illumination (lightmap). Unlit fallback keeps the old blit exactly.
  vec3 light = vec3(1.0);
  if (uLightEnable == 1) {
    ivec2 lt = lightTexel(vWorld);
    if (lt.x < 0) { light = vec3(1.0); }        // outside window — shouldn't happen (resident squares only)
    else {
      // lightmap P1: the FINE lightmap already holds Σ per-light colour·falloff·(1−shadow)·N·L (cold + hot).
      // Sampled NEAREST — the detail lives in the lighting at TEXTILE_SQUARE/tile, so NO bilinear smear (that
      // was the coarse-map blob fix). Ambient is directionless, added once here over cold+hot.
      // The lightmap is an RGBA32F ADDITIVE ACCUMULATOR holding QUANTISED integer deposits (F11b), so
      // de-quantise here — this is the one place the scale is undone. Values may exceed 1 legitimately
      // (many lights on one texel), hence the clamp AFTER the divide rather than an LDR clamp at bake.
      vec3 irr = (texelFetch(uColdLight, lt, 0).rgb + texelFetch(uHotLight, lt, 0).rgb) / uLightQuant;
      light = uAmbient + min(irr, vec3(4.0));
    }
  }
  // Coverage applied at OUTPUT only (premultiplied) so it composites over the canvas background: empty cells
  // (alpha 0) show through, ground/things (alpha 1) draw opaque.
  fragColor = vec4(alb.rgb * light * alpha, alpha);
}
`;

/** The display albedo×lightmap material. Bind the COLD albedo + surface composites, the WARM albedo/surface,
 *  and the baked COLD/HOT lightmaps; the Viewport draws the display mesh with this each frame, passing the
 *  world→clip `uProjection`. Warm samplers default to `empty` (coverage 0 -> pure cold); a missing lightmap
 *  → `empty` (0 = no light, UNLIT fallback via uLightEnable). */
export class AlbedoBlitShader {
  readonly program: Program;
  albedo: Texture | null = null;
  surface: Texture | null = null;
  albedoWarm: Texture | null = null;
  surfaceWarm: Texture | null = null;
  /** #4 The baked COLD/HOT lightmap irradiance maps (from {@link ShadowGather}); null → UNLIT. Per-light N·L
   *  is baked in, so these are the whole illumination — no separate dir/unshadowed maps. */
  coldLight: Texture | null = null;
  hotLight: Texture | null = null;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "viewport-albedo-blit");
  }

  /** The sampler bindings (unset ones fall back to `empty`). A missing irradiance map → `empty` (0 = no light). */
  textures(empty: Texture): Record<string, Texture> {
    return {
      uAlbedo: this.albedo ?? empty,
      uSurface: this.surface ?? empty,
      uAlbedoWarm: this.albedoWarm ?? empty,
      uSurfaceWarm: this.surfaceWarm ?? empty,
      uColdLight: this.coldLight ?? empty,
      uHotLight: this.hotLight ?? empty,
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
