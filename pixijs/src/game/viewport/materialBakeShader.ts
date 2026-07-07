//! The material BAKE shader — the albedo channel's per-prim swap-in for a plain tinted
//! sprite when the prim has packed-map material bindings. It reconstructs the albedo with
//! per-material hue/chroma VARIATION (in OKLab, holding lightness fixed), so a flat albedo
//! gains pigment-like richness WITHOUT reading as light. The lighting engine never sees
//! this — it lights the varied albedo exactly as it lit the flat one. See `material.ts`,
//! `oklab.ts`, and `docs/lighting.md`.
//!
//! A Pixi v8 high-shader Mesh program. We deliberately DROP the stock `textureBit`: it
//! folds the main texture's atlas matrix into `vUV`, but we sample TWO differently-framed
//! atlas sub-textures (the albedo LOD and the packed LOD both live in `LodPool` atlas
//! pages), so we keep `vUV` as the RAW quad uv (0..1) and map each texture with its own
//! `uv-rect` uniform (frame offset/scale on its page). `localUniformBit` still supplies the
//! transform + `gl_Position`; `roundPixelsBit` snaps like the sprite path.
//!
//! Reconstruction (delta form, identity-safe): `out.rgb = albedo + Σ packedᵢ·(jitter(tintᵢ)
//! − tintᵢ)`, `out.a = albedo.a`, re-premultiplied so it composites like a sprite bake.
//!
//! GOTCHAS (as the lighting shader): a GLSL compile error draws the mesh BLACK with only a
//! console.error; `packed` is a GLSL reserved word (uniforms are `uPacked…`).

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  UniformGroup,
} from "pixi.js";
import { OKLAB_GLSL } from "../lighting/oklab";

/** Packed-map channels the shader reconstructs — the width of the `uCh*` uniform arrays
 *  and the RGBA channels of the packed map. */
export const PACKED_CHANNELS = 4;

/** A texture's atlas-page uv rect `[offsetU, offsetV, scaleU, scaleV]` — maps the quad's
 *  raw uv (0..1) onto its sub-region of the `LodPool` page. Identity `[0,0,1,1]` for a
 *  whole-page texture. */
function uvRect(t: Texture): Float32Array {
  const f = t.frame;
  const w = t.source.width;
  const h = t.source.height;
  return new Float32Array([f.x / w, f.y / h, f.width / w, f.height / h]);
}

const materialBitGl = {
  name: "material-bake-bit",
  vertex: { header: "", main: "" },
  fragment: {
    header: /* glsl */ `
      uniform sampler2D uResidual;               // packed_residual LOD (leftover; A = silhouette alpha)
      uniform sampler2D uPacked;                 // per-material weight map (RGBA coefficients)
      uniform sampler2D uNoise;                  // tiling noise atlas (rows = fields; RG = 2 decorrelated fields)
      uniform vec4 uResidualRect;                // residual uv rect on its page (offset.xy, scale.zw)
      uniform vec4 uPackedRect;                  // packed uv rect on its page
      uniform vec4 uChA[${PACKED_CHANNELS}];     // per channel: tint.rgb, hueSwing (radians)
      uniform vec4 uChB[${PACKED_CHANNELS}];     // per channel: chromaSwing, warmCoolBias, noiseRow (-1 = none), sampleSpace (0 uv | 1 world)
      uniform vec4 uNoiseParams;                 // x = atlas row count, y = uv tiling, z = world px per noise tile
      uniform vec4 uWorldRect;                   // xy = prim world origin px, zw = prim world size px (world-space noise)
      uniform float uSeed;                       // STABLE per-instance seed (cell hash for static, objectId for movers)

      ${OKLAB_GLSL}
    `,
    main: /* glsl */ `
      // CANONICAL reconstruction (split_layers): out = residual + Σ packedᵢ·jitter(tintᵢ).
      // Residual carries the silhouette alpha + the leftover (outline / un-stripped) colour;
      // each packed channel re-adds a material's contribution, hue/chroma-jittered. At
      // identity (tintᵢ = split_layers' base colour, zero swing) this rebuilds the albedo.
      vec4 resid = texture(uResidual, uResidualRect.xy + vUV * uResidualRect.zw);
      float alpha = resid.a;
      vec3 base = alpha > 0.0037 ? resid.rgb / alpha : resid.rgb;   // straight-alpha residual colour
      vec4 weights = texture(uPacked, uPackedRect.xy + vUV * uPackedRect.zw); // r,g,b,a coefficients
      vec3 outc = base;
      // Per-instance UV offset from the STABLE seed (NOT the live position) — so identical
      // sprites don't share one noise pattern, and a mover's pattern doesn't swim as it walks.
      vec2 instanceOffset = fract(vec2(
        sin(uSeed * 127.1 + 311.7),
        sin(uSeed * 269.5 + 183.3)) * 43758.5453);
      for (int i = 0; i < ${PACKED_CHANNELS}; i++) {
        vec4 A = uChA[i];
        vec4 B = uChB[i];
        vec3 tint = A.rgb;
        float noiseRow = B.z;
        // UV-space noise rides the sprite (+ per-instance offset); world-space pins it to
        // the ground so tiles of one kind don't all show an identical patch.
        vec2 nuv = B.w < 0.5
          ? vUV * uNoiseParams.y + instanceOffset
          : (uWorldRect.xy + vUV * uWorldRect.zw) / max(uNoiseParams.z, 1.0);
        vec2 n = vec2(0.5);                       // a missing field → 0.5 → no perturbation
        if (noiseRow >= 0.0) {
          float row = (noiseRow + fract(nuv.y)) / max(uNoiseParams.x, 1.0);
          n = texture(uNoise, vec2(fract(nuv.x), row)).rg;
        }
        vec3 jit = jitterHueChroma(tint, n.r, n.g, A.a, B.x, B.y);
        outc += weights[i] * jit;                 // re-add this material's (jittered) contribution
      }
      outColor = vec4(outc * alpha, alpha);        // re-premultiply (framework × vColor = white)
    `,
  },
};

