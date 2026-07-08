//! The normal BAKE shader — the `normal` channel's per-prim material for STANDING things.
//!
//! Our normal maps carry a FLAT background: Laigter emits a full rectangle, flat-up (0.5,0.5,1)
//! outside the sprite. Baked as a plain sprite, that flat background stamps over the whole
//! billboard box and CLOBBERS the relief of things behind it. NO-ALPHA model: instead of
//! premultiplying by coverage, we key the silhouette out of the SURFACE map's B channel and
//! `discard` outside it (HARD edge — AA is off), then write the raw normal OPAQUE (`out = (nrm,1)`).
//! Discarded fragments keep whatever was baked underneath (ground normal / a thing behind), so
//! nothing is clobbered, and the opaque write lets the slot blit REPLACE cleanly. The soft
//! edge-lerp is gone by design; if we want AA on normals we do it in the display shader.
//!
//! A Pixi v8 high-shader Mesh program; drops the stock textureBit (two differently-framed atlas
//! sub-textures — the normal LOD and the alpha LOD — each mapped by its own uv-rect).
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

const normalBitGl = {
  name: "normal-bake-bit",
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uNormalTex;  // normal LOD (RGB; flat-up background, no alpha)
      uniform sampler2D uAlpha;      // coverage source (the SURFACE map; its B = soft α)
      uniform vec4 uNormalRect;      // normal uv rect on its page (offset.xy, scale.zw)
      uniform vec4 uAlphaRect;       // surface uv rect on its page
      uniform float uHasNormal;      // 1 = sample uNormalTex, 0 = flat-up fallback
    `,
    main: /* glsl */ `
      float cov = texture(uAlpha, uAlphaRect.xy + vUV * uAlphaRect.zw).b;  // surface.B = coverage
      if (cov < 0.5) discard;                                              // HARD silhouette (AA off) — no flat-bg clobber
      vec3 nrm = uHasNormal > 0.5
        ? texture(uNormalTex, uNormalRect.xy + vUV * uNormalRect.zw).rgb
        : vec3(0.5, 0.5, 1.0);                                            // flat-up
      outColor = vec4(nrm, 1.0);   // OPAQUE raw normal (α=1); discard masks the silhouette, no premultiply
    `,
  },
  vertex: { header: "", main: "" },
};

let program: GlProgram | null = null;
function normalProgram(): GlProgram {
  if (!program) program = compileHighShaderGlProgram({ name: "normal-bake", bits: [localUniformBitGl, normalBitGl, roundPixelsBitGl] });
  return program;
}

/** The normal bake material for one prim (its own uniform group, pooled by {@link SquareCache}). */
export class NormalBakeShader extends Shader {
  private _alpha: Texture = Texture.EMPTY;

  /** `Mesh` requires a `TextureShader` (expose `texture`); the mesh's main texture is the
   *  coverage source (the surface map), so this aliases {@link alpha}. */
  get texture(): Texture {
    return this._alpha;
  }
  set texture(value: Texture) {
    this.alpha = value;
  }
  /** The coverage source (the SURFACE map LOD) — its B channel is the soft α the normal is
   *  premultiplied by. */
  set alpha(value: Texture) {
    this._alpha = value;
    this.resources.uAlpha = value.source;
    this.resources.uAlphaSampler = value.source.style;
    this.resources.normalUniforms.uniforms.uAlphaRect = uvRect(value);
    this.resources.normalUniforms.update();
  }
  /** The normal LOD (RGB), or `null` to bake a flat-up normal masked by the silhouette. */
  set normalTex(value: Texture | null) {
    const t = value ?? Texture.EMPTY;
    this.resources.uNormalTex = t.source;
    this.resources.uNormalTexSampler = t.source.style;
    const u = this.resources.normalUniforms.uniforms;
    u.uNormalRect = value ? uvRect(value) : new Float32Array([0, 0, 1, 1]);
    u.uHasNormal = value ? 1 : 0;
    this.resources.normalUniforms.update();
  }
}

/** A fresh normal bake material (its own uniform group, so pooled instances hold per-prim params). */
export function makeNormalBakeShader(): NormalBakeShader {
  const e = Texture.EMPTY;
  return new NormalBakeShader({
    glProgram: normalProgram(),
    resources: {
      uAlpha: e.source,
      uAlphaSampler: e.source.style,
      uNormalTex: e.source,
      uNormalTexSampler: e.source.style,
      normalUniforms: new UniformGroup({
        uAlphaRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uNormalRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uHasNormal: { value: 0, type: "f32" },
      }),
    },
  });
}
