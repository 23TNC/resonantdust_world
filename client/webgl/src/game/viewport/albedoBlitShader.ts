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
uniform int uPrimShadow;         // shadows-on-prims: 1 = consume the gather's climbing prim shadow directly
                                 // (default); 0 = the OLD binary front-thing restore (A/B baseline)
uniform float uReliefStrength;   // P2 normal-relief gain (F3/F5 — tuned by eye)
uniform float uAmbient;          // #4 ambient floor — added ONCE over cold+hot (not baked into either map)
uniform int uLCols, uLRows, uLWinCol, uLWinRow, uLSlot; // lightmap window mapping (F2: uniforms — a display consumer)
out vec4 fragColor;
const float SQ = ${SQF};
int pmod(int a, int m) { return ((a % m) + m) % m; }
// #3: is this THING pixel (primRow, 7-bit) at or IN FRONT of (row ≥) the frontmost caster? casterA =
// the class's att2.a (caster row / 127). Signed 7-bit delta so nearby rows compare wrap-safely; a prim
// at or ahead of (south of) the caster occludes the shadow, so light it. Ground (isThing 0) always takes
// shadow. Uses delta >= 0 (not > 0) so a prim is not shadowed by its OWN shadow — a prim and the shadow
// it casts resolve to the SAME tile row, so self-cast gives delta 0. (Same-tile prim-vs-prim occlusion is
// a later problem needing sub-tile UNIT rows; for now same-tile things simply do not shadow each other.)
bool inFront(bool isThing, int primRow, float casterA) {
  if (!isThing || uDepthTest == 0) return false;
  int casterRow = int(casterA * 127.0 + 0.5);
  int d = ((primRow - casterRow) + 128) & 0x7f;
  if (d >= 64) d -= 128;
  return d >= 0;
}
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
      // #4: sum the COLD (static) + HOT (dynamic) lightmaps. #3: a THING at/in-front-of the caster takes
      // the UNSHADOWED map (the shadow is behind the sprite); ground always takes the shadowed map. The
      // proper shadow-climbs-the-billboard replacement is the 2026-07-23-shadows-on-prims stream.
      int zb = int(texture(uZDepth, vUV).b * 255.0 + 0.5);
      bool isThing = (zb & 0x80) != 0;
      int primRow = zb & 0x7f;
      vec2 ldir = (texelFetch(uColdDir, lt, 0).rg + texelFetch(uHotDir, lt, 0).rg) * 2.0 - 2.0;
      vec4 coldU = texelFetch(uColdUnshadowed, lt, 0);
      vec4 hotU = texelFetch(uHotUnshadowed, lt, 0);
      // shadows-on-prims: the gather now bakes the CLIMBING prim shadow directly into shadow-cold on
      // thing texels, so the shadowed irradiance already reads correctly on billboards — consume it as-is
      // (uPrimShadow == 1, default). uPrimShadow == 0 keeps the OLD binary restore (a whole front-thing
      // takes the unshadowed map) as an A/B baseline (retire after P4).
      vec3 irr;
      if (uPrimShadow == 1) {
        irr = texelFetch(uColdLight, lt, 0).rgb + texelFetch(uHotLight, lt, 0).rgb;
      } else {
        irr = (inFront(isThing, primRow, coldU.a) ? coldU.rgb : texelFetch(uColdLight, lt, 0).rgb)
            + (inFront(isThing, primRow, hotU.a) ? hotU.rgb : texelFetch(uHotLight, lt, 0).rgb);
      }
      // P2 per-px relief: decode the normal (warm-over-cold) and dot its HORIZONTAL part with the summed
      // aggregate lit-from direction. Flat ground (n.xy≈0) → relief 1 (neutral); a slope toward the
      // dominant light brightens, away darkens. F3: normal +Y is sprite-north but world +y is south, so
      // flip n.y into the world frame the direction lives in.
      vec3 nEnc = mix(texture(uNormal, vUV).rgb, texture(uNormalWarm, vUV).rgb, wcov);
      vec2 nxy = nEnc.xy * 2.0 - 1.0;
      float relief = 1.0 + uReliefStrength * dot(vec2(nxy.x, -nxy.y), ldir);
      // Ambient is directionless — add it OUTSIDE the relief so flat/dark areas aren't relief-modulated.
      light = uAmbient + irr * max(relief, 0.0);
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
