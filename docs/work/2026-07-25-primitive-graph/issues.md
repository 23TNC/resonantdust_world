# Primitive graph — issues

_Problems found reviewing the design + how they resolved. Chronological append.
**I1–I7 RESOLVED by the user's 2026-07-25 revision** (parent ids, presence split, rotation semantics,
fixed 8-px commands). **I10–I12 are the new open items.**_

## I1 — A carried record cannot find its carrier ✅ RESOLVED (2026-07-25)
**Was.** Leaves held offsets only, links ran downward only, so a light id from presence could not be
located and the inherited flags (ancestor properties) were unreachable.
**Resolved.** `u16 parent_id` added to `light_data` + `billboard_data` (RED), and `prim_data` gained a
`child` bit that reinterprets RED's top half as `u16 parent_id`. The graph is now navigable **upward**,
so either side can resolve — the CPU normally does (it must anyway, to build presence), and the GPU
*can* if a shader needs to. See [I11](#i11) for where the walk should actually happen.

## I2 — Child offsets unsigned ✅ RESOLVED (2026-07-25)
**Resolved: bias-8** per nibble (−8..+7 tiles / units) — user agreed. Applies to every offset field
(`tile_offset`, `unit_offset`) on child prims, billboards, and lights. To be written into VARIABLES at P0.

## I3 — Hot-loop fan-out through carriers ✅ RESOLVED (2026-07-25)
**Resolved** by bucketing **leaves, not carriers**: `light_presence` holds `light_data` ids and
**`billboard_presence`** (renamed from the caster buckets) holds `billboard_data` ids. The user's
reasoning is the decisive one: bucketing carriers makes the per-tile billboard count **unbounded**
(a carrier fans out to ≤4, recursively). Leaf bucketing keeps the corridor walk exactly as cheap as
today, at 8 slots/tile.

## I4 — `rotation`/`layer` precedence ✅ RESOLVED (2026-07-25)
**Resolved, and the question was mis-framed.** There is no render-time rotation precedence: **the shader
always uses the definition's rotation**, because the definition *is* the active sprite. Stored `rotation`
is the **desired facing** — a CPU-side reconciliation signal that triggers a *definition swap* when it
disagrees with the active def. `parent_rotation` (1 bit, from RED's reserved) opts a piece into
inheriting the carrier's facing; cleared, it stays independent (a tool aimed south under a pawn facing
east). `layer` needs no precedence either — **one object per layer** means a prim's pieces occupy
distinct layers, and `prim_data` carries no layer at all.

## I5 — `u3` counts cannot express 8 groups ✅ DISSOLVED (2026-07-25)
The counts scheme is gone. **Fixed 8-px commands** (opcode + set + 7 ids + 7 payloads) give 56
record-writes per 64-px row with no count field, no width problem, and no group padding.

## I6 — The opcode loses its home ✅ DISSOLVED (2026-07-25)
The opcode is now **byte 0 of every command** and *defines* the rest of the command's layout —
a stronger extension point than the old reserved field. `0x01` = write-data; future 8-px operations
(presence writes, bulk clears) take their own opcodes.

## I7 — 8-record write amplification ✅ MOSTLY DISSOLVED (2026-07-25)
A command writes ≤7 records to **one set**. A partial command just issues fewer scatter points, so
there's no padding requirement. *Implementation note:* the trivial index map (record `p` → command
`p/7`, slot `p%7`) assumes full commands, so either (a) pad a partial command by **repeating an id +
payload** (the scatter does absolute, replay-idempotent writes — a duplicate is a harmless second write
of identical data), or (b) upload the `(command, slot)` pair per point in the existing `aIndex`
attribute. **Lean (b)** — zero waste, and `scatterGeo` already carries a per-point integer attribute.

## I8 — Rename reconciliation (2026-07-24 → 2026-07-25) — OPEN, mechanical
Yesterday's `prim_*`→`billboard_*` rename folded `prim_data` into `billboard_data`. This design
**splits** them again. Land as: `prim_data` (node) / `billboard_data` (leaf, NEW) / `light_data` (leaf) /
`definition_data` (revert from `billboard_definition_data` — it now carries a `u4 type` and serves more
than billboards). The 2026-07-24 *vocabulary* still holds (primitive = umbrella, billboard = a
presentation); only the band split changed. Also rename the caster buckets → `billboard_presence`.

## I9 — Subtree lifetime: freeing a carrier must free what it carries — OPEN
Freeing a pawn must free its hands, their billboards, and the torch's light — a **recursive** free — and
a carried record must not be reclaimed while its carrier lives. `parent_id` now makes the child→parent
direction checkable, which helps. **Solution:** free by **reachability from placed roots**
(mark-from-roots, then sweep), replacing the flat "seen this frame" sweep in `freeBillboardsExcept`.
Also: `z` (u8) + `z_offset` (u8) can sum past u8 — clamp and document.

## I10 — ⚠ `light_data` lost `emitter_radius` in the revision (2026-07-25) — OPEN, REGRESSION
**Problem.** The first spec had `BLUE: u12 reach | u8 radius | u12 reserved`; the revision has
`ALPHA: u12 reach | u20 reserved` — **`radius` is gone**. It is load-bearing: `emitter_radius` drives the
entire **16-tap area-light penumbra** (the delivered [penumbra](../2026-07-23-penumbra/README.md)
stream) — `casterCover(...)` takes `emitter` and falls back to a **hard quad** when it's < 0.5
([`shadowGather.ts:225,257`](../../../client/webgl/src/game/viewport/shadowGather.ts)). Dropping it
silently turns every soft shadow hard.
**Solution (assumed an oversight, restored in the README).** `ALPHA: u12 reach | u8 radius | u12 reserved`
— there is ample room (u20 was reserved). Flagged for confirmation in [blockers.md#b2](blockers.md#b2).

## I11 — Where does the parent-chain walk actually happen? (2026-07-25) — OPEN
**Problem.** `parent_id` means the GPU *can* resolve a leaf's absolute position, but the gather's light
loop runs **per texel per light** — doing a chain walk there multiplies the hottest loop in the renderer
(the same cost that makes a fine-res gather unaffordable). The CPU, by contrast, **must** compute each
light's world position anyway to decide which tiles it reaches when building `light_presence`.
**Candidate solutions.** (1) **CPU stamps the resolved absolute position** into the leaf, so the shader
reads it directly (needs a u32 home — `RED`'s reserved u15 + a repurpose, or treat the offset fields as
*resolved* tile/unit post-resolution with `region|zone` in RED's reserved). (2) GPU walks up, with a
**constant-bounded** loop + `break` (never a body-modified loop condition — the known GLSL foot-gun) and
a documented `MAX_DEPTH`. **Lean: (1)** for the hot paths, keeping (2) available for one-off reads.
Needs a bit-home decision → [blockers.md#b2](blockers.md#b2).

## I12 — Two resolvers risk drift (2026-07-25) — OPEN, low-stakes
With both CPU and GPU able to resolve the graph, they must agree **exactly** (the class of bug that bit
us twice on zoom). **Solution:** name **one** authority per consumer — CPU-resolved for presence,
bucketing, and the baked position; GPU walk only for reads with no CPU-side equivalent — and state it in
VARIABLES so nobody adds a second path later. Also cap and document `MAX_DEPTH` for any GPU walk.
