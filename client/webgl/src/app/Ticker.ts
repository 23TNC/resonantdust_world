//! Ticker — the per-frame heartbeat, replacing Pixi's `Ticker`. A single
//! `requestAnimationFrame` loop that fans `deltaMS` (real milliseconds since the
//! previous frame) out to every registered callback — the canonical timebase for
//! all scene + HUD logic. Owns an optional `maxFPS` throttle (the Video panel's
//! frame-rate limit) by skipping frames that arrive sooner than the min interval.

export type TickFn = (deltaMS: number) => void;

export class Ticker {
  /** 0 = uncapped (every rAF). Otherwise the min ms between dispatched frames. */
  private minIntervalMs = 0;
  private readonly fns = new Set<TickFn>();
  private last = 0;
  private lastDispatch = 0;
  private acc = 0;
  private raf = 0;
  private running = false;

  /** Frame-rate cap (0 / undefined = uncapped). Backs the Video panel's limit. */
  set maxFPS(fps: number) {
    this.minIntervalMs = fps > 0 ? 1000 / fps : 0;
  }
  get maxFPS(): number {
    return this.minIntervalMs > 0 ? 1000 / this.minIntervalMs : 0;
  }

  add(fn: TickFn): void {
    this.fns.add(fn);
  }
  remove(fn: TickFn): void {
    this.fns.delete(fn);
  }

  start(): void {
    if (this.running) return;
    this.running = true;
    this.last = performance.now();
    this.lastDispatch = this.last;
    const loop = (t: number): void => {
      if (!this.running) return;
      const dt = t - this.last;
      this.last = t;
      // Frame-cap: accumulate elapsed time and only dispatch once a full
      // interval has passed, carrying the remainder so the average rate holds.
      if (this.minIntervalMs > 0) {
        this.acc += dt;
        if (this.acc >= this.minIntervalMs) {
          // deltaMS = the REAL time since the last dispatch — NOT `this.acc`. `acc` still holds the
          // carried sub-interval remainder from the previous dispatch, so passing it double-counts
          // that time: on a 120Hz display capped to 60fps it inflated deltaMS to ~22.9ms (should be
          // 16.7), running the sim ~1.37× too fast and reading ~44fps instead of 60.
          const step = t - this.lastDispatch;
          this.lastDispatch = t;
          // Carry the sub-interval remainder (NOT reset to 0) so the average dispatch RATE holds
          // exactly at maxFPS; `%=` also collapses a large post-stall backlog into one step.
          this.acc %= this.minIntervalMs;
          for (const fn of this.fns) fn(step);
        }
      } else {
        for (const fn of this.fns) fn(dt);
      }
      this.raf = requestAnimationFrame(loop);
    };
    this.raf = requestAnimationFrame(loop);
  }

  stop(): void {
    this.running = false;
    cancelAnimationFrame(this.raf);
  }
}
