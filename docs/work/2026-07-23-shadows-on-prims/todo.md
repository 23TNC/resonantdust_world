# Todo — shadows on prims (execution order)

_Verifiable in-browser (the 3-light rig; focus the forest at `?focus=34,27&zoom=2.5`, freeze with
`g.lights.forEach(L=>L.dynamic=false)`). Model + inputs in [`README.md`](README.md); decisions in
[`forks.md`](forks.md)._

## P0 · Elevation + depth plumbing
- [ ] Bind `zdepth-world` (cold, and warm later) into the **gather** so it can sample the depth by WORLD
      position (the composite is world-aligned; res differs from `shadow-cold` — map by world pos). Per
      texel: is-thing + base row → `z = f(base_row − pixel_row)` ([forks F5](forks.md#f5) — the constant
      is empirical; our sprites draw full-height while the 65° card is the shadow's own vertical scale).
- [ ] VERIFY: a debug view of `z` (or the re-projected `G`) reads sensibly — 0 on ground, growing up a
      billboard, capped near the sprite top.

## P0b · Pass 1 discards prim texels
- [ ] Make the ground gather **discard** where surface/`zdepth` says a prim is drawn, so its texels are
      free for pass 2 to own ([forks F3](forks.md#f3)). Same `shadow-cold`, no second map.
- [ ] VERIFY: ground shadow reads exactly as before **except** under prims (now blank, awaiting pass 2);
      corridor↔brute still 0 mismatches on the ground texels.

## P1 · Pass 2 — the CONE construction (ratified — see README)
- [ ] Written into the **SAME `shadow-cold`** (only on prim texels), by a gather variant, per the ratified
      cone method: only where the depth map says a prim; compute `shadow.tip.y` (project caster-top
      through the light to the ground); **cull** casters by `shadow.tip.y < receiver.bottom.y <
      prim.bottom.y` + x-in-cone; **exact** test = ray∩caster-silhouette (`v` from the cone, `u` from
      `recv.x + s·(light.x − recv.x)`); exclude same-tile casters ([forks F1](forks.md#f1)); early-out lights that can't light the prim's front ([forks F8](forks.md#f8)); accumulate
      (max). Reuse `casterCover` + the corridor. The climb bound = the caster-top ray∩receiver.
- [ ] VERIFY: isolate one light + a caster behind a receiver — the shadow **climbs** the receiver
      billboard from foot up to the ray crossing; the receiver does NOT self-shadow (near bound is free
      — in-front casts on the invisible back); ground unchanged.

## P2 · Consume — no composite
- [ ] Nothing to composite ([forks F3](forks.md#f3)): the one `shadow-cold` already holds ground-on-ground
      and prim-on-prim (disjoint, presence-partitioned). The lighting bake samples it as today; confirm the
      **union dirty** (dirty if either shadow changed) so a prim moving in/out re-partitions cleanly, and
      the ground→prim pass order with no clear between.
- [ ] VERIFY: full 3-light scene — shadows climb prims correctly, no crossing bands, ground reads as
      before, transparent gaps show ground shadow. A/B against `__depthmode 1` (binary); retire/keep binary.

## P3 · Cold/hot + dirty
- [ ] Mirror the cold/hot light-map split (#4) for pass 2 (a static prim shadowed by a static light = cold; by the
      dynamic light = hot; [forks F4](forks.md#f4)). Gate pass 2 by the SAME dirty as pass 1 (+ prim
      changes cascade to it).
- [ ] VERIFY: only the dynamic reach re-bakes (cold pass-2 dirty = 0 steady-state); corridor↔brute
      **bit-identical** for pass 2 as well (`__corridor` diff on the pass-2 RTs); display cap holds.

## P4 · Polish + close
- [ ] Tune the elevation constant + `G` magnitude by eye ([forks F5](forks.md#f5)); decide the binary
      path's fate; confirm perf (pass 2 adds gather draws — early-exit keeps tileless fragments cheap).
- [ ] Other-caster height residual ([forks F2](forks.md#f2)) noted as a later refinement (the old F6);
      not blocking. Docs + memory updated.
