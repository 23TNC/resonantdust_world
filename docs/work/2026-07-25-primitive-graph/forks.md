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
as `shadow-projection`.**

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

### F11b — WIDE ADDITIVE LIGHTMAP: drop per-light slots entirely (2026-07-26) — user proposal, ADOPTED
User proposal: hold the lightmap **wider than u8** so every light's contribution can simply be **added**,
making an update **invertible** (subtract the old, add the new) instead of clear-and-recast. This dissolves
[F11a](#f11a-can-scatter-update-one-light-incrementally-no--and-that-is-not-a-regression-2026-07-26) at the
root: there are no per-light slots to clear, so the per-tile slot indirection stops being a constraint.

**Three corrections, all measured on the live context (RTX 2080 Ti / ANGLE D3D11).**

1. **The format must be `RGBA32F`, NOT `RGBA32UI`.** ES 3.0 §15.1.4 skips blending for integer formats, and
   it does so **silently** — the FBO reports complete and `getError()` stays 0. Measured: constant
   `(10,20,30,40)` drawn 5× with `blendFunc(ONE,ONE)` →
   - `RGBA32UI` → `[10,20,30,40]` (last write; no accumulation, **no error raised**)
   - `RGBA32F`  → `[50,100,150,200]` (exact)
   `EXT_color_buffer_float` + `EXT_float_blend` are both present; the second is required and is the one
   that is easy to forget, since without it float blending is silently dropped too.
2. **Contributions MUST be quantised**, and this is exactly the hedge the user asked for. FP32 represents
   every integer below 2^24 exactly, so adding and subtracting integer-valued contributions is bit-exact
   **in any order**. Measured add-all-then-subtract-in-reverse-order residuals:
   - quantised (each light casts an integer 0..255, 64 lights) → peak 7,776, residual **exactly 0**
   - quantised, 4096 contributions → peak 522,240, residual **exactly 0**
   - raw float (`colour × falloff × N·L`, unquantised) → peak 32.08, residual **1.8e-7** (drifts)
   So the user's own "each light casts RGBA8" framing is what makes it exact.

   **The bound is PER-TEXEL OVERLAP DEPTH, not a light count** — the accumulator is per texel, so what
   counts against 2^24 is how many lights illuminate the SAME texel at once. The world may hold as many
   lights as the u16 id allows; only co-illumination is charged. Presence admits 16/tile today, ~4,000x
   under. Verified boundaries (add-all then subtract-all, residual):
   - 65,793 x 255 → peak 16,777,215 = 2^24-1, residual **0** (the user's figure, exactly on the boundary)
   - 80,000 x 255 → peak 20,414,204, residual **-2** (breaks; past 2^24 float spacing is 2, so odd
     increments round — the failure is a small bias, not a collapse)

   The axis that actually shrinks the ceiling is **per-light peak magnitude, not count**: the bound is
   2^24 / (max quantised contribution). u8 → 65,793; HDR intensity 4x (1020) → 16,448 (verified exact);
   quantised to 1/4096 for smoother falloff → 4,096 (verified exact). So the real design choice is dynamic
   range and quantisation precision per light, and even the most expensive combination leaves thousands of
   overlapping lights. u16 light ids bind first regardless.
3. **Additivity holds across LIGHTS, not across CASTERS.** Occlusion is a **union**, not a sum: two casters
   shadowing the same texel must MAX, or they over-subtract past black. So each light still needs its own
   shadow resolve before its contribution is added — which is precisely what the existing per-light N·L
   bake already does. The wide accumulator replaces the per-light **storage**, not the per-light **resolve**.

**Subtraction mechanism.** One blend mode does both directions: `FUNC_ADD` + `blendFunc(ONE,ONE)`, and a
**negative fragment output** subtracts. No second pipeline state, no `FUNC_REVERSE_SUBTRACT`. Measured:
8 quantised lights added → `[136,264,520,2040]`; one negated → `[119,231,455,1785]`; all 8 subtracted →
`[0,0,0,0]`, `err 0`.

**The real risk is reproducing the OLD contribution, not arithmetic** — to subtract light L exactly you must
re-cast it with L's old parameters AND the old caster state. If a caster moved in the same frame, the old
contribution is no longer reproducible and light leaks behind. Resolved by the ping-pong below.

### F11b.1 — PING-PONG STATE + ONE DIFFERENTIAL PASS (2026-07-26) — user design, ADOPTED
The user's refinement, and it is a strict upgrade. F11b originally leaned on an ORDERING trick: subtract
before `coldData.flush()` so the data texture still held last frame's state. That works but is fragile — the
correctness of the subtract depends on pass order, and getting it wrong fails silently. **Ping-pong the state
instead:** keep last frame's and this frame's data textures both live, so both contributions are addressable
at any point in the frame and no ordering can be wrong. Cost: a second 1024² RGBA32UI, **16 MB**.

**That collapses the two passes into one.** With both states bound, a single fragment computes the old and
new contribution and emits **`new − old`** under one `blendFunc(ONE,ONE)`. Better three ways:
- **Unchanged texels emit exactly 0** — both terms are quantised integers, so the difference is an exact
  integer and "nothing moved here" is a hard zero, not an epsilon. A correctness check for free.
- **The region is the union, not the sum.** A light that moved one tile has ~90% overlap between old and new
  reach; two passes pay both rects, the differential pass pays their union once.
- **The lightmap is never transiently wrong** — no window where light is subtracted but not yet re-added.

**Frame loop.** Build the dirty-light union → one differential pass per dirty light over its old∪new region.
Nothing else touches the accumulator.

**HOT/COLD DISSOLVES.** Whatever is written settles immediately and is never recalculated, so the split (which
existed so a hot light could not pollute the cold bake) has nothing left to do. One accumulator.

**Dirty-light union.** A moved prim dirties lights at **both ends** — it stops casting where it left and
starts casting where it arrived, and if it moved far those are disjoint light sets:
`dirtyLights = ⋃ presence(t) for t in (oldTiles ∪ newTiles)`.

**PRESENCE MUST PING-PONG TOO** — the trap in this design. A light's old contribution to a tile depends on
whether it was in that tile's **top-16**. If L was slot 9 on tile T last frame and got pushed out this frame
by a nearer light, L's contribution to T must be subtracted — knowable only from T's OLD presence. So
`light_presence_lo/hi` + `billboard_presence` ping-pong with the rest, and presence churn is itself a dirty
event over `oldPresence(T) ∪ newPresence(T)`.

**The recast must GATE ON per-tile presence.** Rasterising L over its full reach crosses tiles where L is not
in the top-16; those texels must contribute nothing or the cap does not actually hold. The differential shader
reads the destination tile's presence (old for the old term, new for the new term) and gates each term on
membership. Cheap — presence is already being read — but load-bearing, not an optimisation.

**The 16/tile cap is RETAINED, and its justification changes.** It was forced by storage (16 slots × 8 bits =
128 bits = one RGBA32UI). Nothing stores per-light anything now, so 16 is purely a **work bound**: how much
recasting one dirty tile may cost. Same number, different reason — and it becomes a **tunable knob** (raise to
24/32 by deciding the recast is affordable) with no format change and no repacking. It also keeps a user from
stacking torches into an unbounded recast. This CORRECTS F11b's original claim that presence demotes to a pure
culling optimisation.

**The big win — dirty lights and dirty prims SHARE recalculation.** Today cost scales as
*dirty tiles × lights per tile*: eight wolves under one torch is eight times the work. Here the unit of work is
"light L, once", so **any number of movers inside L's reach collapses into a single recast of L**. Crowd density
stops costing anything — a superlinear→linear collapse in exactly the scenes that fall over now, which is why
the 32-mover measurement (34 fps) is not predictive of this scheme.

**Steady-state only — two exceptions, both pre-existing costs.** (a) **Scroll/pan**: the lightmap is
world-ordered, so a newly-exposed band of texels now represents different world tiles and must be zeroed and
re-accumulated from scratch (all lights, not a differential) — the same work the cold bake pays on scroll
today. (b) **Zoom**: whole buffer invalid, full rebuild. Also as today.

**The one genuine cost of dissolving hot/cold.** `shadow-cold` persistently stores per-slot coverage today, so
the corridor walk is paid **once at bake**. With on-demand recasts and no persistent shadow store, every recast
pays the walk fresh — it moves from one-time to recurring. Bounded by the dirty-light union, so not fatal, but
it makes **the DDA ([I30](issues.md#i30)) more important, not less**: cutting ~59% of the walk's fetches was a
nice-to-have when the walk ran once; it is on the critical path when the walk runs per recast. Ordering
unchanged — DDA first.

**Deferred optimisation, deliberately (user: "they get complex so for our first pass lets just mark the lights
dirty").** A *moved light* genuinely changed everywhere in its reach → full region. A *static light with a
moved caster* only changed where that caster's shadow moved → the pass could scissor to that caster's
old∪new shadow region, far smaller than the light's reach. Cheap to add later: it is a scissor rect on the
same single pass, no structural change. That is where the second-pass win lives.

**Costs — CORRECTED, and they gate the whole fork.** The earlier figure here (24 MB, from a "fixed 24×16-tile
window") was read before the tile window had sized and is wrong by ~20x. Measured at `zoom=0.25`: the window
is **124×60 tiles** and the lightmap is **64 texels per tile PER AXIS** (one texel per world px), so the RT is
**7936×3840 = 30.5M texels**:

| | `RGBA8` (today) | `RGBA32F` |
|---|---|---|
| per tier | 116 MB | 465 MB |
| both tiers | 233 MB | — |
| single accumulator (hot/cold collapsed) | — | **465 MB** |

465 MB is not viable as a baseline, and the fix now has its own stream:
**[2026-07-26-textile-slot](../2026-07-26-textile-slot/README.md)** sizes every map in TILES on a fixed 24×16
slot grid, which puts the single collapsed `RGBA32F` accumulator at **96 MiB, constant at every zoom** — less
than the 176 MB of `RGBA8` lightmap it replaces. **That stream is a prerequisite for F11b, not an
optimisation**: land it first and `RGBA32F` costs less memory than the scheme it replaces. Plus 16 MB for the
ping-pong data texture. The final blit gains a divide by the quantisation scale and a clamp/tonemap.

**Self-heal, required.** Bookkeeping leaks are the residual risk: a full rebuild on the scroll/zoom paths that
already re-bake, plus a debug **rebuild-and-diff** assertion that re-accumulates from scratch and reports any
disagreeing texel — trivially expressible now there is one buffer. Converts a silent leak into a caught one.

### F11c — keep a persistent shadow cache for static-light × static-caster pairs? DEFERRED
F11b.1 makes every recast re-walk the corridor. A persistent cache of resolved coverage for pairs where
neither light nor caster moved would restore the one-time cost — but that is a cold tier under another name,
and it reintroduces exactly the invalidation bookkeeping this fork deleted. Not for the first pass. Revisit
only if the DDA leaves the recast walk measurably hot.
