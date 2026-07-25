# Primitive graph — todo

_Phases. This stream **preempts** the other lighting/shadow streams — it rewrites the records they read,
so they resume on top of it. Ordering principle: **layouts first, then the transport, then the writers,
then the readers** — the data texture must never be half-migrated across a frame boundary._

_(P0 landed 2026-07-25 → [`completed.md`](completed.md).)_

_(P1 landed 2026-07-25 → [`completed.md`](completed.md).)_

## P2d/P2e — `billboard_data` leaf (set 6) + `prim_data` node (set 1)  ← NEXT
_The degenerate form first: today every `Primitive` is already a **root prim carrying exactly one
billboard**, so the structure can land behaviour-preservingly and P3 then only adds real nesting._

- **Allocation** (`coldShadowData.ts`): one index serves as **both** the prim id and its billboard id
  (they are 1:1 in the degenerate case), which halves the bookkeeping.
  - `prim_data[idx]` (set 1): `R` = absolute `region|zone|tile|unit`; `G` = `child=0`, rotation, hot,
    cast, z, **`set_a = 6`**; `B` = `id_a = idx`.
  - `billboard_data[idx]` (set 6, NEW `BILLBOARD_DATA_BASE = 6 * 65536`): `R` = `parent_id = idx` |
    resolved tile|unit; `G` = layer/rotation/hot/cast/z_offset + authored offsets at the bias-8 zero
    (`0x88`); `B` = `definition_id`.
  - Rename the set-1 const `BILLBOARD_BASE` → `PRIM_BASE` (it *is* the node now).
- **⚠ The non-obvious part — the resolve REFERENCE point.** A billboard leaf stores only `tile|unit`
  (no `resolved_zone` — [F2](forks.md#f2)/containment), so its period is **16 tiles** and the congruent
  representative must be taken **near the billboard**, i.e. **the visited bucket tile — NOT the sample
  point `P`**. In `walkShadow` the sample point can be many tiles from the tile whose bucket is being
  read, so passing `P` would silently mis-place casters at range. The visited tile IS available at both
  call sites (`o` in the corridor branch, `lc + (dx,dy)` in the brute branch), so:
  - add a `vec2 ref` parameter to `casterOne` / `casterCover`, passing `(vec2(o) + 0.5) * UPT`;
  - `receiverCover` (via `receiverAt`) may pass its bucket tile — those are adjacent to `P`;
  - add `vec2 resolvedTilePos(uint tile, uint unit, vec2 ref)` — the 16-tile-period sibling of the
    `resolvedPos` (256-tile) helper P2c added for lights.
  Alternative if this plumbing proves noisy: give the billboard leaf a `resolved_zone` in its reserved
  ALPHA (period → 256 tiles, any nearby reference works) — costs 8 reserved bits, removes the parameter.
- **Decoders to move** (all currently read the set-1 record's `G = position`, `B = orient`):
  `casterCover`, `receiverCover`, `billboardNormal`, `baseRowOf` — position → `resolvedTilePos`,
  `definition_id` → `B >> 16`, rotation → `G` bits 26–27, z → `G` bits 16–23.
- Give the caster-bucket stride a named constant while here ([I17](issues.md#i17)).

## P3 — The graph: carriers, children, resolution
- CPU: build/maintain the prim graph; walk roots → stamp each leaf's absolute position + effective
  `hot_cold`/`cast_shadows`; fill `light_presence` / `billboard_presence` with **resolved leaves**
  ([F1](forks.md#f1)/[F2](forks.md#f2)).
- Inheritance rules: topmost `hot` forces hot; topmost `!cast_shadows` forces no-cast.
- **Rotation reconciliation** (the CPU's job, per the user's model): when a piece's desired `rotation`
  disagrees with its active definition's rotation, swap the definition — honouring `inherit_rotation`
  (on the **definition**, with a `prim_data` override) as a **one-step** parent inherit. Until the swap
  lands the old sprite renders — by design.
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
