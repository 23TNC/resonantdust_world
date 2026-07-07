//! The depth BAKE shader — the `depth` channel's per-prim material. It writes a thing's constant
//! tile-depth into the composite PREMULTIPLIED by the thing's PRESENCE (a hard-ish mask derived
//! from `surface.B`, the coverage/transparency channel): `B = depth·p`, `A = p`. The lighting
//! pass divides `B/A` to recover the true depth even where the presence edge softens (so the
//! depth never "falls toward 0" at the edge and speckles the shadow), and reads `A` as the
//! thing-vs-ground / occlusion presence. Presence is HARD (a semi-transparent pixel still fully
//! occupies its cell for occlusion), so it's `smoothstep(surface.B)`, not the soft B itself.
//! Ground writes NOTHING (a sentinel `uTileDepth < 0` → `A = 0`), so ALPHA distinguishes thing
//! from ground, freeing the depth value to use the whole 0..1 ring.
//!
//! Drawn back-to-front by the cache's z-sort, so the front-most thing wins. The lighting pass
//! samples this composite at `vUV` and omits a shadow where the receiving thing sits at/in front
//! of the caster (wrapped compare). World-space + baked-once: re-bakes only when a prim changes.
//!
//! A Pixi v8 high-shader Mesh program. We drop the stock `textureBit` (it folds the atlas
//! matrix into `vUV`); we keep the RAW quad uv and map the sprite with its own `uv-rect`.
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

const depthBitGl = {
  name: "depth-bake-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uSprite;   // presence source (the SURFACE map; its B = coverage/transparency)
      uniform vec4 uSpriteRect;    // surface uv rect on its page (offset.xy, scale.zw)
      uniform float uTileDepth;    // this thing's tile depth 0..1, or < 0 for ground (write nothing)
    `,
    main: /* glsl */ `
      float b = texture(uSprite, uSpriteRect.xy + vUV * uSpriteRect.zw).b;   // surface.B coverage
      // HARD presence centered on the VISIBLE edge (~0.5), not the low feather: a lower band
      // dilated presence into the soft alpha fringe, so the shadow-omit shaved a bright ring of
      // (mostly-ground) feather around every thing. Keep the band matched to the shadow caster
      // + surface bake so self-shadow omits cleanly.
      float p = smoothstep(0.35, 0.65, b);
      // Premultiplied depth: B = depth·p, A = p → lighting divides B/A (AA-safe) + reads A as
      // presence. Ground (uTileDepth < 0) writes nothing, so A marks thing vs ground.
      outColor = uTileDepth < 0.0 ? vec4(0.0) : vec4(0.0, 0.0, uTileDepth * p, p);
    `,
  },
};

let program: GlProgram | null = null;
function depthProgram(): GlProgram {
  if (!program) program = compileHighShaderGlProgram({ name: "depth-bake", bits: [localUniformBitGl, depthBitGl, roundPixelsBitGl] });
  return program;
}

/** The depth bake material for one prim (its own uniform group, pooled by {@link SquareCache}). */
export class DepthBakeShader extends Shader {
  private _tex: Texture = Texture.EMPTY;

  /** `Mesh` requires a `TextureShader` (expose `texture`); the mesh's main texture is the
   *  silhouette source, so this aliases the sprite binding. */
  get texture(): Texture {
    return this._tex;
  }
  set texture(value: Texture) {
    this._tex = value;
    this.resources.uSprite = value.source;
    this.resources.uSpriteSampler = value.source.style;
    this.resources.depthUniforms.uniforms.uSpriteRect = uvRect(value);
    this.resources.depthUniforms.update();
  }
  /** This prim's constant tile depth (0..1) → premultiplied into B; a negative value marks
   *  ground (writes nothing). */
  set tileDepth(value: number) {
    this.resources.depthUniforms.uniforms.uTileDepth = value;
    this.resources.depthUniforms.update();
  }
}

/** A fresh depth bake material (its own uniform group, so pooled instances hold per-prim params). */
export function makeDepthBakeShader(): DepthBakeShader {
  const e = Texture.EMPTY;
  return new DepthBakeShader({
    glProgram: depthProgram(),
    resources: {
      uSprite: e.source,
      uSpriteSampler: e.source.style,
      depthUniforms: new UniformGroup({
        uSpriteRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uTileDepth: { value: 0, type: "f32" },
      }),
    },
  });
}
