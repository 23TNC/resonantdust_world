# Primitive graph — todo

_Phases. This stream **preempts** the other lighting/shadow streams — it rewrites the records they read,
so they resume on top of it. Ordering principle: **layouts first, then the transport, then the writers,
then the readers** — the data texture must never be half-migrated across a frame boundary._

## P0 — Ratify + land the layouts in VARIABLES (no code)
- Write the four records (`prim_data`, `billboard_data`, `light_data`, `definition_data`) + the new
  command format into [`VARIABLES.md`](../../VARIABLES.md) — authoritative, code conforms after.
- **Settle the semantics the layouts don't yet state** ([blockers.md](blockers.md)):
  - child offset signedness ([F3](forks.md#f3) — lean bias-8) — blocks every writer/reader.
  - header count width ([F4](forks.md#f4) — lean u4) + the opcode's home ([I6](issues.md#i6)).
  - `rotation`/`layer` precedence + whether carrier rotation re-faces or orbits children ([I4](issues.md#i4)).
  - where leaves get an absolute position ([I1](issues.md#i1)) — falls out of [F1](forks.md#f1).
- Decide **[F1](forks.md#f1)** (CPU-resolved vs GPU-walked). Everything downstream depends on it.
- Update the presence/bucket spec to 8 slots (16 lights / 8 casters per tile).

## P1 — Command buffer v3: ids in the command
- Header → 16× count + opcode; add the **8 id px** block; `SCATTER_VERT` reads the target id from the id
  block instead of `px(vPayload).x >> 16`.
- Writer: group commands in 8s, pad by **repeating the id + payload** (replay-idempotent —
  [I7](issues.md#i7)); rotating-cursor + multi-row fills already exist (`rowsNeeded`), a 73-px fill just
  spans 2 rows.
- **Verify in isolation before any record changes**: keep today's record layouts, flip only the
  transport, confirm the scene is pixel-identical. This is the one phase that can be proven independently
  — do not bundle it with the layout rewrite.

## P2 — Records: `prim_data` node + `billboard_data` leaf + `light_data`/`definition_data` rewrite
- Drop the self-address from all four (freeing the u16); apply the new lanes.
- Reconcile naming ([I8](issues.md#i8)): `prim_data` (node), `billboard_data` (leaf, NEW),
  `definition_data` (revert from `billboard_definition_data`).
- Rebuild presence + buckets on 8 slots.
- Update every shader read site (`fetchLin` decoders in `GATHER_COMMON`/`GATHER_FRAG`/`LIGHT_FRAG`:
  `receiverCover`, `casterOne`, `billboardNormal`, the light loop).

## P3 — The graph: carriers, children, resolution
- CPU: build/maintain the prim graph; resolve per [F1](forks.md#f1) (lean: walk roots → stamp leaves'
  absolute position + effective `hot_cold`/`cast_shadows`/`rotation`; bucket resolved leaves).
- Inheritance rules: topmost `hot` forces hot; topmost `!cast_shadows` forces no-cast.
- Subtree lifetime: free by **reachability from placed roots** ([I9](issues.md#i9)), replacing the flat
  "seen this frame" sweep.
- Retire the bespoke light array — a light is now *carried*, never placed
  ([light-prims](../2026-07-24-light-prims/README.md) P1/P2 subsumed).

## P4 — Dirty, generalised (inherited from light-prims P3)
- `markPrimDirty` / `markBillboardDirty` / `markLightDirty` → `pendingRects` → `buildDirty`; a carrier's
  dirty **cascades to its subtree** ([F6](forks.md#f6)).
- Generalise cold/hot routing; retire `coldDirty`/`forceColdDirty`/`lightsVer` flag-flipping; keep one
  explicit `rebakeAll()` for genuinely-global shader-constant changes.

## P5 — Compose the real objects
- **torch** = prim{ billboard, light } — the first true two-presentation object.
- **pawn** = prim{ billboard head, billboard body, prim hand, prim hand }, hand = prim{ billboard, prim tool }.
- Confirms the 4-child fan-out and multi-level nesting on real content.

## P6 — Verify
- **Transport**: P1 proven pixel-identical before layouts moved.
- **Zoom sweep** (the recurring drift class) + **corridor↔brute identity** after P2/P3.
- **Perf**: the 50-light test *through the real path* at `?focus=100,50&zoom=0.25`; plus a caster-heavy
  scene to measure the bucket fan-out decision ([I3](issues.md#i3)).
- **Lifetime**: place/move/free a nested pawn (carrying hands carrying a torch carrying a light) and
  confirm no leaked records and no orphaned subtree.
