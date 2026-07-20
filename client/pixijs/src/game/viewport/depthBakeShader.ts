//! The zdepth_world BAKE shader — the `zdepth_world` channel's per-prim material. NO-ALPHA,
//! OPAQUE: it writes a thing's constant tile-Y depth into B (`out = (0, 0, tileDepth, 1)`), with
//! R reserved for COLD (static, world-space) shadow and G unused. Ground writes opaque black
//! (`(0,0,0,1)`) — it is NOT a thing; thing-vs-ground is read from `surface.R` (presence), so
//! this channel doesn't need alpha to mark it. The thing silhouette is a HARD `discard` on
//! `surface.B` (AA off); the opaque writes let the slot blit REPLACE the slot exactly, which is
//! the fix for the stale-tree bug (a transparent write left the old tree behind).
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
      // zdepth_world (OPAQUE, no alpha): B = the thing's tile-Y depth, R reserved for COLD
      // (static, world-space) shadow, G unused. Ground writes opaque black — it is NOT a thing;
      // thing-vs-ground is read from surface.R (presence), not from this alpha. The HARD thing
      // silhouette is a discard on surface.B (AA off), so the opaque write lets the slot blit
      // REPLACE cleanly (the fix for the stale-tree bug).
      if (uTileDepth < 0.0) {
        outColor = vec4(0.0, 0.0, 0.0, 1.0);       // ground / geo-tier: opaque black, no thing
      } else {
        float cov = texture(uSprite, uSpriteRect.xy + vUV * uSpriteRect.zw).b;   // surface.B coverage
        if (cov < 0.5) discard;                    // HARD thing silhouette
        outColor = vec4(0.0, 0.0, uTileDepth, 1.0);
      }
    `,
  },
};

let program: GlProgram | null = null;
function depthProgram(): GlProgram {
  if (!program) program = compileHighShaderGlProgramES300({ name: "depth-bake", bits: [localUniformBitGl, depthBitGl, roundPixelsBitGl] });
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
