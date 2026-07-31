//! Frame-cost harness (lighting-rework P0) — the one instrument this stream measures with.
//!
//! Wall clock around N hand-driven frames, with `gl.finish()` at BOTH ends so the number prices the
//! whole frame (CPU submission + GPU execution), reported as a median of repeats.
//!
//! **Why not `EXT_disjoint_timer_query_webgl2`.** The debug tab runs backgrounded, so `rAF` never
//! fires and `setTimeout` is throttled to ~1 s. Timer-query results need an event-loop turn to
//! retire, and every faster drain fails by returning PLAUSIBLE PARTIAL DATA rather than erroring —
//! measured: driving extra frames retired 0 of 810 queries, `gl.finish()` retired 0 of 810, and
//! unthrottled `MessageChannel` yields still lost 740 of 810. A per-pass breakdown is not worth an
//! instrument that under-reports silently. See the strip's `issues.md` I7.
//!
//! The trade this accepts: one number per frame, not per pass. When a pass needs isolating, isolate
//! it by turning it OFF and differencing — which is how the strip priced the entire lighting system
//! (0.508 lit vs 0.045 unlit) without a single timer query.

/** A measurement: the median of `reps` runs, with the spread that says whether to believe it. */
export interface FrameCost {
  /** Median ms/frame across repeats — the headline. */
  ms: number;
  /** Fastest and slowest repeat. A delta smaller than this spread is not a result. */
  min: number;
  max: number;
  /** Draw calls issued in one frame, counted by wrapping the GL entry points. */
  draws: number;
  reps: number[];
}

interface Tickable { tick(): void }

/** Measure `frames` ticks after `warm` warm-up ticks, `reps` times, and return the median.
 *
 *  Warm-up matters more than it looks: the first ticks after a change do the dirty work the change
 *  caused, so measuring them prices the transition rather than the steady state. */
export function measureFrameCost(
  view: Tickable,
  gl: WebGL2RenderingContext,
  { frames = 60, warm = 15, reps = 5 } = {},
): FrameCost {
  const run = (): number => {
    for (let i = 0; i < warm; i++) view.tick();
    gl.finish();
    const t0 = performance.now();
    for (let i = 0; i < frames; i++) view.tick();
    gl.finish();                                   // the GPU must be DONE, or we time submission
    return (performance.now() - t0) / frames;
  };
  const out: number[] = [];
  for (let i = 0; i < reps; i++) out.push(run());
  const sorted = [...out].sort((a, b) => a - b);

  // Draw count is a separate, exact fact — worth reporting alongside, because a cost change with an
  // unchanged draw count means the work moved inside a pass rather than between passes.
  let draws = 0;
  const de = gl.drawElements, da = gl.drawArrays;
  gl.drawElements = function (this: WebGL2RenderingContext, ...a: unknown[]) {
    draws++; return (de as (...x: unknown[]) => void).apply(gl, a);
  } as typeof gl.drawElements;
  gl.drawArrays = function (this: WebGL2RenderingContext, ...a: unknown[]) {
    draws++; return (da as (...x: unknown[]) => void).apply(gl, a);
  } as typeof gl.drawArrays;
  view.tick();
  gl.drawElements = de; gl.drawArrays = da;

  const round = (n: number): number => Math.round(n * 1000) / 1000;
  return {
    ms: round(sorted[Math.floor(sorted.length / 2)]),
    min: round(sorted[0]),
    max: round(sorted[sorted.length - 1]),
    draws,
    reps: out.map(round),
  };
}

/** Install `__framecost([opts])` — the console entry point. */
export function installFrameCost(view: Tickable, gl: WebGL2RenderingContext): void {
  (globalThis as unknown as { __framecost: (o?: object) => FrameCost }).__framecost =
    (o?: object) => measureFrameCost(view, gl, o ?? {});
}
