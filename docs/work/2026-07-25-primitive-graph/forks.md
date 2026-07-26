# Primitive graph — forks

_Decision points. **F1–F4 RESOLVED by the user 2026-07-25.** F5–F7 remain._

## F1 — Is the graph resolved on the CPU, or walked on the GPU? ✅ RESOLVED — BOTH POSSIBLE (2026-07-25)
**Resolved by adding `u16 parent_id`** to `light_data`/`billboard_data` and a `child` bit to `prim_data`
(reinterpreting RED's top half as `parent_id`). The graph is navigable **upward**, so the choice is no
longer architectural — it's per-consumer:
- **CPU resolves** for `light_presence` / `billboard_presence` / bucketing (user: "the CPU can work out
  what needs to be written"). It has to: deciding which tiles a light reaches *requires* its world position.
- **GPU can resolve** where needed (user: "with parent ids allocated, the GPU should also be able to if
  required") — bounded walk, constant loop + `break`.

**Sub-decision RESOLVED (2026-07-25):** the gather never walks. The **CPU stamps** the resolved position
into the leaf's RED lane (`u16 parent_id | u8 resolved_tile | u8 resolved_unit`), walking root → child
offsets → leaf. `parent_id` remains for CPU cascade/free/reconciliation. (Plus `resolved_zone` —
[I13](issues.md#i13)/[B3](blockers.md#b3).)

## F2 — What do the tile buckets hold? ✅ RESOLVED — LEAVES (2026-07-25)
`light_presence` holds **`light_data` ids**; **`billboard_presence`** (renamed from the caster buckets)
holds **`billboard_data` ids**. Decisive reasoning (user): bucketing *carriers* makes the per-tile
billboard count **unbounded**, since a carrier fans out to ≤4 recursively. Leaf bucketing keeps the
corridor walk at today's cost, 8 slots/tile.

## F3 — Child offset encoding ✅ RESOLVED — BIAS-8 (2026-07-25)
Signed via bias-8 per nibble (−8..+7 tiles / units), so children can be placed in any direction.

## F4 — Header count width ✅ DISSOLVED (2026-07-25)
Superseded by **fixed 8-px commands** — there is no count field. See F7.

## F5 — Does `prim_data` need a type/identity field? (2026-07-25, OPEN, low-stakes)
`prim_data` is pure structure (position/flags/4 refs); nothing says "this is a pawn". Fine for rendering
(presentation lives in the leaves) but gameplay/picking/debug may eventually want an object id. `G` has
`u3 reserved` after the `child` bit. **Lean: leave it out** until a consumer needs it.

## F6 — Dirty entry points, inherited (2026-07-25, carried over from light-prims)
`markPrimDirty` / `markBillboardDirty` / `markLightDirty` → `pendingRects` → `buildDirty`, cold/hot
routing generalised. **New wrinkle from inheritance:** a carrier's `hot_cold` flip (or a re-facing that
swaps definitions) re-classes its whole subtree, so the cascade must walk **down** the children — which
the CPU resolve pass already does. `parent_id` additionally lets a leaf's dirty find its carrier.

## F7 — Partial-command indexing ✅ RESOLVED — `id = 0` SENTINEL (2026-07-25)
A partial command **pads unused id slots with 0**; the scatter discards zero-id points (degenerate
`gl_Position` → clipped). Keeps the trivial `p/7`, `p%7` map with no per-point bookkeeping. Costs one
burned entry per set ([I14](issues.md#i14)).

## F8 — Where do a kind's light properties live? (2026-07-25, RESOLVED — the per-kind table)
A thing row is stride-7 `[tileX, tileY, tint, geoColor, kindId, data, variant]` — no room for colour/
reach/radius, and per-*instance* light props would be wasteful anyway (every torch is the same torch).
**Chosen: a per-kind light table in the content manifest**, beside the `thingLayout` / `thingPacked` /
`thingStems` tables the client already indexes by `kindId`. **No wire change at all.** Rejected: widening
the row (costs bytes per instance for data that is per-kind); a separate fetch (another round trip for
something the manifest already carries).

## F9 — How does a light reach the gather from content? (2026-07-25, RESOLVED — an aspect on `Primitive`)
Reverses the [F3](#f3) lean. F3 chose a parallel `lightPrims()` list because no carrier existed yet;
P3a built one, so the cheap path is now real: `billboardDataFor` already allocates a carrier per standing
prim, so a prim carrying `.light` allocates a light leaf **under that same carrier**. That is
"one placed object, two presentations" for free, with no second delivery list and no second traversal.
A pure light (no sprite) stays expressible — a prim whose only carried piece is a light.

## F10 — Multi-part pawns: this stream or a new one? (2026-07-25, RESOLVED — a new stream)
Pawns are **movers** (warm tier / `MoverLayer`), not cold things, so `prim{head, body, hand, hand}`
arrives by a different delivery path than a torch. Bundling them makes P5 unexecutable — the phase would
straddle two pipelines. **Chosen: torch (cold path) proves the two-presentation case here; pawn assembly
opens its own stream against the warm tier.** Rejected: a P7 in this stream (same straddle, later); doing
pawns first (the cold path is simpler and already carries the graph).
