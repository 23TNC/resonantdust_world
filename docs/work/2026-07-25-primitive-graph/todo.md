# Primitive graph — todo

_Phases. This stream **preempts** the other lighting/shadow streams — it rewrites the records they read,
so they resume on top of it. Ordering principle: **layouts first, then the transport, then the writers,
then the readers** — the data texture must never be half-migrated across a frame boundary._

_(P0 landed 2026-07-25 → [`completed.md`](completed.md).)_

_(P1 landed 2026-07-25 → [`completed.md`](completed.md).)_

_(P2a–P2e landed 2026-07-25 → [`completed.md`](completed.md). Remaining: P2f naming/stride tidy.)_

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
