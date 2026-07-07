//! The surface BAKE shader — the `surface` channel's per-prim material. The SURFACE source map
//! is RGB data — R = height (reserved; unused by the current lighting), G = ambient occlusion,
//! B = the coverage/transparency (diffuse alpha, straight, 0 outside the object). It carries NO
//! stored alpha (data in a source alpha channel fights the upload-premultiply + on-demand
//! downscale path — see docs/lighting.md), so the bake DERIVES the presence coverage from B and
//! premultiplies by it: `out = (R·p, G·p, B·p, p)`, `p = smoothstep(B)`.
//!
//! Presence is HARD (a semi-transparent pixel still fully occupies its cell — occlusion must see
//! it as present, or a thing's edge halos), so `p` is a `smoothstep` of B, not B itself. The
//! lighting pass then recovers `ao = G/A`, the soft transparency `B/A`, and reads `A` as presence
//! — all `/A`, exactly like the other premultiplied composites. Where B = 0 (outside the object)
//! it writes nothing, so the flat data background never clobbers a thing behind it.
//!
//! A Pixi v8 high-shader Mesh program; drops the stock textureBit (its own uv-rect).
//!
//! GOTCHA (as the other bake shaders): a GLSL compile error draws the mesh BLACK with only a
//! console.error; a backtick inside a GLSL comment closes the template literal.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  UniformGroup,
} from "pixi.js";

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
      vec3 s = texture(uSurface, uSurfaceRect.xy + vUV * uSurfaceRect.zw).rgb;
      // Presence (HARD) from B, centered on the VISIBLE edge (~0.5) — matched to the depth bake
      // + shadow caster so occlusion presence, the receiver, and the caster all share one
      // silhouette. Premultiply the whole surface by it → composite carries A = presence, and
      // G/A = ao, B/A = soft transparency (recovered AA-safe in the lighting pass).
      float p = smoothstep(0.35, 0.65, s.b);
      outColor = vec4(s * p, p);
    `,
  },
};

let program: GlProgram | null = null;
function surfaceProgram(): GlProgram {
  if (!program) program = compileHighShaderGlProgram({ name: "surface-bake", bits: [localUniformBitGl, surfaceBitGl, roundPixelsBitGl] });
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
