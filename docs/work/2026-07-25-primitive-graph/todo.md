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
- [ ] Add an optional `light` aspect (colour/reach/radius/height/hot/cast) to `Primitive`.
      Acceptance: `tsc` clean; a prim without it writes byte-identical records.
- [ ] Allocate a light leaf under the SAME carrier when a standing prim has `.light`.
      Acceptance: mirror shows one prim with `set_a=6, set_b=2`, both leaves' `parent_id` equal to it.
- [ ] Include carried lights when building `light_presence`.
      Acceptance: the torch's tile lists its light id; the scene is visibly lit by it.
- [ ] Add a per-kind light table to the content manifest, beside `thingLayout`/`thingPacked`.
      Acceptance: `bin/dsl` emits it and the client reads a row by `kindId`.
- [ ] Declare a torch kind in `content/data/things.rd` + `content/visual/things.rd`.
      Acceptance: `rd dsl` publishes; the manifest contains the torch's light row.
- [ ] Attach the kind's light aspect to the prim in `WorldBridge.onColdThings`.
      Acceptance: a worldgen-placed torch renders its sprite and lights its surroundings.
- [ ] Place the debug lights through the content path instead of `seed()`.
      Acceptance: the scene is lit with `__gather.lights.length === 0`.
- [ ] Delete `this.lights`, `Light`, `seed()`, `__manylights`, `EXTRA_LIGHT_TILES`.
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
