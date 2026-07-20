//! The `/overlayRT` debug material — draws ONE of the viewport's G-buffer composites straight
//! over the display, so a channel (albedo / normal / surface / depth) can be inspected in-place,
//! world-aligned, at full viewport size.
//!
//! It reuses the display mesh's geometry (the same per-square `aPosition`/`aUV` the display blit
//! fills each frame), so the overlay samples the chosen composite in exact register with the
//! world beneath it. `textureBit` samples the mesh's main texture (bound to the composite) into
//! `outColor`; this bit then re-emits it with a per-mode "drop the empty/default value" rule so
//! the overlay reveals only the channel's MEANINGFUL pixels and the scene shows through
//! everywhere else:
//!
//!   • {@link OVERLAY_ALL}   — albedo / surface: every texel opaque (nothing dropped).
//!   • {@link OVERLAY_FLAT}  — normal: drop flat-up (`0x8080ff`) + empty/black cells (no relief).
//!   • {@link OVERLAY_BLACK} — zdepth: drop near-black (no data there).
//!
//! Same Pixi v8 high-shader assembly as the display blit (a GLSL compile error draws the mesh
//! BLACK with only a `console.error`; a backtick inside a GLSL comment closes the literal).

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

/** Overlay draw modes — which "empty" value the shader treats as transparent so the lit scene
 *  shows through. Passed to {@link OverlayShader.mode}. */
export const OVERLAY_ALL = 0; // opaque everywhere (albedo, surface)
export const OVERLAY_FLAT = 1; // drop flat-up normal + empty cells (normal)
export const OVERLAY_BLACK = 2; // drop near-black (zdepth)
export const OVERLAY_BITS = 3; // decode a packed bitfield → one colour per set bit (bitfield-rt: lightmap-cold)

/** Pick the drop-mode for a composite by its channel name — normals hide their flat-up default,
 *  the depth channel hides its black "no data", the bitfield RT decodes per-bit, everything else is opaque. */
export function overlayModeFor(name: string): number {
  if (name.startsWith("normal")) return OVERLAY_FLAT;
  // zdepth hides its black "no data" so the scene reads through where there's no thing depth.
  if (name.startsWith("zdepth")) return OVERLAY_BLACK;
  // bitfield-rt experiment: lightmap-cold is a packed bitfield, decoded to 24 colours.
  if (name.startsWith("lightmap")) return OVERLAY_BITS;
  return OVERLAY_ALL;
}

const overlayBitGl = {
  name: "viewport-overlay-bit",
  fragment: {
    header: /* glsl */ `
      uniform float uMode;   // OVERLAY_ALL | OVERLAY_FLAT | OVERLAY_BLACK | OVERLAY_BITS

      // HSV→RGB for the bitfield palette: bit i → hue i/24, so 24 bits map to 24 spread hues.
      vec3 hsv2rgb(vec3 c) {
        vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
        vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
        return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
      }
    `,
    main: /* glsl */ `
      // outColor = the composite sample (textureBit, OPAQUE RGB). Re-emit it opaque, but drop the
      // channel's "empty" value to α = 0 so the lit viewport reads through where there's no data.
      vec3 c = outColor.rgb;
      if (uMode > 2.5) {
        // BITS mode (bitfield-rt): the RGB bytes ARE a 24-bit field (bits 0-7 R, 8-15 G, 16-23 B).
        // Pixi's high-shader compiles to GLSL ES 1.00 (no uint/bitwise), so extract bits with float
        // math — exact on rgba8 (bytes are k/255). Sum a unique hue per set bit → overlaps add.
        vec3 acc = vec3(0.0);
        float count = 0.0;
        for (int i = 0; i < 24; i++) {
          float chVal = i < 8 ? c.r : (i < 16 ? c.g : c.b);   // which byte holds bit i
          float n = floor(chVal * 255.0 + 0.5);               // reconstruct the byte 0..255
          float set = mod(floor(n / exp2(mod(float(i), 8.0))), 2.0); // bit (i mod 8) of that byte
          if (set > 0.5) { acc += hsv2rgb(vec3(float(i) / 24.0, 0.9, 1.0)); count += 1.0; }
        }
        outColor = count < 0.5 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
      } else if (uMode > 1.5) {
        // BLACK mode: zdepth — near-black is "nothing here".
        outColor = length(c) < 0.02 ? vec4(0.0) : vec4(c, 1.0);
      } else if (uMode > 0.5) {
        // FLAT mode: normal — flat-up (0.5,0.5,1.0) is bare ground, black is an empty/cleared cell.
        // (isFlat/isEmpty, NOT flat/empty: flat is a reserved GLSL interpolation qualifier.)
        bool isFlat = length(c - vec3(0.5, 0.5, 1.0)) < 0.06;
        bool isEmpty = length(c) < 0.06;
        outColor = (isFlat || isEmpty) ? vec4(0.0) : vec4(c, 1.0);
      } else {
        // ALL mode: albedo / surface — always opaque.
        outColor = vec4(c, 1.0);
      }
    `,
  },
};

let program: GlProgram | null = null;
function overlayProgram(): GlProgram {
  if (!program) {
    program = compileHighShaderGlProgram({
      name: "viewport-overlay",
      // textureBit BEFORE overlayBit: outColor must hold the composite sample when overlayBit runs.
      bits: [localUniformBitGl, textureBitGl, overlayBitGl, roundPixelsBitGl],
    });
  }
  return program;
}

/** The overlay material. Bind the composite to inspect via {@link texture} and set {@link mode}
 *  (usually via {@link overlayModeFor}); the display mesh samples it through its `aUV`. */
export class OverlayShader extends Shader {
  private _texture: Texture = Texture.EMPTY;

  /** The composite to overlay — the mesh's MAIN texture (`textureBit` samples it into `outColor`).
   *  Identity texture-matrix: `vUV` = `aUV` (the composites are stored upright). */
  get texture(): Texture {
    return this._texture;
  }
  set texture(value: Texture) {
    this._texture = value;
    this.resources.uTexture = value.source;
    this.resources.uSampler = value.source.style;
  }

  /** The drop-mode ({@link OVERLAY_ALL} | {@link OVERLAY_FLAT} | {@link OVERLAY_BLACK} | {@link OVERLAY_BITS}). */
  set mode(value: number) {
    this.resources.overlayUniforms.uniforms.uMode = value;
    this.resources.overlayUniforms.update();
  }
}

export function makeOverlayShader(): OverlayShader {
  const empty = Texture.EMPTY;
  return new OverlayShader({
    glProgram: overlayProgram(),
    resources: {
      uTexture: empty.source,
      uSampler: empty.source.style,
      // Identity — vUV = aUV (composite coords), no frame/flip remap (composites are upright).
      textureUniforms: { uTextureMatrix: { type: "mat3x3<f32>", value: new Matrix() } },
      overlayUniforms: new UniformGroup({
        uMode: { value: OVERLAY_ALL, type: "f32" },
      }),
    },
  });
}
