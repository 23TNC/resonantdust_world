//! `reachFromIntensity` — the ONE definition, in TS and GLSL, in one file (lighting-rework F6).
//!
//! No record holds a light's reach: both `prim_data` and `definition_data` are exactly 128 bits with
//! nothing spare. Reach is DERIVED from `u10 intensity`, which is not a workaround — physically,
//! reach *is* the distance at which a light falls below the visible threshold, so a stored field
//! would be a second source of truth for one fact.
//!
//! **Why the two implementations live in the same file.** Two consumers must agree exactly:
//!
//!   - the CPU, building each tile's 8-light set — a *reach* relation
//!   - the GPU, bounding `corridor_walk`
//!
//! Disagreement is silent and asymmetric. CPU reach > GPU reach: a light registers on tiles the walk
//! never visits — harmless, a wasted slot. CPU reach < GPU reach: the walk would reach tiles the
//! light was never registered in, so its shadow is **never cast at all** — invisible, and precisely
//! the class of bug this project keeps rediscovering. Putting them in separate files is how that
//! starts. `__reachcheck()` proves them equal at all 1024 values.

import { UNITS_PER_TILE } from "./squareMath";

/** Intensity is `u10` — 1024 levels. */
export const INTENSITY_MAX = 1023;

/** Below this fraction of full brightness a light contributes under one 8-bit level: invisible. */
export const REACH_EPSILON = 1 / 255;

/** Falloff shape constant, in UNITS. One tile — the distance at which a light is at half strength. */
export const REACH_FALLOFF_UNITS = UNITS_PER_TILE;

/** Reach in UNITS for a `u10` intensity.
 *
 *  Falloff is `L(d) = I / (1 + (d/d0)²)`; reach solves `L(d) = ε`:
 *
 *      d = d0 · sqrt(I/ε − 1)      (0 when I ≤ ε — a light too dim to see anywhere)
 *
 *  At full intensity that lands on **16 tiles**, which is deliberately the reach the stream's
 *  headline measures at, so the worst case the plan prices is also the worst case content can author.
 */
export function reachFromIntensity(intensity: number): number {
  const i = Math.max(0, Math.min(INTENSITY_MAX, Math.floor(intensity))) / INTENSITY_MAX;
  if (i <= REACH_EPSILON) return 0;
  return REACH_FALLOFF_UNITS * Math.sqrt(i / REACH_EPSILON - 1);
}

/** Reach in TILES, rounded UP — the walk bound and the tile-registration radius.
 *
 *  Ceil, never floor: a floor would under-register the rim tiles a light genuinely touches, which is
 *  the silent-loss direction described above. */
export function reachTilesFromIntensity(intensity: number): number {
  return Math.ceil(reachFromIntensity(intensity) / UNITS_PER_TILE);
}

/** The SAME function in GLSL. Injected into every shader that bounds a walk; never re-typed. */
export const REACH_GLSL = /* glsl */ `
const float INTENSITY_MAX      = ${INTENSITY_MAX}.0;
const float REACH_EPSILON      = ${REACH_EPSILON};
const float REACH_FALLOFF_UNITS = ${REACH_FALLOFF_UNITS}.0;
// Mirror of reachFromIntensity() in lightReach.ts -- kept in that file so the two cannot drift.
float reachFromIntensity(uint intensity) {
  float i = float(min(intensity, uint(INTENSITY_MAX))) / INTENSITY_MAX;
  if (i <= REACH_EPSILON) return 0.0;
  return REACH_FALLOFF_UNITS * sqrt(i / REACH_EPSILON - 1.0);
}
`;

/** Install `__reachcheck()` — runs the GLSL against the TS at every `u10` value and reports the
 *  worst disagreement. F6's acceptance; cheap enough to run whenever the falloff is touched. */
export function installReachCheck(gl: WebGL2RenderingContext): void {
  (globalThis as unknown as { __reachcheck: () => unknown }).__reachcheck = () => {
    const N = INTENSITY_MAX + 1;
    const tex = gl.createTexture(), fbo = gl.createFramebuffer();
    const prevFbo = gl.getParameter(gl.FRAMEBUFFER_BINDING) as WebGLFramebuffer | null;
    try {
      gl.bindTexture(gl.TEXTURE_2D, tex);
      gl.texStorage2D(gl.TEXTURE_2D, 1, gl.R32F, N, 1);
      gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
      gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0);
      if (gl.checkFramebufferStatus(gl.FRAMEBUFFER) !== gl.FRAMEBUFFER_COMPLETE) return { error: "fbo" };
      const vs = `#version 300 es
in vec2 aPos; void main(){ gl_Position = vec4(aPos, 0.0, 1.0); }`;
      const fs = `#version 300 es
precision highp float;
${REACH_GLSL}
out float o;
void main(){ o = reachFromIntensity(uint(gl_FragCoord.x)); }`;
      const mk = (t: number, s: string): WebGLShader => {
        const sh = gl.createShader(t)!; gl.shaderSource(sh, s); gl.compileShader(sh);
        if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(sh) ?? "");
        return sh;
      };
      const prog = gl.createProgram()!;
      gl.attachShader(prog, mk(gl.VERTEX_SHADER, vs));
      gl.attachShader(prog, mk(gl.FRAGMENT_SHADER, fs));
      gl.linkProgram(prog); gl.useProgram(prog);
      const buf = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, buf);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(prog, "aPos");
      gl.enableVertexAttribArray(loc);
      gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
      gl.viewport(0, 0, N, 1);
      gl.disable(gl.BLEND);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
      const got = new Float32Array(N);
      gl.readPixels(0, 0, N, 1, gl.RED, gl.FLOAT, got);
      gl.deleteProgram(prog); gl.deleteBuffer(buf);

      let worst = 0, worstAt = -1;
      for (let i = 0; i < N; i++) {
        const d = Math.abs(got[i] - reachFromIntensity(i));
        if (d > worst) { worst = d; worstAt = i; }
      }
      return {
        values: N,
        worstAbsDiffUnits: worst,
        worstAtIntensity: worstAt,
        agree: worst < 1e-3,
        sample: { i0: reachFromIntensity(0), iHalf: reachFromIntensity(512), iMax: reachFromIntensity(1023) },
        maxReachTiles: reachTilesFromIntensity(INTENSITY_MAX),
      };
    } finally {
      gl.bindFramebuffer(gl.FRAMEBUFFER, prevFbo);
      gl.deleteFramebuffer(fbo); gl.deleteTexture(tex);
    }
  };
}
