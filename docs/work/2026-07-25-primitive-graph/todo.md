# Primitive graph — todo

_Phases. This stream **preempts** the other lighting/shadow streams — it rewrites the records they read,
so they resume on top of it. Ordering principle: **layouts first, then the transport, then the writers,
then the readers** — the data texture must never be half-migrated across a frame boundary._

## P0 — Land the layouts in VARIABLES (no code)
- Write the four records (`prim_data`, `billboard_data`, `light_data`, `definition_data`) + the fixed
  8-px command format into [`VARIABLES.md`](../../VARIABLES.md) — authoritative, code conforms after.
- **Settled 2026-07-25** (record, don't re-litigate): bias-8 signed offsets; `parent_id` on leaves +
  `child` bit on `prim_data`; presence carries **leaves** (`light_presence` / `billboard_presence`);
  rotation is a **CPU reconciliation signal**, the shader uses the **definition's** rotation, with
  `parent_rotation` for inheritance; one object per layer, no layer on `prim_data`.
- **Still open** ([blockers.md#b2](blockers.md#b2)): restore `emitter_radius` to `light_data`
  ([I10](issues.md#i10)) and pick the bit-home for a leaf's **resolved position**
  ([I11](issues.md#i11)).
- Update the presence spec to 8 slots (16 lights / 8 billboards per tile) + rename the caster buckets to
  `billboard_presence`; state the single resolve authority per consumer ([I12](issues.md#i12)).

## P1 — Command buffer v3: fixed 8-px commands
- One command = 8 px: `px0 = u8 opcode (0x01) | u8 set | 7× u16 ids` (`R=opcode|set|id0`, `G=id1|id2`,
  `B=id3|id4`, `A=id5|id6`), `px1..7` = the 7 payload records. 8 commands/row → **56 writes/row**.
- `SCATTER_VERT`: **delete the 16-iteration count scan** — record `p` → command `p/7`, slot `p%7`,
  target `(set << 16) | id[slot]`; payload px = `cmd*8 + 1 + slot`. Strictly cheaper than today.
- Writer: group by set, ≤7 records per command; handle partial commands per [F7](forks.md#f7)
  (lean: encode `(command, slot)` in the existing `aIndex` attribute — no padding waste).
- Retire the self-address write (`R`'s high half) from every record writer.
- **Verify in isolation before any record layout changes**: keep today's layouts, flip only the
  transport, confirm the scene is pixel-identical. This phase is independently provable — do not bundle
  it with the layout rewrite.

## P2 — Records: `prim_data` node + `billboard_data` leaf + `light_data`/`definition_data` rewrite
- Drop the self-address from all four (freeing the u16); apply the new lanes.
- Reconcile naming ([I8](issues.md#i8)): `prim_data` (node), `billboard_data` (leaf, NEW),
  `definition_data` (revert from `billboard_definition_data`).
- Rebuild presence + buckets on 8 slots.
- Update every shader read site (`fetchLin` decoders in `GATHER_COMMON`/`GATHER_FRAG`/`LIGHT_FRAG`:
  `receiverCover`, `casterOne`, `billboardNormal`, the light loop).

## P3 — The graph: carriers, children, resolution
- CPU: build/maintain the prim graph; walk roots → stamp each leaf's absolute position + effective
  `hot_cold`/`cast_shadows`; fill `light_presence` / `billboard_presence` with **resolved leaves**
  ([F1](forks.md#f1)/[F2](forks.md#f2)).
- Inheritance rules: topmost `hot` forces hot; topmost `!cast_shadows` forces no-cast.
- **Rotation reconciliation** (the CPU's job, per the user's model): when a piece's desired `rotation`
  disagrees with its active definition's rotation, swap the definition (honouring `parent_rotation` for
  inherited facing). Until the swap lands the old sprite renders — by design.
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
