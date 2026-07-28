//! Renderer — owns the WebGL2 context, the canvas, and the GL state (blend / viewport / bound target). The
//! single draw primitive binds a target, sets state, binds textures + uniforms, and draws a {@link Geometry}
//! with a {@link Program}. Explicit-first (set state per draw); a tracked-state cache can come later
//! (webgl-engine F3). Because we own the whole loop, there's no foreign framework whose state we corrupt —
//! the raw-GL interop problem that sank the Pixi approach simply doesn't exist here.

import { Program } from "./program";
import { Geometry } from "./geometry";
import { Texture } from "./texture";
import { RenderTarget } from "./renderTarget";

export type BlendMode = "none" | "normal" | "add" | "mulConstant";

export interface DrawOptions {
  program: Program;
  geometry: Geometry;
  /** null / undefined → the screen (default framebuffer). */
  target?: RenderTarget | null;
  /** Textures bound by uniform name → texture unit (assigned in insertion order). */
  textures?: Record<string, Texture>;
  /** Set uniforms just before the draw (program is already `use()`d). */
  uniforms?: (p: Program) => void;
  blend?: BlendMode;
  /** The constant for `mulConstant` blending (`dst *= blendColor`, fragment output ignored) —
   *  the decay lightmap's in-place fade (lighting-feel F2). */
  blendColor?: [number, number, number, number];
  /** Float clear before drawing (`[r,g,b,a]`), or an integer clear for a uint target. */
  clear?: [number, number, number, number];
  clearInt?: [number, number, number, number];
  mode?: number;
  /** Vertex count override for a non-indexed draw (scatter: exactly N command points). */
  count?: number;
}

export class Renderer {
  readonly gl: WebGL2RenderingContext;
  readonly canvas: HTMLCanvasElement;

  constructor(canvas?: HTMLCanvasElement) {
    this.canvas = canvas ?? document.createElement("canvas");
    const gl = this.canvas.getContext("webgl2", { alpha: false, antialias: false, premultipliedAlpha: true, powerPreference: "high-performance" });
    if (!gl) throw new Error("[gl] WebGL2 not available");
    this.gl = gl;

    // FLOAT RENDER TARGETS — enabled here, once, and asserted LOUDLY.
    //
    // `EXT_color_buffer_float` makes `rgba32float` renderable at all. `EXT_float_blend` additionally
    // permits BLENDING into it, which the additive lightmap depends on entirely (work
    // `2026-07-25-primitive-graph` F11b): every light's contribution is accumulated with
    // `blendFunc(ONE, ONE)`, and a light is REMOVED by emitting a negative fragment.
    //
    // Both are asserted rather than feature-detected because the failure is SILENT. Measured on this
    // machine: an `RGBA32UI` target with blending enabled reports a complete FBO, raises no GL error,
    // and simply keeps the LAST write instead of summing — ES 3.0 §15.1.4 skips blending for integer
    // formats. Without `EXT_float_blend` a float target degrades the same quiet way. A lightmap that
    // silently stops accumulating looks like a lighting bug anywhere but here, so refuse to start.
    for (const ext of ["EXT_color_buffer_float", "EXT_float_blend"]) {
      if (!gl.getExtension(ext)) {
        throw new Error(
          `[gl] ${ext} unavailable — the additive lightmap cannot accumulate without it. ` +
          "Refusing to start rather than rendering silently-wrong lighting.",
        );
      }
    }
  }

  /** Match the drawing buffer to the CSS size × DPR. Returns true if it changed. */
  resize(): boolean {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(this.canvas.clientWidth * dpr));
    const h = Math.max(1, Math.floor(this.canvas.clientHeight * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      return true;
    }
    return false;
  }

  private setBlend(mode: BlendMode): void {
    const gl = this.gl;
    if (mode === "none") {
      gl.disable(gl.BLEND);
      return;
    }
    gl.enable(gl.BLEND);
    if (mode === "add") gl.blendFunc(gl.ONE, gl.ONE);
    // `dst *= CONSTANT_COLOR` — the fragment's output is multiplied by ZERO, so ANY draw over the
    // target scales it in place (no read, no ping-pong). The constant arrives via blendColor.
    else if (mode === "mulConstant") gl.blendFunc(gl.ZERO, gl.CONSTANT_COLOR);
    else gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA); // "normal" (straight alpha)
  }

  draw(opts: DrawOptions): void {
    const gl = this.gl;
    // Target + viewport.
    if (opts.target) {
      opts.target.bind();
    } else {
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    }
    // Clear.
    if (opts.clearInt) {
      gl.clearBufferuiv(gl.COLOR, 0, new Uint32Array(opts.clearInt));
    } else if (opts.clear) {
      gl.clearColor(opts.clear[0], opts.clear[1], opts.clear[2], opts.clear[3]);
      gl.clear(gl.COLOR_BUFFER_BIT);
    }
    this.setBlend(opts.blend ?? "none");
    if (opts.blend === "mulConstant") {
      const c = opts.blendColor ?? [1, 1, 1, 1];
      gl.blendColor(c[0], c[1], c[2], c[3]);
    }
    opts.program.use();
    // Textures → units.
    if (opts.textures) {
      let unit = 0;
      for (const [name, tex] of Object.entries(opts.textures)) {
        tex.bind(unit);
        opts.program.uSampler(name, unit);
        unit++;
      }
    }
    opts.uniforms?.(opts.program);
    opts.geometry.draw(opts.mode, opts.count);
  }

  /** Clear the screen (default framebuffer). */
  clearScreen(r: number, g: number, b: number, a: number): void {
    const gl = this.gl;
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(r, g, b, a);
    gl.clear(gl.COLOR_BUFFER_BIT);
  }
}
