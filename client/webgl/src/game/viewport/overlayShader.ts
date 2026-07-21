//! The `/overlayRT` debug material (webgl port) — draws ONE of the viewport's G-buffer composites
//! straight over the display, so a channel (albedo / normal / surface / zdepth / shadow) can be
//! inspected in-place, world-aligned, at full viewport size.
//!
//! Reuses the display quad's geometry (the same per-square `aPosition`/`aUV` the blit fills each
//! frame) + the same world→clip `uProjection`, so the overlay samples the chosen composite in exact
//! register with the world beneath it. Ported from the pixijs high-shader to a self-contained engine
//! `Program` (GLSL drop-mode logic verbatim; only the harness — a `uProjection` vertex + a single
//! `uComposite` sampler — replaces Pixi's textureBit/localUniform boilerplate). Each mode re-emits the
//! sample opaque but drops the channel's "empty" value to α=0 so the scene reads through:
//!
//!   • {@link OVERLAY_ALL}   — albedo / surface: every texel opaque (nothing dropped).
//!   • {@link OVERLAY_FLAT}  — normal: drop flat-up (0x8080ff) + empty/black cells (no relief).
//!   • {@link OVERLAY_BLACK} — zdepth: drop near-black (no data there).
//!   • {@link OVERLAY_BITS}  — shadow bitfield: decode the RED byte → one colour per set bit.
//!
//! GOTCHA: a backtick inside the GLSL closes the `/* glsl */` literal.

import { Program, type Texture } from "../../gl";

/** Overlay draw modes — which "empty" value the shader treats as transparent so the lit scene
 *  shows through. Passed to {@link OverlayShader.mode}. */
export const OVERLAY_ALL = 0; // opaque everywhere (albedo, surface)
export const OVERLAY_FLAT = 1; // drop flat-up normal + empty cells (normal)
export const OVERLAY_BLACK = 2; // drop near-black (zdepth)
export const OVERLAY_BITS = 3; // decode a shadow bitfield's RED byte → one colour per set bit (shadow-*)

/** Pick the drop-mode for a composite by its channel name — normals hide their flat-up default,
 *  the depth channel hides its black "no data", a shadow bitfield decodes per-bit, else opaque. */
export function overlayModeFor(name: string): number {
  if (name.startsWith("normal")) return OVERLAY_FLAT;
  if (name.startsWith("zdepth")) return OVERLAY_BLACK;
  if (name.startsWith("shadow")) return OVERLAY_BITS;
  return OVERLAY_ALL;
}

const OVERLAY_VERT = /* glsl */ `#version 300 es
in vec2 aPosition;              // display-quad world px (same as the blit)
in vec2 aUV;                    // composite UV (identity: the bake stores upright)
uniform mat3 uProjection;       // world px → clip (pan + zoom + screen→clip)
out vec2 vUV;
void main() {
  vUV = aUV;
  vec3 p = uProjection * vec3(aPosition, 1.0);
  gl_Position = vec4(p.xy, 0.0, 1.0);
}
`;
const OVERLAY_FRAG = /* glsl */ `#version 300 es
precision highp float;
in vec2 vUV;
uniform sampler2D uComposite;  // the G-buffer channel to inspect (bound by the Viewport)
uniform float uMode;           // OVERLAY_ALL | OVERLAY_FLAT | OVERLAY_BLACK | OVERLAY_BITS
out vec4 fragColor;
void main() {
  // c = the composite sample (OPAQUE RGB). Re-emit it opaque, but drop the channel's "empty"
  // value to alpha 0 so the lit viewport reads through where there's no data. (Output is
  // premultiplied — for opaque pixels alpha is 1, so c*1 = c — matching the display blit's blend.)
  vec3 c = texture(uComposite, vUV).rgb;
  vec4 outColor;
  if (uMode > 2.5) {
    // BITS: shadow bitfield — the RED byte's bits 0..4 are 5 lights. Sum a colour per set bit
    // (overlaps add); transparent where no bit is set. Float bit-extract (exact on rgba8).
    float n = floor(c.r * 255.0 + 0.5);
    vec3 acc = vec3(0.0);
    if (mod(floor(n /  1.0), 2.0) > 0.5) acc += vec3(1.0, 0.25, 0.25); // light 0 — red
    if (mod(floor(n /  2.0), 2.0) > 0.5) acc += vec3(0.25, 1.0, 0.30); // light 1 — green
    if (mod(floor(n /  4.0), 2.0) > 0.5) acc += vec3(0.30, 0.55, 1.0); // light 2 — blue
    if (mod(floor(n /  8.0), 2.0) > 0.5) acc += vec3(1.0, 0.95, 0.25); // light 3 — yellow
    if (mod(floor(n / 16.0), 2.0) > 0.5) acc += vec3(1.0, 0.35, 1.0);  // light 4 — magenta
    outColor = length(acc) < 0.01 ? vec4(0.0) : vec4(clamp(acc, 0.0, 1.0), 1.0);
  } else if (uMode > 1.5) {
    // BLACK: zdepth — near-black is "nothing here".
    outColor = length(c) < 0.02 ? vec4(0.0) : vec4(c, 1.0);
  } else if (uMode > 0.5) {
    // FLAT: normal — flat-up (0.5,0.5,1.0) is bare ground, black is an empty/cleared cell.
    bool isFlat = length(c - vec3(0.5, 0.5, 1.0)) < 0.06;
    bool isEmpty = length(c) < 0.06;
    outColor = (isFlat || isEmpty) ? vec4(0.0) : vec4(c, 1.0);
  } else {
    // ALL: albedo / surface — always opaque.
    outColor = vec4(c, 1.0);
  }
  fragColor = outColor;
}
`;

/** The overlay material. Bind the composite to inspect via {@link composite} and set {@link mode}
 *  (usually via {@link overlayModeFor}); the Viewport draws the display geometry with this, over the
 *  lit blit, passing the same world→clip `uProjection`. */
export class OverlayShader {
  readonly program: Program;
  composite: Texture | null = null;
  mode = OVERLAY_ALL;

  constructor(gl: WebGL2RenderingContext) {
    this.program = new Program(gl, OVERLAY_VERT, OVERLAY_FRAG, "viewport-overlay");
  }

  /** The single sampler binding (unset falls back to `empty`). */
  textures(empty: Texture): Record<string, Texture> {
    return { uComposite: this.composite ?? empty };
  }

  destroy(): void {
    this.program.destroy();
  }
}
