# Primitive graph — issues

_Problems found reviewing the 2026-07-25 design + candidate solutions. Chronological append.
I1–I3 are **structural** (must be answered before P1 writes code); I5–I9 are mechanical._

## I1 — A carried record cannot find its carrier: no absolute position, no upward link (2026-07-25) ⚠ CRITICAL
**Problem.** `light_data` and `billboard_data` hold **offsets only** (`tile_offset`/`unit_offset`/
`z_offset`) — no region/zone, no absolute position. Links run **downward only** (`prim.set/id → child`);
nothing points **up**. But the GPU enters through the tile-keyed maps: `light_presence` gives a fragment a
**light id**, `caster_buckets` gives it a **prim id**. Given a light id, the shader fetches `light_data`,
finds only offsets, and **cannot determine where the light is** — the carrier is unreachable and the
ancestor chain unknown. The same applies to the inherited `hot_cold` / `cast_shadows` (both are defined by
"topmost wins", i.e. an ancestor property).
**Candidate solutions.**
1. **CPU resolves the graph and writes leaves absolutely** (pairs with [F1](forks.md#f1)-a). At build
   time the CPU walks the tree, computes each leaf's absolute `region/zone/tile/unit` + effective
   `hot_cold`/`cast_shadows`, and stores *that* in the leaf record. Requires the leaf to hold a full
   position: `light_data` has `u12`+`u32` reserved, `billboard_data` has `u32`+`u32` reserved, so **the
   room exists** (needs `u16 region|zone` alongside the existing tile/unit fields — the offset fields
   become *resolved* position at bake time). GPU hot paths never walk the graph.
2. **Add `u16 carrier_id` to the leaf** (upward pointer) and let the shader hop up. One hop is cheap, but
   *multi-level* nesting (pawn → hand → torch → light) needs a chain walk in the hot loop, and inherited
   flags need the full chain.
3. **Presence/buckets store `(prim_id, slot)` instead of the leaf id**, so entry is always via a carrier.
   Costs presence slot bits and still needs the chain for deeper nesting.
**Lean: (1).** It keeps the GPU's read path flat (exactly as fast as today, zoom-safe by construction)
and makes the graph a *CPU authoring/update* structure. See [F1](forks.md#f1) — this is the central fork.

## I2 — Child offsets appear UNSIGNED → children can only go right/down (2026-07-25) ⚠ CRITICAL
**Problem.** `tile_offset` / `unit_offset` are `u8 = x:4|y:4`. Read as unsigned, a child can only be
placed at **+x/+y** from its carrier. But a pawn's **left** hand, a torch held to the left, and a light
centred on a billboard all need **negative** offsets. As specified, half the plane is unreachable.
**Candidate solutions.** (a) **Signed nibbles** (two's complement `u4` → −8..+7 tiles / −8..+7 units);
(b) **biased** (`stored − 8`), same range, simpler to reason about; (c) widen the field (costs bits we
don't obviously have). **Lean: (b) bias-8** — identical cost, no sign-extension bugs in GLSL, and −8..+7
tiles is ample for limbs/held items. **Must be decided at P0** — it's a layout semantic.

## I3 — Hot-loop cost: buckets → prim → 4 children → def is 2 extra hops and up to 4× the tests (2026-07-25)
**Problem.** The corridor walk is **the** cost of the shadow pass (established in
[shadow-edge-refine](../2026-07-24-shadow-edge-refine/README.md)): per texel, per light, march the
segment reading `caster_buckets` and testing silhouettes. Today a bucket slot is a caster prim → def →
test. Under the graph, a bucket slot is a **carrier** → up to 4 `set/id` → billboard → def. That's 2 more
indirections *and* up to **8 prims × 4 billboards = 32 silhouette tests/tile** where today it's 7.
**Candidate solutions.** (1) **Bucket the resolved leaf billboards, not the carriers** — the CPU flattens
(I1-1), so `caster_buckets` holds 8 *billboard* ids exactly as it holds 7 caster ids today; cost is
unchanged and the "8 prims/tile" figure becomes "8 casting billboards/tile". (2) Keep carriers in buckets
and accept the fan-out (needs a real perf test before committing). **Lean: (1).** Note this slightly
re-reads the user's "8 prims per tile" — same slot count, leaf semantics.

## I4 — `rotation` and `layer` appear at three levels with no precedence rule (2026-07-25)
**Problem.** `rotation` (u2) is in `prim_data`, in `billboard_data`/`light_data`, **and** in
`definition_data`; `layer` (u4) is in both the leaf and the def. `hot_cold`/`cast_shadows` have explicit
rules ("topmost wins"); these don't. Unresolved, each reader invents its own.
**Also a semantic question:** is a carrier's rotation **inherited** (a pawn turning west re-faces its
hands) or **composed geometrically** (children orbit the parent)? For a billboard world the first is
almost certainly right — rotation *selects the facing frame*, it does not rotate a coordinate frame — but
if children orbit, the left/right hands must **swap** on an E↔W flip, which is a placement concern.
**Candidate.** State it at P0 alongside the other inheritance rules: `rotation` inherits downward as a
*facing* (leaf `rotation` = override when non-zero, else carrier's), `layer` is leaf-then-def fallback.

## I5 — `u3` counts cannot express a full 8 groups (2026-07-25)
**Problem.** Counts are in groups of 8 and a fill has 64 data px = **8 groups**, but `u3` maxes at **7**
→ a single set can only fill 56 records/fill, and the last group of a full fill is unaddressable.
**Solution.** Use **`u4` counts** — 16 sets × u4 = **u64 counts + u64 reserved**, still one px, and 0..8
groups is expressible (with headroom). Costs nothing. **Lean: u4.**

## I6 — The `u8 opcode` loses its home (2026-07-25)
**Problem.** Today the header carries `u8 opcode` in A bits 0–7 (`0 = write-data`; future opcodes were
reserved for presence/other maps). The new header is specified as "u48 counts + u80 reserved" with no
opcode named.
**Solution.** Park the opcode explicitly in the reserved bits (e.g. A bits 0–7, as today) so the future
opcode path survives. Trivial, but must be *written down* or it will be silently dropped.

## I7 — 8-record granularity = write amplification on small updates (2026-07-25)
**Problem.** One moved prim = 1 record, but the fill pads to a group of 8 → 8 data px + 1 id px + header
(~10 px) where v2.1 cost 2 px. ~5× on the common single-mover update.
**Analysis/solution.** Acceptable: the command texture is 64×64 = 4096 px (~56 fills of 73 px), and fills
are batched per frame. Pad by **repeating the same id + payload** — the scatter does absolute,
**replay-idempotent** writes, so a duplicate is a harmless second write of identical data. No sentinel id
needed. (Write it down so nobody invents an "empty" id and has it scatter to record 0.)

## I8 — The 2026-07-24 `prim_*`→`billboard_*` rename needs partial reconciliation (2026-07-25)
**Problem.** Yesterday's rename folded the old `prim_data` into `billboard_data` on the premise that
"prim always meant billboard". This design **splits** it: `prim_data` returns as the composition node
(position + carried ids) and `billboard_data` is a genuinely new leaf (def + offsets). Also
`billboard_definition_data` now carries a `u4 type` and serves more than billboards.
**Solution.** `prim_data` (node) / `billboard_data` (leaf) / `light_data` (leaf) /
`definition_data` (revert from `billboard_definition_data`). The *vocabulary* from yesterday still
holds — primitive is the umbrella, billboard is a presentation — only the band split changed.

## I9 — Subtree lifetime: freeing a carrier must free what it carries (2026-07-25)
**Problem.** The free-list (`billboardFreeList`, `freeBillboardsExcept`) frees flat records by "not seen
this frame". With carriers, freeing a pawn must free its hands, their billboards, and the torch's light —
a **recursive** free — and a carried record must not be reclaimed while its carrier lives.
**Solution.** Free by reachability from the placed roots (mark-from-roots, then sweep), or refcount by
carrier. Mark-from-roots matches the existing "seen this frame" sweep most closely. Also: `z` (u8) +
`z_offset` (u8) can sum past u8 — clamp and document.
