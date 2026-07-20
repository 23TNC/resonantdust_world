//! The viewport's UNLIT display shader — the display mesh's swap-in for Pixi's default
//! flat-texture material. It draws the baked ALBEDO G-buffer straight to screen, with no
//! lighting or shadow pass:
//!
//!   out = albedo.rgb × alpha       (alpha = surface.B, the visual coverage)
//!
//! The only compositing it still does is WARM-over-COLD: the warm tier (movers/pawns) is
//! slot-aligned with cold (same window), so it samples both at the same `vUV` and blends by
//! warm coverage (warm `surface.B`). Where no mover sits, warm coverage is 0 → pure cold.
//! Without this the pawns wouldn't draw.
//!
//! This is the deliberate "lighting nuked" stopgap: the whole tiered lighting/shadow stack
//! (LightRig, cold lightmap/shadow, scatter/bitfield, the wedge shadow pass) was removed and
//! will be rebuilt. The G-buffer bakes (albedo/normal/surface/depth) survive as the foundation
//! for that retry; only ALBEDO is sampled here.
//!
//! GOTCHAS (silent failure modes): a GLSL compile error makes the mesh draw BLACK with only a
//! `console.error`; a backtick inside a GLSL comment closes the `/* glsl */` template literal.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  Matrix,
} from "pixi.js";

/** The high-shader bit that draws the albedo. `textureBit` runs BEFORE this and samples the
 *  mesh's main texture (bound to the COLD ALBEDO composite) into `outColor`, so `outColor` here
 *  IS the cold albedo; we blend the warm albedo over it by warm coverage and write it out with
 *  the composited visual alpha. */
const blitBitGl = {
  name: "viewport-albedo-blit-bit",
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uSurface;      // COLD surface: B = alpha (visual coverage)
      uniform sampler2D uAlbedoWarm;   // WARM tier: mover albedo (composited over cold by warm coverage)
      uniform sampler2D uSurfaceWarm;  // WARM tier: mover surface (its B = the composite coverage)
    `,
    main: /* glsl */ `
      // WARM-over-COLD composite: warm (movers) is slot-aligned with cold, so sample it at the
      // SAME vUV and blend by warm coverage (warm surface.B). Where no mover sits, wcov = 0 → cold.
      float wcov = texture(uSurfaceWarm, vUV).b;
      // outColor = the COLD ALBEDO composite (textureBit). Blend warm albedo over it.
      vec4 alb = mix(outColor, texture(uAlbedoWarm, vUV), wcov);
      vec4 surf = mix(texture(uSurface, vUV), texture(uSurfaceWarm, vUV), wcov);
      float alpha = surf.b;              // visual coverage → output alpha
      // Coverage is applied at OUTPUT only (premultiplied) so it composites over the canvas
      // background: empty cells (α 0) show the background, ground/things (α 1) draw opaque.
      outColor = vec4(alb.rgb * alpha, alpha);
    `,
  },
};

let program: GlProgram | null = null;
function blitProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "viewport-albedo-blit",
      // textureBit BEFORE blitBit: outColor must hold the cold-albedo sample when blitBit runs.
      bits: [localUniformBitGl, textureBitGl, blitBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** The unlit albedo-blit material. Bind the COLD ALBEDO composite via `albedo` (the mesh's main
 *  texture), the COLD SURFACE via `surface`, and the WARM albedo/surface. The display mesh samples
 *  them all through its `aUV`. */
export class AlbedoBlitShader extends Shader {
  private _albedo: Texture = Texture.EMPTY;

  /** The COLD ALBEDO composite — the mesh's MAIN texture (`textureBit` samples it into
   *  `outColor`). Identity texture-matrix: `vUV` = `aUV` (the bake stores it upright). */
  set albedo(value: Texture) {
    this._albedo = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  /** `Mesh` requires its shader to be a `TextureShader` (expose `texture`); the mesh's main
   *  texture IS the cold albedo composite, so this aliases {@link albedo}. */
  get texture(): Texture {
    return this._albedo;
  }
  set texture(value: Texture) {
    this.albedo = value;
  }
  /** The COLD SURFACE composite — its B channel is the visual coverage → output alpha. */
  set surface(value: Texture) {
    this.resources.uSurface = value.source;
    this.resources.uSurfaceSampler = value.source.style;
  }
  /** WARM-tier albedo (movers), blended over cold by warm coverage. EMPTY (coverage 0 → pure
   *  cold) until the warm cache is ready. */
  set albedoWarm(value: Texture) {
    this.resources.uAlbedoWarm = value.source;
    this.resources.uAlbedoWarmSampler = value.source.style;
  }
  /** WARM-tier surface (movers) — its B is the composite coverage that drives the warm blend. */
  set surfaceWarm(value: Texture) {
    this.resources.uSurfaceWarm = value.source;
    this.resources.uSurfaceWarmSampler = value.source.style;
  }
}

export function makeAlbedoBlitShader(): AlbedoBlitShader {
  const empty = Texture.EMPTY;
  return new AlbedoBlitShader({
    glProgram: blitProgram(),
    resources: {
      uTexture: empty.source,
      uSampler: empty.source.style,
      // Identity — vUV = aUV (composite coords), no frame/flip remap (bake is upright).
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      uSurface: empty.source,
      uSurfaceSampler: empty.source.style,
      uAlbedoWarm: empty.source,
      uAlbedoWarmSampler: empty.source.style,
      uSurfaceWarm: empty.source,
      uSurfaceWarmSampler: empty.source.style,
    },
  });
}
