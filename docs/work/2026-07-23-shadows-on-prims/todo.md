# Todo — shadows on prims (execution order)

_Verifiable in-browser (the 3-light rig; focus the forest at `?focus=34,27&zoom=2.5`). NOTE: the render
loop is change-gated (idles when fully static) — to inspect a static frame, keep one **zero-reach**
light `dynamic` as a keep-alive so ticks keep firing while the subject light stays put. Model + inputs in
[`README.md`](README.md); decisions in [`forks.md`](forks.md)._

**P0–P3 delivered + verified 2026-07-24 — see [`completed.md`](completed.md).** Shadows climb billboards
(zdepth → elevation → ground projection → cone culls), one map two disjoint passes in ONE gather shader,
corridor↔brute bit-identical with the prim path live, cold/hot free. Only P4 (small) remains.

## P4 · Polish + close
- [ ] Final `elevk` eye-tune — `sin65` is the principled default and live-tunable (`__elevk`); pick a
      value that seats the climb rate best across the forest ([forks F5](forks.md#f5)).
- [ ] Decide the binary path's fate — kept as the `__primshadow(false)` A/B baseline for now; retire once
      the climb is trusted.
- [ ] Other-caster height residual ([forks F2](forks.md#f2)) noted as a later refinement (the old F6); not
      blocking.
- [ ] Warm (mover) receivers — pass 2 uses the COLD zdepth today (matches the blit); movers-as-receivers is
      the warm-zdepth follow-up.
