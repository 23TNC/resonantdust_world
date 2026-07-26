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

## F11 — Eliminating the corridor walk: cast map vs projected-fan rasterisation (2026-07-26)
User asked to explore replacing the per-texel corridor walk with a **push** model — each dirty tile casts
its shadows forward into a "cast map" holding caster ids per destination tile — and to rule it out if it
fails. **Conclusion: the cast-map form is ruled out; the push INSTINCT is right and is already specified
as [`shadow-projection`](../shadow-projection/README.md).**

**Measured baseline (16 moving lights, [I29](issues.md#i29)):** 1479 dirty tiles × 256 shadow texels ×
~660 bucket fetches/texel ≈ **250M fetches/frame** (55 fps). Overlap analysis
([I30](issues.md#i30)): 48% of walked tiles are shared between lights, and the 5-tile cross pad wastes a
further 59% *within a single* walk.

**Why the CAST MAP is ruled out** (worst problem first — note the user's suspected breaker, storage, is
the least of them):
1. **Variable fan-out.** A fragment writes ONE texel. Pushing from tile A into tiles B…N needs scatter with
   an unknown per-source count, and WebGL2 has no geometry shader. Over-provisioning points (say 64 per
   caster-light) gives `8 × 16 × 64 = 8192` points per dirty tile ≈ **12M points/frame**, mostly discarded
   — worse than the walk it replaces.
2. **Caster ids are the wrong payload.** The receiver needs *coverage*, not identity. Store coverage and
   the "128 casters per tile" problem disappears — `shadow-cold` already is 16 slots × u8. Storing ids only
   helps if the silhouette test is deferred to the receiver, which reinstates the per-texel work.
3. **Priority eviction is unimplementable cheaply.** "The 8 highest priority per light" is a per-texel sort
   with no cheap GPU form, and it is lossy: a dropped caster is a missing shadow.

**Why PROJECTED-FAN RASTERISATION works instead.** A shadow IS a projected quad, so rasterising it *is* the
scatter: the GPU derives which destination texels are covered, with no fan-out to express. Same scene:
**189k instanced 5-triangle fans** (1479 × 8 casters × 16 lights) covering ~1M texels — three orders of
magnitude under the gather. This is exactly `design/shadows.md`'s model, proven in
`bin/shadow-projection-sandbox.html`.

**Two constraints to design around:**
- **Accumulation** — multiple casters per light per texel need MAX. Integer RTs cannot blend, so this wants
  4× `RGBA8` MRT (16 channels, one per light slot) with `gl.blendEquation(gl.MAX)`, not the packed `RGBA32UI`.
- **Clearing** — scatter cannot un-shadow incrementally; a moved light must clear its region and re-cast it.

**Which resolves the tier question from the other direction:** **gather suits COLD** (baked once, the pull
cost is paid a single time, no clear problem) and **rasterise suits HOT** (always-fresh, where clear+recast
per frame is precisely the intended behaviour and is cheap). Not either/or — the tiered design's split,
re-derived from cost.

**Ordering.** The cross-pad DDA ([I30](issues.md#i30)) stays first regardless: ~59% fewer fetches, no
restructuring, verifiable against the existing corridor↔brute identity check, and it makes the cold gather
cheaper whatever happens to hot.

### F11a — can scatter update ONE light incrementally? No — and that is not a regression (2026-07-26)
User challenge: with MAX accumulation you cannot subtract a contributor, so moving 1 of 64 lights seems
to force re-casting all 64 over the tiles it dirtied.

**Storage is not the problem.** `shadow-cold` is 16 SLOTS, slot `i` = presence slot `i` = one light; MAX
accumulates across **casters within one light's slot**, never across lights.

**The problem is that slot→light is PER TILE.** Presence is built per tile, so light L sits in slot 3 on
one tile and slot 11 on the next. Therefore:
- **Writing is isolated and fine.** A fan reads its DESTINATION tile's presence, finds L's slot, and emits
  a 16-wide one-hot × coverage across the 4 `RGBA8` targets. MAX-blending 0 into the other 15 channels is
  a no-op, so only L's channel changes.
- **Clearing is NOT expressible.** Zeroing only L's channel needs a per-FRAGMENT channel mask; `colorMask`
  is per-draw. And it cannot go through the blend either — MAX-with-0 is exactly the no-op that made the
  write safe.

**So the user is right:** a moved light ⇒ clear + re-cast **all** lights present over the affected region.
The cause is the per-tile slot indirection, not MAX itself.

**But it is the SAME granularity we already run.** `buildDirty` marks tiles and `GATHER_FRAG` re-walks all
16 slots on every dirty tile — nothing today re-gathers a single light either. Per dirty region, all lights:
gather = 256 texels/tile × 16 lights × ~660 fetches; rasterise = 16 × 8 = **128 fans**. Same unit, far
cheaper. Same for a moved CASTER: clear each affected light's channel over its shadow region and re-cast the
others there — bounded by the shadow region, and identical to today's cascade.

**Escape hatch, deliberately declined:** a GLOBAL light→channel assignment would make `colorMask` per-light
clears trivial, but converts the per-tile 16-light cap into a GLOBAL 16-light cap. The per-tile slot
indirection is precisely what buys 64+ lights, so it is also precisely what forbids per-light clearing.
The trade is intentional, not accidental.