let program: GlProgram | null = null;
function materialProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "material-bake",
      bits: [localUniformBitGl, materialBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** The material bake material for one prim. Bind the master ALBEDO ({@link albedo}), the
 *  PACKED map ({@link packed}), the shared NOISE atlas ({@link noise}), then push the
 *  per-channel params + world rect each bake. Pooled by {@link SquareCache} — one instance
 *  per concurrent material prim in a square, so each holds its own per-prim uniforms. */
export class MaterialBakeShader extends Shader {
  private _residual: Texture = Texture.EMPTY;

  /** `Mesh` requires its shader to be a `TextureShader` (expose `texture`). The mesh's
   *  "main" texture is the residual, so this aliases {@link residual}. */
  get texture(): Texture {
    return this._residual;
  }
  set texture(value: Texture) {
    this.residual = value;
  }

  /** The packed_residual LOD — the reconstruction base — sets its sampler + uv rect. */
  set residual(value: Texture) {
    this._residual = value;
    this.resources.uResidual = value.source;
    this.resources.uResidualSampler = value.source.style;
    this.resources.materialUniforms.uniforms.uResidualRect = uvRect(value);
    this.resources.materialUniforms.update();
  }
  /** The packed weight map LOD — sets its sampler + uv rect. */
  set packed(value: Texture) {
    this.resources.uPacked = value.source;
    this.resources.uPackedSampler = value.source.style;
    this.resources.materialUniforms.uniforms.uPackedRect = uvRect(value);
    this.resources.materialUniforms.update();
  }
  /** The shared tiling NOISE atlas (rows = fields; R/G = two decorrelated fields). */
  set noise(value: Texture) {
    this.resources.uNoise = value.source;
    this.resources.uNoiseSampler = value.source.style;
  }

  /** Push the 4 packed channels' params. `chA[i] = (tintR, tintG, tintB, hueSwing)`,
   *  `chB[i] = (chromaSwing, warmCoolBias, noiseRow, sampleSpace)`. */
  setChannels(chA: Float32Array, chB: Float32Array): void {
    const u = this.resources.materialUniforms.uniforms;
    u.uChA = chA;
    u.uChB = chB;
    this.resources.materialUniforms.update();
  }
  /** Noise globals: atlas `rows`, `uvTile` (noise tiles across a sprite), `worldTile`
   *  (world px per noise tile). */
  setNoiseGlobals(rows: number, uvTile: number, worldTile: number): void {
    this.resources.materialUniforms.uniforms.uNoiseParams = new Float32Array([rows, uvTile, worldTile, 0]);
    this.resources.materialUniforms.update();
  }
  /** The prim's world rect (px) — world-space noise samples `origin + vUV·size`. */
  setWorldRect(x: number, y: number, w: number, h: number): void {
    this.resources.materialUniforms.uniforms.uWorldRect = new Float32Array([x, y, w, h]);
    this.resources.materialUniforms.update();
  }
  /** The prim's STABLE per-instance seed (cell hash for static things, objectId for movers)
   *  — the UV-space noise offset is hashed from it, so instances differ without swimming. */
  setSeed(seed: number): void {
    this.resources.materialUniforms.uniforms.uSeed = seed;
    this.resources.materialUniforms.update();
  }
}

/** A fresh material bake material (its own uniform group, so pooled instances hold
 *  per-prim params independently). */
export function makeMaterialBakeShader(): MaterialBakeShader {
  const empty = Texture.EMPTY;
  return new MaterialBakeShader({
    glProgram: materialProgram(),
    resources: {
      uResidual: empty.source,
      uResidualSampler: empty.source.style,
      uPacked: empty.source,
      uPackedSampler: empty.source.style,
      uNoise: empty.source,
      uNoiseSampler: empty.source.style,
      materialUniforms: new UniformGroup({
        uResidualRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uPackedRect: { value: new Float32Array([0, 0, 1, 1]), type: "vec4<f32>" },
        uChA: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uChB: { value: new Float32Array(PACKED_CHANNELS * 4), type: "vec4<f32>", size: PACKED_CHANNELS },
        uNoiseParams: { value: new Float32Array([1, 1, 128, 0]), type: "vec4<f32>" },
        uWorldRect: { value: new Float32Array([0, 0, 64, 64]), type: "vec4<f32>" },
        uSeed: { value: 0, type: "f32" },
      }),
    },
  });
}
