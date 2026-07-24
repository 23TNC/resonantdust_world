# Todo — shadow-edge refine (execution order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md); caveats in
[`issues.md`](issues.md). Approach (b): re-test the caster silhouette at fine res in `LIGHT_FRAG`, gated to
partial-coverage (edge) texels. The gather stays coarse + unchanged — VERIFY the corridor↔brute identity is
untouched. A/B every step against the current nearest-upsample._

## P0 · Make the fine re-test callable in the bake
- [ ] Move `walkShadow` (currently defined after `GATHER_COMMON`, so `GATHER_FRAG`-only) into the shared block
      so `LIGHT_FRAG` can call it. Confirm `casterOne`/`casterCover` are already shared (they're in
      `GATHER_COMMON`). No behaviour change to `GATHER_FRAG` — just scope.
- [ ] VERIFY: build green, gather output bit-identical (the shared-scope move must not alter `GATHER_FRAG`).

## P1 · Refine the shadow edge in `LIGHT_FRAG`, gated to `(0,1)` coverage
- [ ] Per fine texel, per presence light: after reading the coarse per-slot coverage `v9/511`, if it is `0` or
      `1`, keep it (free). If it is strictly between, re-run the caster silhouette test at the FINE texel `P`
      (a small local bucket re-walk via `walkShadow`/`casterOne`, same projection Q as the gather) and use that
      sharp coverage instead.
- [ ] Bound the re-walk: only the SHADOWING light needs re-testing (the coarse value already told us this slot
      is partially occluded), and only a few tiles around P (the caster is local to its shadow). Keep the walk
      constant-bounded (no body-modified loop condition — [[glsl-loop-condition-foot-gun]]).
- [ ] Keep the coarse value as the fallback when the re-test finds nothing (numerical edge cases → no regression).

## P2 · Verify sharpness + cost
- [ ] VERIFY in-browser: the cast-shadow edge under a bush/tree is near-pixel-perfect (no 16/tile blocks) while
      the shadow INTERIOR + open ground are unchanged. Zoom-stable (frame-indexed silhouette, co-pack).
- [ ] MEASURE fps: the refine only runs on `(0,1)` edge texels, so the added cost should be bounded by shadow
      perimeter. Confirm static many-light scenes still hold the cap (dirty-gated bake); a moving light re-bakes
      its reach as before.
- [ ] Confirm the shadow GATHER is untouched (corridor↔brute `debugReadShadow` identity still 0 mismatches).

## P3 · Close
- [ ] Docs + memory. Note the gather stayed coarse (intentional) and only the bake refines the edge.
- [ ] If (b)'s per-edge re-walk proves too costly, reconsider (a) — record the caster prim id in the gather +
      re-test that one caster ([forks.md#f2](forks.md#f2)); log the switch in [`forks.md`](forks.md) + a deviation.
