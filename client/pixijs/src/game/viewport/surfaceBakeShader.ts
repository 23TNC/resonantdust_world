//! The surface BAKE shader — the `surface` channel's per-prim material. SOURCE map is RGB:
//! R = height (unused), G = ambient occlusion, B = coverage (straight, 0 outside the object).
//!
//! NO-ALPHA model: the composite is OPAQUE (α = 1) and carries data in RGB — R = PRESENCE
//! (1 where a thing is), G = AO, B = ALPHA (the coverage, used by the display shader). Nothing
//! is premultiplied. The silhouette is a HARD `discard` on B (AA is off, so there's no soft
//! edge to blend); discarded fragments keep whatever was baked underneath, and the opaque writes
//! let the slot blit REPLACE cleanly — the root fix for the stale-tree bug. The lighting pass
//! reads R/G/B directly (no `/A` un-premultiply).
//!
//! A Pixi v8 high-shader Mesh program; drops the stock textureBit (its own uv-rect).
//!
//! GOTCHA (as the other bake shaders): a GLSL compile error draws the mesh BLACK with only a
//! console.error; a backtick inside a GLSL comment closes the template literal.

import {
  localUniformBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  UniformGroup,
} from "pixi.js";
import { compileHighShaderGlProgramES300 } from "./es3HighShader";

/** A texture's atlas-page uv rect `[offsetU, offsetV, scaleU, scaleV]`. */
function uvRect(t: Texture): Float32Array {
  return new Float32Array([t.frame.x / t.source.width, t.frame.y / t.source.height, t.frame.width / t.source.width, t.frame.height / t.source.height]);
}

const surfaceBitGl = {
  name: "surface-bake-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uSurface;   // surface source (RGB = height, ao, coverage/transparency)
      uniform vec4 uSurfaceRect;    // surface uv rect on its page (offset.xy, scale.zw)
    `,
    main: /* glsl */ `
      vec3 s = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).rgb;  // R unused, G=ao, B=coverage
      // HARD silhouette: DISCARD outside the thing (AA off, so no soft edge to blend). The
      // ground/thing already baked underneath survives in the discarded fragments; the kept
      // fragments write OPAQUE data (α=1) so the slot blit replaces cleanly (no premultiply,
      // no stale). Composite layout: R = presence (1 where a thing is), G = ao, B = alpha.
      if (s.b < 0.5) discard;
      outColor = vec4(1.0, s.g, s.b, 1.0);
    `,
  },
};

let program: GlProgram | null = null;
function surfaceProgram(): GlProgram {
  if (!program) program = compileHighShaderGlProgramES300({ name: "surface-bake", bits: [localUniformBitGl, surfaceBitGl, roundPixelsBitGl] });
  return program;
}

/** The surface bake material for one prim (its own uniform group, pooled by {@link SquareCache}). */
export class SurfaceBakeShader extends Shader {
  private _tex: Texture = Texture.EMPTY;

  /** `Mesh` requires a `TextureShader` (expose `texture`); the mesh's main texture IS the
   *  surface source, so this aliases the surface binding. */
  get texture(): Texture {
    return this._tex;
  }
  set texture(value: Texture) {
    this._tex = value;
    this.resources.uSurface = value.source;
    this.resources.uSurfaceSampler = value.source.style;
    this.resources.surfaceUniforms.uniforms.uSurfaceRect = uvRect(value);
    this.resources.surfaceUniforms.update();
  }
}

/** A fresh surface bake material (its own uniform group, so pooled instances hold per-prim params). */
export function makeSurfaceBakeShader(): SurfaceBakeShader {
  const e = Texture.EMPTY;
  return new SurfaceBakeShader({
    glProgram: surfaceProgram(),
    resources: {
      uSurface: e.source,
      uSurfaceSampler: e.source.style,
      surfaceUniforms: new UniformGroup({
        uSurfaceRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
      }),
    },
  });
}
