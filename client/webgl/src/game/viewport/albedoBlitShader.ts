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
in vec2 vUV;
uniform sampler2D uAlbedo;       // COLD albedo composite (the mesh's main texture)
uniform sampler2D uSurface;      // COLD surface: B = alpha (visual coverage)
uniform sampler2D uAlbedoWarm;   // WARM tier: mover albedo (over cold by warm coverage)
uniform sampler2D uSurfaceWarm;  // WARM tier: mover surface (its B = composite coverage)
out vec4 fragColor;
void main() {
  vec4 outColor = texture(uAlbedo, vUV);
  // WARM-over-COLD: warm is slot-aligned with cold, sampled at the SAME vUV. Where no mover
  // sits, warm coverage is 0 -> pure cold.
  float wcov = texture(uSurfaceWarm, vUV).b;
  vec4 alb = mix(outColor, texture(uAlbedoWarm, vUV), wcov);
  vec4 surf = mix(texture(uSurface, vUV), texture(uSurfaceWarm, vUV), wcov);
  float alpha = surf.b;          // visual coverage -> output alpha
  // Coverage applied at OUTPUT only (premultiplied) so it composites over the canvas
  // background: empty cells (alpha 0) show through, ground/things (alpha 1) draw opaque.
  fragColor = vec4(alb.rgb * alpha, alpha);
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

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, BLIT_VERT, BLIT_FRAG, "viewport-albedo-blit");
  }

  /** The four sampler bindings (unset ones fall back to `empty` — a 1×1 black texture whose
   *  B = 0, so a missing warm tier reads as no coverage and a missing cold surface as alpha 0). */
  textures(empty: Texture): Record<string, Texture> {
    return {
      uAlbedo: this.albedo ?? empty,
      uSurface: this.surface ?? empty,
      uAlbedoWarm: this.albedoWarm ?? empty,
      uSurfaceWarm: this.surfaceWarm ?? empty,
    };
  }

  destroy(): void {
    this.program.destroy();
  }
}
