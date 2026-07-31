//! The viewport's display shader (webgl port) — draws the baked ALBEDO G-buffer, warm (movers)
//! composited over cold by warm coverage and resolved per pixel by the zdepth painter's key:
//!
//!   out = albedo.rgb × alpha                              (alpha = surface.B, the visual coverage)
//!
//! lighting-strip P1 (2026-07-31): UNLIT. The lightmap multiply, ambient×AO, the decay glow and the
//! emissive add were removed with the lighting system that produced them. GOTCHA: a backtick inside
//! the GLSL closes the `/* glsl */` literal.
import { Program, type Texture } from "../../gl";

const BLIT_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // display-quad world px
in vec2 aUV;                    // composite UV (identity: the bake stores upright)
uniform mat3 uProjection;       // world px → clip (pan + zoom + screen→clip)
out vec2 vUV;
void main() {
  vUV = aUV;
  vec3 p = uProjection * vec3(aPosition, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const BLIT_FRAG = /* glsl */ `#version 300 es
precision highp float;
precision highp int;
in vec2 vUV;
uniform sampler2D uAlbedo;       // COLD albedo composite (the mesh's main texture)
uniform sampler2D uSurface;      // COLD surface: B = alpha (visual coverage)
uniform sampler2D uAlbedoWarm;   // WARM tier: mover albedo (over cold by warm coverage)
uniform sampler2D uSurfaceWarm;  // WARM tier: mover surface (its B = composite coverage)
uniform sampler2D uDepth;        // COLD zdepth composite — B = the painter's key (z-order)
uniform sampler2D uDepthWarm;    // WARM zdepth composite — same lane for movers
out vec4 fragColor;
void main() {
  vec4 outColor = texture(uAlbedo, vUV);
  // WARM-over-COLD: warm is slot-aligned with cold, sampled at the SAME vUV. Where no mover sits, warm
  // coverage is 0 -> pure cold.
  float wcov = texture(uSurfaceWarm, vUV).b;
  // DEPTH (pawn-render P1/F1): both tiers' zdepth B lanes carry the painter's key
  // "0x80 OR (baseRow AND 0x7f)" for things (0 for ground). A COLD thing whose base row sits
  // serially SOUTH of the mover's base row is IN FRONT — it wins the pixel OUTRIGHT and the
  // mover is occluded. Wrap-aware mod-128 compare (the viewport spans far under 64 rows);
  // equal rows keep warm-over-cold (F6). Ground (no high bit) never occludes.
  if (wcov > 0.0) {
    int cdb = int(texture(uDepth, vUV).b * 255.0 + 0.5);
    int wdb = int(texture(uDepthWarm, vUV).b * 255.0 + 0.5);
    if (cdb >= 128 && wdb >= 128) {
      int south = (cdb - wdb) & 0x7f;
      if (south > 0 && south < 64) wcov = 0.0;
    }
  }
  vec4 alb = mix(outColor, texture(uAlbedoWarm, vUV), wcov);
  vec4 surf = mix(texture(uSurface, vUV), texture(uSurfaceWarm, vUV), wcov);
  float alpha = surf.b;          // visual coverage -> output alpha
  // lighting-strip P1 (fork F1): UNLIT. The lightmap sum, the ambient x AO term, the decay glow and
  // the emissive add are all gone with the system that fed them, leaving albedo at full brightness.
  // Deliberately NOT a dim ambient floor: a dark world reads as a bug, and a token light would be a
  // lighting system small enough to feel free and permanent enough to constrain the re-think.
  // Coverage applied at OUTPUT only (premultiplied) so it composites over the canvas background: empty cells
  // (alpha 0) show through, ground/things (alpha 1) draw opaque.
  fragColor = vec4(alb.rgb * alpha, alpha);
}
`;

/** The display material. Bind the COLD albedo + surface composites and the WARM albedo/surface; the
 *  Viewport draws the display mesh with this each frame, passing the world→clip `uProjection`. Warm
 *  samplers default to `empty` (coverage 0 -> pure cold).
 *
 *  lighting-strip P1: the lightmap, decay and emissive bindings are gone — the blit is UNLIT. */
export class AlbedoBlitShader {
  readonly program: Program;
  albedo: Texture | null = null;
  surface: Texture | null = null;
  albedoWarm: Texture | null = null;
  surfaceWarm: Texture | null = null;
  /** The cold/warm zdepth composites — B carries the painter's key that resolves warm-over-cold. */
  depth: Texture | null = null;
  depthWarm: Texture | null = null;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "viewport-albedo-blit");
  }

  /** The sampler bindings (unset ones fall back to `empty`). */
  textures(empty: Texture): Record<string, Texture> {
    return {
      uAlbedo: this.albedo ?? empty,
      uSurface: this.surface ?? empty,
      uAlbedoWarm: this.albedoWarm ?? empty,
      uSurfaceWarm: this.surfaceWarm ?? empty,
      uDepth: this.depth ?? empty,
      uDepthWarm: this.depthWarm ?? empty,
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
