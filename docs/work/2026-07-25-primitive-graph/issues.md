# Primitive graph — issues

_Problems found reviewing the design + how they resolved. Chronological append.
**I1–I7, I10, I11 RESOLVED** by the user's 2026-07-25 revisions (parent ids + CPU-stamped resolved
position, presence split, rotation semantics, fixed 8-px commands, `id = 0` sentinel, `radius` restored).
**Open: I8, I9, I12, I13 ⚠, I14.**_

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
disagrees with the active def. **Revised placement (2026-07-25):** `inherit_rotation` lives on
`definition_data` (RED, 1 bit) — all objects of a kind behave alike, so the choice is per-*kind*, not
per-object — plus one bit on `prim_data` (GREEN) for a prim that should act independently. Inheritance is
**one step** (child takes its parent's rotation), not a chain walk; the leaves carry no inherit bit and
the GPU simply uses the **bottom-most** rotation, already reconciled. `layer` needs no precedence either —
**one object per layer** means a prim's pieces occupy distinct layers, and `prim_data` carries no layer.

## I5 — `u3` counts cannot express 8 groups ✅ DISSOLVED (2026-07-25)
The counts scheme is gone. **Fixed 8-px commands** (opcode + set + 7 ids + 7 payloads) give 56
record-writes per 64-px row with no count field, no width problem, and no group padding.

## I6 — The opcode loses its home ✅ DISSOLVED (2026-07-25)
The opcode is now **byte 0 of every command** and *defines* the rest of the command's layout —
a stronger extension point than the old reserved field. `0x01` = write-data; future 8-px operations
(presence writes, bulk clears) take their own opcodes.

## I7 — 8-record write amplification ✅ RESOLVED — `id = 0` SENTINEL (2026-07-25)
A command writes ≤7 records to **one set**, and a partial command **pads its unused id slots with 0**
(user). `id = 0` is a global sentinel, so the scatter discards those points — emit a degenerate
`gl_Position` (outside clip space) so the point is clipped and no write lands. This keeps the trivial
index map (record `p` → command `p/7`, slot `p%7`) with no per-point bookkeeping. Cost: **one burned
entry per set** ([I14](#i14)).

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

## I10 — `light_data` lost `emitter_radius` ✅ RESOLVED — RESTORED (2026-07-25)
**Problem.** The first spec had `BLUE: u12 reach | u8 radius | u12 reserved`; the revision has
`ALPHA: u12 reach | u20 reserved` — **`radius` is gone**. It is load-bearing: `emitter_radius` drives the
entire **16-tap area-light penumbra** (the delivered [penumbra](../2026-07-23-penumbra/README.md)
stream) — `casterCover(...)` takes `emitter` and falls back to a **hard quad** when it's < 0.5
([`shadowGather.ts:225,257`](../../../client/webgl/src/game/viewport/shadowGather.ts)). Dropping it
silently turns every soft shadow hard.
**Resolved.** User confirmed the oversight; `u8 radius` is back. Final:
`ALPHA: u12 reach | u8 radius | u8 resolved_zone | u4 reserved`.

## I11 — Where does the parent-chain walk happen? ✅ RESOLVED — CPU STAMPS IT (2026-07-25)
**Problem.** `parent_id` means the GPU *can* resolve a leaf's absolute position, but the gather's light
loop runs **per texel per light** — doing a chain walk there multiplies the hottest loop in the renderer
(the same cost that makes a fine-res gather unaffordable). The CPU, by contrast, **must** compute each
light's world position anyway to decide which tiles it reaches when building `light_presence`.
**Resolved (user).** The **CPU stamps the resolved position** and the leaf spends its whole RED lane on
it: `u16 parent_id | u8 resolved_tile | u8 resolved_unit`. Resolution starts at the root's tile/unit and
applies each child prim's offsets down the chain to the leaf. The authored `tile_offset`/`unit_offset`
stay in GREEN as the durable relative placement; RED is the derived cache the GPU reads. **The GPU never
walks the graph in the hot loop.** (Follow-on: `resolved_zone` is also required — [I13](#i13).)

## I12 — Two resolvers risk drift (2026-07-25) — OPEN, low-stakes
With both CPU and GPU able to resolve the graph, they must agree **exactly** (the class of bug that bit
us twice on zoom). **Solution:** name **one** authority per consumer — CPU-resolved for presence,
bucketing, and the baked position; GPU walk only for reads with no CPU-side equivalent — and state it in
VARIABLES so nobody adds a second path later. Also cap and document `MAX_DEPTH` for any GPU walk.

## I13 — ⚠ `resolved_tile` + `resolved_unit` alone are ambiguous beyond 8 tiles of reach (2026-07-25) — OPEN
**Problem.** The leaf's resolved position is `u8 resolved_tile | u8 resolved_unit` — tile is `x:4|y:4`,
i.e. the **in-zone** tile (0–15). The gather needs the light's position in the same frame as the sample
point: it computes `toL = Lxy - P` ([`shadowGather.ts:573`](../../../client/webgl/src/game/viewport/shadowGather.ts))
for falloff/N·L, and marches the corridor from the light. A fragment knows its own absolute world tile,
so it can reconstruct the light by picking the **congruent tile nearest itself** — but that has a period
of **16 tiles**, so it is only unambiguous when the light is within **8 tiles**. `LIGHT_REACH` is
**12 tiles** today, and the `u12 reach` field permits up to 255 — so a light 12 tiles north reconstructs
as 4 tiles south. This is precisely the position-ambiguity class that caused the two zoom regressions.
**Solution.** Also stamp **`u8 resolved_zone`** (in-region zone, `x:4|y:4`). Zone+tile+unit gives a
period of **256 tiles** — unambiguous for any reach under 128 tiles, comfortably covering the field's
range; region stays inferred by nearest-congruent (exactly what the region-torus fold already does).
Room exists with no layout pressure: `light_data` ALPHA (`u12 reach | u8 radius | u8 resolved_zone |
u4 reserved`) and `billboard_data` ALPHA (`u8 resolved_zone | u24 reserved`). **Applied in the README;
needs your confirmation.**

## I14 — `id = 0` cannot be a global sentinel: tile-keyed sets address by fold ✅ RESOLVED (2026-07-25)
**Problem.** The first plan made `id = 0` a global sentinel so partial commands could pad with zeros
("burns one entry per set"). That is safe for **record** sets (defs/prims/billboards/lights — allocators
just start at 1) but **not** for the three **tile-keyed** sets (`light_presence_lo/_hi`,
`billboard_presence`), whose in-set id is not an allocated counter but a **computed fold**:
`foldTile(0,0) = 0`, and the fold spans **0..65535 exhaustively** over a region (verified), so there is
**no spare id to bias into** (`fold + 1` overflows u16). Since a command carries **one** `set` shared by
all 7 slots, a partial command targeting a tile-keyed set would pad with `id = 0`; discarding `id == 0`
would make that tile **permanently unwritable** — exactly one tile per region-torus silently holding no
lights and casting no shadows.
**Resolved (user).** Steal bits from the header instead: **`R = u8 operation | u5 set | u3 count |
u16 id₀`**. `count` states how many of the 7 ids are live, so **no sentinel is needed anywhere** and
`id = 0` stays fully usable. `set` narrows to **u5** (32 sets; 16 in use) — widen out of `operation`
later if needed. The scatter issues 7 points per command and drops `slot ≥ count` (off-clip position),
so the trivial `p/7`, `p%7` index map survives. Nothing is burned and no allocator needs to change.

## I15 — Vite HMR gives a FALSE failure when the command format changes (2026-07-25) — verification gotcha
**Problem.** After swapping the transport to v3, the scene rendered as flat blocks with no prims — but
there were **no console errors**, and the logic was correct on inspection. Cause: **HMR had replaced the
module while the live `ColdShadowData` instance (and its already-compiled scatter `Program`) survived**,
so a NEW `flush()` was feeding an OLD shader — exactly a format mismatch. A full navigate to a
cache-busted URL rendered correctly and the data confirmed flowing (525 billboards, 3 lights, last flush
→ set 2).
**Rule for this stream:** any command-format or record-layout change must be verified on a **fresh page
load**, never on an HMR update. A silent flat/garbled frame with a clean console is the signature.
