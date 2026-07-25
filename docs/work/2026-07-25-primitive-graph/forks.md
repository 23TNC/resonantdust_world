# Primitive graph — forks

_Decision points. F1 is the architectural crux — it determines the answers to
[I1](issues.md#i1), [I2](issues.md#i2), and [I3](issues.md#i3)._

## F1 — Is the graph resolved on the CPU, or walked on the GPU? (2026-07-25, OPEN — the crux)
The design gives prims children with relative placement and inherited flags. **Where does that get
resolved?** Everything else follows from this answer.

- **(a) CPU-RESOLVED / flat GPU view (lean).** The prim graph is the **authoring + update** structure.
  Each build, the CPU walks placed roots, resolves every leaf's **absolute position** and **effective
  `hot_cold`/`cast_shadows`/`rotation`**, and writes those into the leaf records; `light_presence` and
  `caster_buckets` are filled with **resolved leaves**. The GPU's read path stays exactly what it is
  today — bucket → record → def, one hop, no traversal.
  - *For:* hot-loop cost unchanged ([I3](issues.md#i3)); no GPU recursion or depth cap; inherited flags
    are free (stamped at resolve); zoom-safety preserved (contiguous/index-addressed, no new world-coord
    reads — the recurring drift class stays closed); leaves need no upward pointer ([I1](issues.md#i1)).
  - *Against:* moving a carrier re-writes every descendant leaf (a scene-graph cost — but that IS the
    semantic; and compare-write means an unmoved subtree emits no command); leaf records must hold a
    full absolute position, so the reserved bits get spent ([I1](issues.md#i1) — the room exists).
- **(b) GPU-WALKED.** Records stay relative; shaders walk carrier→child (and up, via an added
  `carrier_id`) at read time.
  - *For:* one source of truth in the texture; a carrier move costs exactly one texel.
  - *Against:* unbounded-depth traversal in the **hottest loop in the renderer**; needs a hard depth cap
    + an upward pointer; inherited flags require walking the ancestor chain **per texel per light**;
    directly multiplies the corridor cost that [shadow-edge-refine](../2026-07-24-shadow-edge-refine/README.md)
    established as the bottleneck.
- **(c) Hybrid.** CPU resolves *position + inherited flags* (the hot-path inputs) but the texture retains
  the graph for updates/gameplay. This is (a) plus keeping `prim_data` authoritative in-texture — likely
  where we actually land, since `prim_data` exists in the design regardless.

**Lean: (a)/(c).** The GPU should see a flat leaf view; the graph lives in `prim_data` for updates and
CPU resolution. **This does not weaken the model** — composition, inheritance, and relative placement all
work exactly as specified; only *where they're evaluated* changes.

## F2 — What do `caster_buckets` slots hold? (2026-07-25, OPEN — follows F1)
Under F1-(a): **resolved casting billboards** (8/tile), keeping today's cost. Under F1-(b): **carrier
prims** (8/tile), each fanning out to ≤4 children. The user's phrasing was "8 prims per tile"; under the
lean this becomes 8 *casting leaves* per tile — same slot count, leaf semantics ([I3](issues.md#i3)).

## F3 — Child offset encoding (2026-07-25, OPEN — must be settled at P0)
Unsigned as literally specified makes −x/−y unreachable ([I2](issues.md#i2)). **Lean: bias-8** on each
nibble (`value − 8` → −8..+7) for both `tile_offset` and `unit_offset`. Alternative: two's-complement u4.
Either works; it must be *chosen and written into VARIABLES* before any writer/reader is built.

## F4 — Header count width (2026-07-25, OPEN, mechanical)
`u3` can't express the 8th group ([I5](issues.md#i5)). **Lean: `u4` counts** (16 × u4 = u64 counts + u64
reserved, still one header px), which also leaves obvious room for the opcode ([I6](issues.md#i6)).

## F5 — Does `prim_data` need a type/identity field? (2026-07-25, OPEN, low-stakes)
`prim_data` is pure structure — position + flags + 4 carried refs. Nothing says "this is a pawn". Fine for
rendering (presentation is entirely in the leaves), but gameplay/picking//debug may want an object id.
`G` has `u4 reserved` and `definition_data` has a `u4 type`. **Lean: leave it out for now** — add when a
consumer actually needs it, rather than speculatively spending bits.

## F6 — Dirty entry points, inherited (2026-07-25, carried over from light-prims)
The [light-prims](../2026-07-24-light-prims/todo.md) P3 design survives and applies to the graph:
`markBillboardDirty` / `markLightDirty` (+ a `markPrimDirty` for a carrier, which cascades to its
subtree), all funnelling into `pendingRects → buildDirty`, with cold/hot routing generalised.
**Note the new wrinkle:** with inheritance, a carrier's `hot_cold` flip re-classes its whole subtree, so
the dirty cascade must walk children — which F1-(a) already does at resolve time.
