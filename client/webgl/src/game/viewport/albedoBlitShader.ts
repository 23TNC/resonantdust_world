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
uniform sampler2D uLightmap;     // baked LIGHTMAP irradiance (world-space toroidal, TEXEL-aligned w/ shadow-cold)
uniform sampler2D uLightDir;     // baked aggregate light DIR (RG = mean horizontal lit-from dir, enc 0.5+0.5)
uniform sampler2D uNormal;       // COLD normal composite (enc: flat = 0.5,0.5,1.0) — per-px relief
uniform sampler2D uNormalWarm;   // WARM normal composite (mover relief)
uniform int uLightEnable;        // 0 = UNLIT (albedo only) — the fallback when the lightmap isn't ready
uniform float uReliefStrength;   // P2 normal-relief gain (F3/F5 — tuned by eye)
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot; // lightmap window mapping (F2: uniforms — a display consumer)
out vec4 fragColor;
const float SQ = ${SQF};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// World → the toroidal lightmap texel (matches the gather's fc→world), or (-1,-1) if outside the window.
ivec2 lightTexel(vec2 world) {
  int tx = int(floor(world.x / SQ)), ty = int(floor(world.y / SQ));
  if (tx < uLWinCol || tx >= uLWinCol + uLCols || ty < uLWinRow || ty >= uLWinRow + uLRows) return ivec2(-1);
  int sx = pmod(tx, uLCols), sy = pmod(ty, uLRows);
  float lx = fract(world.x / SQ), ly = fract(world.y / SQ);
  return ivec2(sx * uLSlot + int(lx * float(uLSlot)), sy * uLSlot + int(ly * float(uLSlot)));
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
      vec3 irr = texelFetch(uLightmap, lt, 0).rgb;
      // P2 per-px relief: decode the normal (warm-over-cold) and dot its HORIZONTAL part with the
      // aggregate lit-from direction. Flat ground (n.xy≈0) → relief 1 (neutral); a slope toward the
      // dominant light brightens, away darkens. F3: normal +Y is sprite-north but world +y is south,
      // so flip n.y into the world frame the direction lives in.
      vec3 nEnc = mix(texture(uNormal, vUV).rgb, texture(uNormalWarm, vUV).rgb, wcov);
      vec2 nxy = nEnc.xy * 2.0 - 1.0;
      vec2 ldir = texelFetch(uLightDir, lt, 0).rg * 2.0 - 1.0;   // world-xy lit-from dir (mag = confidence)
      float relief = 1.0 + uReliefStrength * dot(vec2(nxy.x, -nxy.y), ldir);
      light = irr * max(relief, 0.0);
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
  /** The baked lightmap (from {@link ShadowGather}); null → the blit stays UNLIT (uLightEnable 0). */
  lightmap: Texture | null = null;
  /** The aggregate light-direction map (P2 relief) — sibling of {@link lightmap}. */
  lightDir: Texture | null = null;
  /** The cold + warm normal composites (P2 per-px relief). */
  normal: Texture | null = null;
  normalWarm: Texture | null = null;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "viewport-albedo-blit");
  }

  /** The sampler bindings (unset ones fall back to `empty` — a 1×1 black texture whose B = 0, so a
   *  missing warm tier reads as no coverage and a missing cold surface as alpha 0). A missing normal
   *  falls back to `empty` (0,0,0) → decoded (-1,-1) is only sampled where alpha 0 (invisible). */
  textures(empty: Texture, flat: Texture): Record<string, Texture> {
    return {
      uAlbedo: this.albedo ?? empty,
      uSurface: this.surface ?? empty,
      uAlbedoWarm: this.albedoWarm ?? empty,
      uSurfaceWarm: this.surfaceWarm ?? empty,
      uLightmap: this.lightmap ?? empty,
      uLightDir: this.lightDir ?? flat,
      uNormal: this.normal ?? flat,
      uNormalWarm: this.normalWarm ?? flat,
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
