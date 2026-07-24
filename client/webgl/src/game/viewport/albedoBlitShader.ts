//! The viewport's UNLIT display shader (webgl port) — draws the baked ALBEDO G-buffer
//! straight to screen, warm (movers) composited over cold by warm coverage:
//!
//!   out = albedo.rgb × alpha       (alpha = surface.B, the visual coverage)
//!
//! Ported from the pixijs high-shader to a self-contained engine `Program`: the vertex maps
//! the display quad's world px → clip via `uProjection` (folding pan + zoom + screen→clip,
//! set by the Viewport), replacing Pixi's transform/roundPixels boilerplate. The fragment
//! logic is verbatim. GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

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
uniform sampler2D uColdLight;    // #4 COLD lightmap SHADOWED irradiance (static lights, baked once)
uniform sampler2D uHotLight;     // #4 HOT lightmap SHADOWED irradiance (dynamic lights, per frame)
uniform sampler2D uColdUnshadowed; // #3 COLD unshadowed irradiance (rgb) + caster row (a)
uniform sampler2D uHotUnshadowed;  // #3 HOT unshadowed irradiance (rgb) + caster row (a)
uniform sampler2D uColdDir;      // COLD aggregate light DIR (RG = mean horizontal lit-from dir, enc 0.5+0.5)
uniform sampler2D uHotDir;       // HOT aggregate light DIR
uniform sampler2D uNormal;       // COLD normal composite (enc: flat = 0.5,0.5,1.0) — per-px relief
uniform sampler2D uNormalWarm;   // WARM normal composite (mover relief)
uniform sampler2D uZDepth;       // #3 zdepth-world: B byte = 0x80|row for a thing, 0 for ground
uniform int uLightEnable;        // 0 = UNLIT (albedo only) — the fallback when the lightmap isn't ready
uniform int uDepthTest;          // #3 1 = a thing in front of the caster takes the unshadowed map (0 = flat)
uniform int uPrimShadow;         // shadows-onto-prims: 1 = consume the gather's climbing prim shadow directly
                                 // (default); 0 = placeholder (billboards take full light, no on-prim shadow)
