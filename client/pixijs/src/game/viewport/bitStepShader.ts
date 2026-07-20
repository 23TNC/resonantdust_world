//! bitfield-rt E5 — the ping-pong read-modify-write STEP shader. Reads the SOURCE bitfield RT, produces
//! the NEXT bitfield for the DESTINATION RT: each cell's single set bit marches +1 (mod 24) per frame.
//! On the seed frame (`uSeed = 1`) each cell is seeded from its screen position instead. Proves the
//! read-back + transform + write half of the ping-pong (the display half reuses `overlayShader`'s BITS
//! decode). ES 1.00 float-mod (Pixi's high-shader is ES 1.00 — see design/rendering-platform.md).
//!
//! GOTCHA: never sample the RT currently bound as the target (feedback loop, UB) — the caller ping-pongs
//! (reads A, writes B), so source ≠ destination. Output is opaque (A = 1) so no premultiply touches the
//! data bytes.

import {
  compileHighShaderGlProgram,
  localUniformBitGl,
  textureBitGl,
  roundPixelsBitGl,
  GlProgram,
  Shader,
  Texture,
  Matrix,
  UniformGroup,
} from "pixi.js";

const stepBitGl = {
  name: "bit-step-bit",
  fragment: {
    header: /* glsl */ `
      uniform float uSeed;   // 1 on the seed frame (position-seed each cell), else 0 (march)
      uniform float uCell;   // seed-cell size in fragment px
    `,
    main: /* glsl */ `
      // outColor = the SOURCE bitfield sample (textureBit). Decode its one set bit (0..23), float-mod.
      vec3 c = outColor.rgb;
      float found = -1.0;
      for (int i = 0; i < 24; i++) {
        float chVal = i < 8 ? c.r : (i < 16 ? c.g : c.b);
        float n = floor(chVal * 255.0 + 0.5);
        if (mod(floor(n / exp2(mod(float(i), 8.0))), 2.0) > 0.5) found = float(i);
      }
      // Seed from screen position, or march the found bit. A missing bit while marching (found < 0)
      // means corruption — emit black so it's VISIBLE (not silently re-seeded).
      float nb;
      if (uSeed > 0.5) {
        vec2 cell = floor(gl_FragCoord.xy / uCell);
        nb = mod(cell.x + cell.y, 24.0);
      } else {
        nb = found < 0.0 ? -1.0 : mod(found + 1.0, 24.0);
      }
      if (nb < 0.0) {
        outColor = vec4(0.0, 0.0, 0.0, 1.0);
      } else {
        float within = mod(nb, 8.0);
        float chan = floor(nb / 8.0);            // 0 = R, 1 = G, 2 = B
        float bv = exp2(within) / 255.0;         // the one-hot byte
        outColor = vec4(chan < 0.5 ? bv : 0.0, (chan > 0.5 && chan < 1.5) ? bv : 0.0, chan > 1.5 ? bv : 0.0, 1.0);
      }
    `,
  },
};

let program: GlProgram | null = null;
function stepProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "bit-step",
      bits: [localUniformBitGl, textureBitGl, stepBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** The ping-pong step material. Bind the SOURCE bitfield via {@link field}; render into the DESTINATION
 *  RT. `seed` = 1 on the first frame (position-seed), 0 after (march). */
export class BitStepShader extends Shader {
  private _field: Texture = Texture.EMPTY;

  /** The SOURCE bitfield composite — the mesh's MAIN texture (`textureBit` samples it into `outColor`). */
  get texture(): Texture {
    return this._field;
  }
  set texture(value: Texture) {
    this._field = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }
  set field(value: Texture) {
    this.texture = value;
  }

  setSeed(seed: number): void {
    this.resources.stepUniforms.uniforms.uSeed = seed;
    this.resources.stepUniforms.update();
  }
  setCell(px: number): void {
    this.resources.stepUniforms.uniforms.uCell = px;
    this.resources.stepUniforms.update();
  }
}

export function makeBitStepShader(): BitStepShader {
  const empty = Texture.EMPTY;
  return new BitStepShader({
    glProgram: stepProgram(),
    resources: {
      uTexture: empty.source,
      uSampler: empty.source.style,
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      stepUniforms: new UniformGroup({
        uSeed: { value: 1, type: "f32" },
        uCell: { value: 48, type: "f32" },
      }),
    },
  });
}
