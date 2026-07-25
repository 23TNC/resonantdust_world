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
- [ ] Delete `coldDirty` / `forceColdDirty` / `lightsVer`; keep one explicit `rebakeAll()` for shader constants.

## P5 — Compose the real objects
- [ ] Build a torch = prim{ billboard, light }; verify both leaves resolve to the one carrier.
- [ ] Build a pawn = prim{ head, body, hand-prim, hand-prim }; verify the 4-child fan-out.
- [ ] Hang a torch off a hand prim; verify 3-level nesting resolves and moves with the pawn.
- [ ] Author a light kind in `content/data/things.rd` so worldgen can place one.
- [ ] Supply `layer` / `rotation` / `type` / `inherit_rotation` from the DSL (all written 0 today).
- [ ] Record each def's own rotation, then swap a piece's def when its desired rotation differs ([I23](issues.md#i23)).
- [ ] Delete `this.lights` + `seed()` + `__manylights` once worldgen places lights ([I23](issues.md#i23)).

## P6 — Verify
- [ ] Sweep zoom 0.25→2 and confirm shadow shape is stable (the recurring drift class).
- [ ] Re-check corridor↔brute identity after the record moves.
- [ ] Run the 50-light test through the placement path at `?focus=100,50&zoom=0.25`; record fps.
- [ ] Measure a caster-heavy scene against the bucket fan-out decision ([I3](issues.md#i3)).
- [ ] Free a nested pawn; confirm no leaked records and no orphaned subtree.
- [ ] Replace screenshot-only checks with a mirror/readback assertion for at least one invariant.
