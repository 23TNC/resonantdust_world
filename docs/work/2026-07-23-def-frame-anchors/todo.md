# Todo — def frame/anchor rework (execution order)

_Phases run in order; each verifiable on `/overlayRT shadow-cold` + the corridor↔brute identity
diff (`__corridor(false)` + `__gather.debugReadShadow()`, must stay **0 mismatches**). Items move to
[`completed.md`](completed.md) when done **and** verified. Layout + model in
[`README.md`](README.md); amendments in [`forks.md`](forks.md)._

## P0 · Ratify the amended layout

- [ ] User confirms the [README](README.md) layout **as amended**: `frame_span` u3 added (F1 — ppu
      underivable without it), GREEN reserve corrected u14→u12 (10+10+12=32), RED tail = span + u11.
- [ ] `docs/VARIABLES.md` §prim_definition_data rewritten to the ratified layout (code conforms in
      P2/P3; VARIABLES leads).

## P1 · Pipeline invariants (art/resolver side)

- [ ] Assert/verify the resolver + pools guarantee: frames are **pow2 squares ≥ 16px**, packed
      16-aligned (per-size pools, zero padding). Warn loudly on any violation at pack time.
- [ ] Expose the resolved frame's **lod exponent** + confirm the frame's **world span** (tiles) is
      derivable at def-write (prim footprint ÷ … — the source for `frame_span`).
- [ ] (Standing chip) `bin/art` build-time enforcement: 16px floor, 1024² per-prim ceiling.

## P2 · CPU encode (`coldShadowData.definitionFor`)

- [ ] Compute the minimum bbox in **even units** (round out to 2-unit steps), `offset_x/y` as
      unsigned frame-relative units, `frame_lod` from the resolved frame side, `frame_span`,
      `nudge_x/y` in px (x: center opaque px in the bbox; y: bottom-align), `frame_anchor` =
      bottom-center (1, 2) for shadow casters.
- [ ] Drop the ±512 offset bias + the dilated-rect scheme; `defTight` (bucketing) derives from the
      anchored bbox in units.
- [ ] Compare-write unchanged (lod swap → new frame_x/y/lod/nudge in one write → re-dirty).
- [ ] `debugDef` decodes the new words.

## P3 · GPU decode (`shadowGather.casterCover` + placement)

- [ ] Reconstruct `ppu = 2^frame_lod / (16·2^frame_span)` (bit shifts, no float pow); sample
      `uv = frame_xy·16 + offset·ppu + nudge + (s, 1−t)·prim_wh·ppu`.
- [ ] Anchor-aware quad placement: card origin = prim position shifted by
      `−anchor_x·prim_width/2, −anchor_y·prim_height/2` (units); rotation=W mirrors `anchor_x`
      (0↔2) — replaces the old signed offset shift.
- [ ] `prim_data` position semantics reviewed: reported x/y + def anchor replaces the CPU-side
      base-centre precompute ([forks F3](forks.md#f3)).

## P4 · Verify (identity + visual)

- [ ] Corridor↔brute diff **0 mismatches** at ≥2 light positions.
- [ ] Silhouettes visually identical to the pre-rework baseline at zoom 0.5 / 1 / 2 (screenshots).
- [ ] Nudge check: a sprite with asymmetric transparent padding aligns opaque px bottom-center
      (compare against the P4-era dilated-rect render).
- [ ] LOD-swap check: zoom across a LOD boundary → def rewrites (new lod/nudge), no visual pop
      beyond resolution, no stale defs.

## P5 · Prim size sourcing (USER-DEFERRED — "we will fix that later")

- [ ] Move prim size off the DSL pickup onto the def's bbox model (align `thingPlacement` /
      `thing_layout` with anchored-bbox semantics). Scope TBD with the user.
