# Primitive graph — todo

_Phases. This stream **preempts** the other lighting/shadow streams — it rewrites the records they read,
so they resume on top of it. Ordering principle: **layouts first, then the transport, then the writers,
then the readers** — the data texture must never be half-migrated across a frame boundary._

_(P0, P1, P2a–P2f, P3a, P3b landed 2026-07-25 → [`completed.md`](completed.md).)_

## P3 — The graph: carriers, children, resolution
- [x] Resolve walk: climb carrier→root, sum bias-8 offsets, stamp the leaf's absolute position.
- [x] Inheritance: `hot_cold` by OR, `cast_shadows` by AND, capped at `MAX_PRIM_DEPTH`.
- [x] Set `child = 1` + `parent_id` when a prim is authored under another; verify a 2-level chain resolves.
- [x] Sum a non-zero child offset in `resolveCarried`; verify against a hand-computed position.
- [x] Replace the "seen this frame" sweep with a mark-from-roots reachability sweep ([I9](issues.md#i9)).
- [x] Clamp `z + z_offset` to u8 and note the ceiling ([I9](issues.md#i9)).

_(Two former P3 items moved to P5 — they depend on the authoring path: [I23](issues.md#i23).)_

## P4 — Dirty, generalised (inherited from light-prims P3)
- [x] Add `markPrimDirty`; make it cascade to the carried subtree ([F6](forks.md#f6)).
- [x] Add `markLightDirty` covering placement, move and removal (queue the last reach box before freeing).
- [x] Carry cold/hot class on the `markDirty` entry point; drop the ad-hoc `cls` args + `L.dynamic` scatter.
- [x] Replace the two force-all fallbacks at `tick:1396` with scoped cascades.
- [x] Delete `coldDirty` / `forceColdDirty` / `lightsVer`; keep one explicit `rebakeAll()` for shader constants.

## P5 — Compose the real objects (authoring)
- [x] Add an optional `light` aspect (colour/reach/radius/height/hot/cast) to `Primitive`.
      Acceptance: `tsc` clean; a prim without it writes byte-identical records.
- [x] Allocate a light leaf under the SAME carrier when a standing prim has `.light`.
      Acceptance: mirror shows one prim with `set_a=6, set_b=2`, both leaves' `parent_id` equal to it.
- [x] Include carried lights when building `light_presence`.
      Acceptance: the torch's tile lists its light id; the scene is visibly lit by it.
- [x] Add a per-kind light table to the content manifest, beside `thingLayout`/`thingPacked`.
      Acceptance: `bin/dsl` emits it and the client reads a row by `kindId`.
- [x] Declare a torch kind in `content/data/things.rd` + `content/visual/things.rd`.
      Done in [P11](#p11--author-real-lights-a-torch-kind-placed-in-the-world-user-2026-07-26).
- [x] Attach the kind's light aspect to the prim in `WorldBridge.onColdThings`.
      Acceptance: a worldgen-placed torch renders its sprite and lights its surroundings.
- [x] Place the debug lights through the content path instead of `seed()`.
      Acceptance: the scene is lit with `__gather.lights.length === 0`.
- [x] Delete `this.lights`, `Light`, `seed()`, `__manylights`, `EXTRA_LIGHT_TILES`.
      Acceptance: grep clean; scene still lit; fps unchanged at zoom 0.25.
- [ ] Write each def's own rotation into `definition_data` RED (currently 0).
      Acceptance: mirror shows `rotation=3` for a W-facing def.
- [ ] Swap a piece's def when its desired rotation differs from the active def's.
      Acceptance: flipping a facing swaps `definition_id` within one frame.

## P6 — Verify
- [ ] Sweep zoom 0.25→2 and confirm shadow shape is stable (the recurring drift class).
- [ ] Re-check corridor↔brute identity after the record moves.
- [ ] Run the 50-light test through the placement path at `?focus=100,50&zoom=0.25`; record fps.
- [ ] Measure a caster-heavy scene against the bucket fan-out decision ([I3](issues.md#i3)).
- [ ] Free a nested pawn; confirm no leaked records and no orphaned subtree.
- [ ] Replace screenshot-only checks with a mirror/readback assertion for at least one invariant.

## P7 — Harden ([I26](issues.md#i26))
- [x] Release a carried light when its owner stops presenting one; dirty its last cast region first.
      Acceptance: `p.light = undefined` ⇒ 0 tiles list it and the record reads 0.
- [x] Reclaim carried-light ids through a free list.
      Acceptance: turning a torch off then on reuses the id instead of incrementing.
- [x] Detect cycles + depth-exhaustion in `resolveCarried`; warn once instead of resolving to (0,0).
      Acceptance: a hand-built cycle logs a named warning and does not teleport the leaf.
- [x] Claim a FREE carrier slot for a carried light rather than always slot b.
      Acceptance: a carrier already using slot b keeps it; the light takes the next free slot.
- [x] Give each presentation its own guard in the standing loop ([H4](issues.md#i26)).
      Acceptance: a prim with no resolvable def still gets its light processed.

## P8 — Cut the corridor walk ([I30](issues.md#i30), [F11](forks.md#f11))
- [x] Replace the 5-tile cross pad with a supercover DDA visiting each crossed tile once.
      Acceptance: 0 mismatches (met, over 102,442 texels); fetches −52.3% not 59% ([D-1](deviations.md#d-1)).
- [x] Re-measure the moving-light curve after the DDA.
      **120 fps (vsync cap) at 64 lights / 32 moving ±2 tiles per frame**, zoom 0.25. Was 34 fps at 32
      movers. Still capped at 259 lights / 224 movers, so the true ceiling is unmeasured — see [I33](issues.md#i33).
- [ ] Measure `casterOne`'s per-light cull hit-rate before attempting a union walk.
      Acceptance: a number for what fraction of candidates each cull rejects.
- [ ] Decide gather-vs-rasterise per tier from that data ([F11](forks.md#f11)).
      Acceptance: a fork row naming which tier uses which, with the cost basis.

## P9 — RETIRED, superseded by [2026-07-26-textile-slot](../2026-07-26-textile-slot/README.md)
_Screen-density tracking was the right direction but the wrong ceiling; the slot grid fixes the map size in
TILES instead. That stream now gates P10. See [I31](issues.md#i31)._
- [x] Retired in favour of the slot grid — no work lands here.
      Acceptance: P10's memory basis reads from the slot grid (96 MiB), not screen density.

## P11 — Author real lights: a `torch` kind placed in the world (user, 2026-07-26)
_The world has **zero** content-authored lights — 3,429 prims, none carrying one. `flora` briefly glowed and
was reverted ("our world doesn't have a sun") because ~230 lights read as an ambient wash, the model the
lighting design rejects. Sparse authored point lights are the intended shape. This lands FIRST because
[P10](#p10--additive-rgba32f-lightmap) cannot be verified without lights — a lighting check with no lights
passes vacuously ([D-2](deviations.md#d-2))._
- [x] Author the torch kind's `&thing.light.*` (kind added to data + visual `things.rd`, after `wolf`).
      Verified: reach 768px (6 tiles), radius 44.8, height 76.8, warm (1, 0.85, 0.55), cast, static.
- [x] Scatter it sparsely in `forest` AND `plains` (the catch-all) at 0.006, salt 10.
      Verified: 74 torches / 74 carried lights in an 8192-tile window.
- [x] Verify the whole content→light chain end to end.
      Verified: world lit by discrete warm pools, no `__torch` involved. 44/44 DSL tests green.

## P10 — Additive RGBA32F lightmap ([F11b](forks.md#f11b), [F11b.1](forks.md#f11b1))
_Memory gate CLEARED by [textile-slot](../2026-07-26-textile-slot/README.md): 128 MiB fixed on the slot
grid vs 465 MB world-sized. Verify every step against P11's lights — a lighting check with no lights passes
vacuously ([D-2](deviations.md#d-2))._
- [x] Assert `EXT_color_buffer_float` + `EXT_float_blend` at startup and fail loudly if absent.
      Both enabled in the `Renderer` ctor; a miss throws a named error rather than degrading silently.
- [x] Move the lightmap to `RGBA32F` with quantised integer per-light contributions.
      Verified: accumulator reads 308–321, ALL exact integers and >255 — impossible in the old unorm map.
_The three rows below are ONE atomic change — see [I32](issues.md#i32). Landing any alone regresses:
collapsing the tiers without the differential makes every hot-dirty tile re-sum its static lights, and the
self-heal check is vacuous until contributions can actually leak._
- [ ] Ping-pong the data texture AND the presence bands (COPY before flush, not a swap — the scatter only
      writes changed texels, so a swapped buffer would be missing every unchanged record).
      Acceptance: last frame's and this frame's state are both readable in one pass.
- [ ] Emit `new − old` in ONE differential pass, each term gated on that tile's presence.
      Acceptance: a texel whose lights and casters all held still emits exactly 0.
- [ ] Collapse hot/cold into the single accumulator.
      Acceptance: no tier plumbing remains; 64 static lights cost nothing when one mover moves.
- [ ] Build the dirty-light union from presence over old∪new tiles, including presence churn.
      Acceptance: a light pushed out of a tile's top-16 loses its contribution to that tile.
- [ ] Add the rebuild-and-diff self-heal assertion.
      Acceptance: a deliberately leaked contribution is reported, not silent.
