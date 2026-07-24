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

## P1 · Pass 2 — the elevated, self-excluding gather
- [ ] A **second `shadow-cold`** RT (per-light u9), written by a gather variant: early-exit where the
      depth map says no prim; lift the receiver to `z` (re-project `G` per light); walk the caster
      buckets **excluding casters on the receiver's own tile** ([forks F1](forks.md#f1)). Reuse
      `casterCover` + the corridor.
- [ ] VERIFY: isolate one light + a caster behind a receiver — the shadow **climbs** the receiver
      billboard at the right height; the receiver does NOT self-shadow; ground unchanged.

## P2 · Composite in the lighting bake
- [ ] Lighting pass picks per texel: **billboard** shadow where `zdepth` says a prim is drawn, **ground**
      shadow elsewhere ([forks F3](forks.md#f3) — bake-side pick, blit stays simple). Bake the chosen
      shadow into the lightmap as today.
- [ ] VERIFY: full 3-light scene — shadows climb prims correctly, no crossing bands, ground reads as
      before. A/B against `__depthmode 1` (binary) to confirm the improvement; retire/keep binary per eye.

## P3 · Cold/hot + dirty
- [ ] Mirror the [[cold/hot split]] for pass 2 (a static prim shadowed by a static light = cold; by the
      dynamic light = hot; [forks F4](forks.md#f4)). Gate pass 2 by the SAME dirty as pass 1 (+ prim
      changes cascade to it).
- [ ] VERIFY: only the dynamic reach re-bakes (cold pass-2 dirty = 0 steady-state); corridor↔brute
      **bit-identical** for pass 2 as well (`__corridor` diff on the pass-2 RTs); display cap holds.

## P4 · Polish + close
- [ ] Tune the elevation constant + `G` magnitude by eye ([forks F5](forks.md#f5)); decide the binary
      path's fate; confirm perf (pass 2 adds gather draws — early-exit keeps tileless fragments cheap).
- [ ] Other-caster height residual ([forks F2](forks.md#f2)) noted as a later refinement (the old F6);
      not blocking. Docs + memory updated.
