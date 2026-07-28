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
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot; // lightmap window mapping (uLSlot = TEXTILE_LIGHT >> lod)
uniform sampler2D uDecay;        // lighting-feel P2: the COARSE decay lightmap (RGBA16F, un-quantised)
uniform int uDSlot;              // decay texels per tile (SHADOW_TEXELS >> lod)
uniform float uAoStr;            // lighting-feel P3: AO strength on the AMBIENT term (0 = off, the A/B)
uniform float uEmissiveBoost;    // lighting-feel P3: self-lit strength (0 = off, the A/B)
uniform sampler2D uDepth;        // COLD zdepth composite — R relays the emissive mask (B = depth)
uniform sampler2D uDepthWarm;    // WARM zdepth composite — same lane for movers
out vec4 fragColor;
const float SQ = ${SQF};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// lighting-feel P2: sample the decay map with MANUAL bilinear whose four taps each fold through the
// toroidal window independently — hardware LINEAR would blend across the wrap seam (opposite world
// edges). Glow is soft, so bilinear at coarse res is the whole upsample.
vec3 decaySample(vec2 world) {
  vec2 u = world / SQ * float(uDSlot) - 0.5;          // continuous decay-texel coords (world-aligned)
  vec2 f = fract(u);
  vec2 base = floor(u);
  vec3 acc = vec3(0.0);
  for (int dy = 0; dy < 2; dy++)
    for (int dx = 0; dx < 2; dx++) {
      vec2 tex = base + vec2(float(dx), float(dy));   // world-space texel index
      float wgt = (dx == 0 ? 1.0 - f.x : f.x) * (dy == 0 ? 1.0 - f.y : f.y);
      int tx = int(floor((tex.x + 0.5) / float(uDSlot)));
      int ty = int(floor((tex.y + 0.5) / float(uDSlot)));
      if (tx < uLWinCol || tx >= uLWinCol + uLCols || ty < uLWinRow || ty >= uLWinRow + uLRows) continue;
      ivec2 fold = ivec2(pmod(tx, uLCols) * uDSlot, pmod(ty, uLRows) * uDSlot)
                 + ivec2(int(tex.x) - tx * uDSlot, int(tex.y) - ty * uDSlot);
      acc += texelFetch(uDecay, fold, 0).rgb * wgt;
    }
  return acc;
}
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
      // Sampled NEAREST — the detail lives in the lighting at TEXTILE_LIGHT/tile, so NO bilinear smear (that
      // was the coarse-map blob fix). Ambient is directionless, added once here over cold+hot.
      // The lightmap is an RGBA32F ADDITIVE ACCUMULATOR holding QUANTISED integer deposits (F11b), so
      // de-quantise here — this is the one place the scale is undone.
      //
      // The clamp below is a DISPLAY clamp and must stay here (B-5). Clamping the ACCUMULATOR instead
      // would break light removal outright: if two lights each deposit 255 and their sum is clipped to
      // 255, subtracting one leaves 0 where the answer is 255, and that light can never be fully turned
      // off. Over-bright is resolved at read time; the stored sum stays exact.
      vec3 irr = (texelFetch(uColdLight, lt, 0).rgb + texelFetch(uHotLight, lt, 0).rgb) / uLightQuant;
      // lighting-feel P3: AMBIENT × AO. The surface composite is premultiplied by presence
      // (A = presence, so ao = G/A — the bake's own encoding); tiles without occlusion art carry
      // G = 1 and are untouched. AO attenuates the OMNIDIRECTIONAL term only — direct light keeps
      // its N·L + shadows (physically, occlusion of a point light IS its shadow).
      float ao = clamp(surf.g, 0.0, 1.0);          // composite G is straight (bakes are opaque)
      float amb = uAmbient * mix(1.0, ao, uAoStr);
      // lighting-feel P2: + the decay lightmap — ephemeral particle glow (flicker), un-quantised,
      // bilinear (soft by nature). Inside the same display clamp.
      light = amb + min(irr + decaySample(vWorld), vec3(4.0));
    }
  }
  // lighting-feel P3: EMISSIVE — self-lit pixels. The mask rides the DEPTH composite's R (the one
  // spare lane whose alpha stays 1 through the blended bakes — surface A is a BLEND FACTOR, not
  // storage). Added AFTER the light multiply: emission is light the material MAKES, not light it
  // receives — a wolf's eyes glow in pitch dark. uEmissiveBoost 0 = off (the A/B).
  float emask = mix(texture(uDepth, vUV).r, texture(uDepthWarm, vUV).r, wcov);
  vec3 emissive = alb.rgb * emask * uEmissiveBoost;
  // Coverage applied at OUTPUT only (premultiplied) so it composites over the canvas background: empty cells
  // (alpha 0) show through, ground/things (alpha 1) draw opaque.
  fragColor = vec4((alb.rgb * light + emissive) * alpha, alpha);
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
  /** lighting-feel P2: the coarse decay (particle glow) map; null → empty (no glow). */
  decay: Texture | null = null;
  /** lighting-feel P3: the cold/warm zdepth composites — R carries the emissive mask. */
  depth: Texture | null = null;
  depthWarm: Texture | null = null;

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
      uDecay: this.decay ?? empty,
      uDepth: this.depth ?? empty,
      uDepthWarm: this.depthWarm ?? empty,
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