uniform int uNLbaked;            // lightmap P1 (__finelight): 1 = irr already holds per-light N·L → skip aggregate relief
uniform float uReliefStrength;   // P2 normal-relief gain (F3/F5 — tuned by eye)
uniform float uAmbient;          // #4 ambient floor — added ONCE over cold+hot (not baked into either map)
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot; // lightmap window mapping (F2: uniforms — a display consumer)
out vec4 fragColor;
const float SQ = ${SQF};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// A BILLBOARD never takes the world-space GROUND shadow. That shadow value belongs to the GROUND at this
// world position; a standing sprite is an elevated surface, so painting the ground shadow onto it reads as
// an offset flat blob (the old bug). Proper shadows-that-climb-the-billboard is a separate feature (the
// reverted shadows-on-prims stream) and must sample per-caster in the gather, NOT reuse the ground map
// here. Until then: things take the UNSHADOWED map (full light), only the ground receives shadows.
// (primRow/casterA + uDepthTest kept so a future on-prim path can re-enable a real test; __depthtest(0)
// falls back to the old ground-shadow-on-things behaviour for A/B.)
bool inFront(bool isThing, int primRow, float casterA) {
  return isThing && uDepthTest == 1;   // any thing → unshadowed (no ground shadow on the sprite)
}
// World → the toroidal lightmap texel (matches the gather's fc→world), or (-1,-1) if outside the window.
ivec2 lightTexel(vec2 world) {
  int tx = int(floor(world.x / SQ)), ty = int(floor(world.y / SQ));
  if (tx < uLWinCol || tx >= uLWinCol + uLCols || ty < uLWinRow || ty >= uLWinRow + uLRows) return ivec2(-1);
  int sx = pmod(tx, uLCols), sy = pmod(ty, uLRows);
  float lx = fract(world.x / SQ), ly = fract(world.y / SQ);
  return ivec2(sx * uLSlot + int(lx * float(uLSlot)), sy * uLSlot + int(ly * float(uLSlot)));
}
// COLD+HOT shadowed irradiance at a world position (0 if outside the window). One nearest tap; bilinear
// blends 4 of these below.
vec3 irrAt(vec2 world) {
  ivec2 lt = lightTexel(world);
  if (lt.x < 0) return vec3(0.0);
  return texelFetch(uColdLight, lt, 0).rgb + texelFetch(uHotLight, lt, 0).rgb;
}
// Bilinear the COARSE (uLSlot = 16 texels/tile) lightmap so its shadow edge blends into the sharp albedo,
// instead of showing the map's blocky texels (the gap the user flagged). MANUAL 4-tap with a per-tap
// toroidal fold — a GPU LINEAR sampler would bleed across the map's wrap seam, which can land on-screen.
vec3 irrBilinear(vec2 world) {
  float lmPx = SQ / float(uLSlot);            // world px per lightmap texel (= UNIT)
  vec2 tp = world / lmPx - 0.5;               // lightmap-texel space, texel-CENTRE aligned
  vec2 f = fract(tp), b = floor(tp);
  vec3 i00 = irrAt((b + vec2(0.5, 0.5)) * lmPx), i10 = irrAt((b + vec2(1.5, 0.5)) * lmPx);
  vec3 i01 = irrAt((b + vec2(0.5, 1.5)) * lmPx), i11 = irrAt((b + vec2(1.5, 1.5)) * lmPx);
  return mix(mix(i00, i10, f.x), mix(i01, i11, f.x), f.y);
}
void main() {
  vec4 outColor = texture(uAlbedo, vUV);
  // WARM-over-COLD: warm is slot-aligned with cold, sampled at the SAME vUV. Where no mover
  // sits, warm coverage is 0 -> pure cold.
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
      // #4: sum the COLD (static) + HOT (dynamic) lightmaps.
      int zb = int(texture(uZDepth, vUV).b * 255.0 + 0.5);
      bool isThing = (zb & 0x80) != 0;
      int primRow = zb & 0x7f;
      vec2 ldir = (texelFetch(uColdDir, lt, 0).rg + texelFetch(uHotDir, lt, 0).rg) * 2.0 - 2.0;
      vec4 coldU = texelFetch(uColdUnshadowed, lt, 0);
      vec4 hotU = texelFetch(uHotUnshadowed, lt, 0);
      // shadows-onto-prims (attempt #3): the gather now bakes the CLIMBING prim shadow into shadow-cold on
      // thing texels (from an in-family, zoom-safe receiver mask) — so the shadowed irradiance reads
      // correctly on billboards AND ground; consume it directly (uPrimShadow == 1, default). uPrimShadow == 0
      // is the placeholder A/B: a billboard takes the UNSHADOWED map (full light, no on-prim shadow).
      vec3 irr;
      if (uPrimShadow == 1) {
        irr = irrBilinear(vWorld);            // blended → shadow edge fades into the albedo (no blocky gap)
      } else {
        irr = (inFront(isThing, primRow, coldU.a) ? coldU.rgb : texelFetch(uColdLight, lt, 0).rgb)
            + (inFront(isThing, primRow, hotU.a) ? hotU.rgb : texelFetch(uHotLight, lt, 0).rgb);
      }
      // P2 per-px relief: decode the normal (warm-over-cold) and dot its HORIZONTAL part with the summed
      // aggregate lit-from direction. Flat ground (n.xy≈0) → relief 1 (neutral); a slope toward the
      // dominant light brightens, away darkens. F3: normal +Y is sprite-north but world +y is south, so
      // flip n.y into the world frame the direction lives in.
      // lightmap P1 (__finelight): when the per-light N·L is BAKED into irr, the aggregate-direction relief is
      // gone (it was the lossy stand-in) — the irradiance already carries the surface response; use it directly.
      if (uNLbaked == 1) {
        light = uAmbient + irr;
      } else {
        vec3 nEnc = mix(texture(uNormal, vUV).rgb, texture(uNormalWarm, vUV).rgb, wcov);
        vec2 nxy = nEnc.xy * 2.0 - 1.0;
        float relief = 1.0 + uReliefStrength * dot(vec2(nxy.x, -nxy.y), ldir);
        // Ambient is directionless — add it OUTSIDE the relief so flat/dark areas aren't relief-modulated.
        light = uAmbient + irr * max(relief, 0.0);
      }
    }
  }
  // Coverage applied at OUTPUT only (premultiplied) so it composites over the canvas
  // background: empty cells (alpha 0) show through, ground/things (alpha 1) draw opaque.
  fragColor = vec4(alb.rgb * light * alpha, alpha);
}
`;

/** The unlit albedo-blit material. Bind the COLD albedo + surface composites and the WARM
 *  albedo/surface; the Viewport draws the display mesh with this each frame, passing the
 *  world→clip `uProjection`. Warm samplers default to `empty` (coverage 0 -> pure cold). */
export class AlbedoBlitShader {
  readonly program: Program;
  albedo: Texture | null = null;
  surface: Texture | null = null;
  albedoWarm: Texture | null = null;
  surfaceWarm: Texture | null = null;
  /** #4 The baked COLD/HOT lightmap SHADOWED irradiance maps (from {@link ShadowGather}); null → UNLIT. */
  coldLight: Texture | null = null;
  hotLight: Texture | null = null;
  /** #3 The cold/hot UNSHADOWED irradiance + caster-row maps (att2) — the depth-test restore. */
  coldUnshadowed: Texture | null = null;
  hotUnshadowed: Texture | null = null;
  /** The cold/hot aggregate light-direction maps (P2 relief). */
  coldDir: Texture | null = null;
  hotDir: Texture | null = null;
  /** The cold + warm normal composites (P2 per-px relief). */
  normal: Texture | null = null;
  normalWarm: Texture | null = null;
  /** #3 zdepth-world composite — the thing depth for the shadow-vs-prim test. */
  zdepth: Texture | null = null;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "viewport-albedo-blit");
  }

  /** The sampler bindings (unset ones fall back to `empty`/`flat`). A missing irradiance map → `empty`
   *  (0 = no light); a missing dir/normal → `flat` (0.5,0.5,1.0 = zero dir / flat-up normal → neutral). */
  textures(empty: Texture, flat: Texture): Record<string, Texture> {
    return {
      uAlbedo: this.albedo ?? empty,
      uSurface: this.surface ?? empty,
      uAlbedoWarm: this.albedoWarm ?? empty,
      uSurfaceWarm: this.surfaceWarm ?? empty,
      uColdLight: this.coldLight ?? empty,
      uHotLight: this.hotLight ?? empty,
      uColdUnshadowed: this.coldUnshadowed ?? empty,
      uHotUnshadowed: this.hotUnshadowed ?? empty,
      uColdDir: this.coldDir ?? flat,
      uHotDir: this.hotDir ?? flat,
      uNormal: this.normal ?? flat,
      uNormalWarm: this.normalWarm ?? flat,
      uZDepth: this.zdepth ?? empty, // ground (0) → no depth test
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
