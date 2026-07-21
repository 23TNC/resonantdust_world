# Work — hot-shadows (hot lights + prims, work-item cast, per-light bitfield output)

_Opened 2026-07-21. Add the **hot (dynamic) tier** to the shadow cast on top of the delivered
[`cold-data-textures`](../cold-data-textures/README.md) cold tier: hot lights + hot prims passed by
**uniform** (same packed format as their cold texture counterparts), a **work-item** system that replaces the
per-light LUT run and bounds which lights act on which prims, per-frame **budgeting**, and a per-light
**bitfield** shadow output (`*-hot` / `*-cold` maps). Component: `client/webgl`. Reconciles with the
[`shadows`](../shadows/README.md) screen-hot→world-cold design (see [F3](forks.md#f3))._

## The idea (the user's design)

**Temporal tiering — recompute only what moves.** Cold data (static lights/casters) lives in
`RGBA32UI` **textures** (cold-data-textures) and changes rarely; **hot** data (movers) is passed by
**uniform**, in the **same packed layout**, so the shader decodes a light/prim identically whether it came
from a `texelFetch` or a uniform. Shadows land in two counterpart maps by freshness:

| light | prim | writes to | recompute |
|---|---|---|---|
| cold | cold | `*-cold` map | rarely (budgeted) |
| cold | hot | `*-hot` map | per frame |
| hot | hot | `*-hot` map | per frame |
| hot | cold | `*-hot` map | per frame |

Any hot participant → the `*-hot` map (rebuilt each frame). Only cold×cold → the `*-cold` map (persistent,
rebuilt in slices under a budget).

**Work items replace the per-light LUT run.** Today each `cold_light_data` row carries its own
`lut_index/lut_count`. Remove those; instead pass a **`hot-light-work`** uniform array, each item = **2 `u32`**:

```
u32  u1 light_temp | u1 prim_temp | u14 reserved | u16 index   // which light (hot/cold + index), which prim source
u32  u16 lut_index | u16 lut_count                             // its run of prims to shadow
```

So a work item = *(a light — hot or cold, at `index`) × (a run of prims — hot or cold)*. This decouples the
association from the light record, expresses every cell of the matrix above, and — because the shader only
processes the work items submitted — lets us **generate shadows per frame under a budget** (always run the hot
items; round-robin the cold×cold ones), instead of "draw everything, always."

**Per-light bitfield output.** Add a **`u8 shadow_bit_index`** to `cold_light_data` (into the `u32` freed by
removing the LUT run). Each light writes its **single bit** into the shadow map — up to `u8` = 256 bit slots.
`RGBA32UI` = 128 bits, so **128 lights** in one map (cold + hot sharing the space is likely enough; 256 = two
maps if wanted). This is the packed shadow *output* the `shadows` stream wants — our current cast draws
coloured triangles straight to screen; this replaces that with a real bitfield.

## Assessment (feedback, per the request)

**Good strategy — the right tiering, cleanly factored.** Three things are especially sound:

- **Uniform-vs-texture, same format.** Hot data changes every frame, so a small uniform upload beats a
  `texSubImage2D`; reusing the cold packed layout means **one decode path** in the shader (uniform *or*
  texel). Elegant + low-risk.
- **Work items over per-light runs.** Lifting the association out of the light record is what makes the
  cross-temporal cells (cold-light×hot-prim, hot×cold) expressible at all, and it's what turns "cast
  everything" into a **budgetable** list. Removing `lut_index/lut_count` and spending the freed `u32` on
  `shadow_bit_index` is a clean trade.
- **Per-light bit output.** Moving off coloured-triangles-to-screen onto a bitfield is the correct
  destination (feeds lighting; 128/256 lights).

The open items are details to pin, not flaws (see [`forks.md`](forks.md)):

- **F1 — hot prims need a hot LUT.** The work item's `lut_index/lut_count` for `prim_temp=cold` indexes the
  static cold LUT texture — fine. But a **hot** prim's associations change every frame, so they can't live in
  that static texture; they need a **hot-LUT uniform** (parallel to the cold LUT), selected by `prim_temp`.
  Same for a hot prim's *definition* (generic geometry): reuse the cold `prim_definition_data` (geometry is
  temp-agnostic) rather than a hot def.
- **F2 — the bitfield WRITE mechanism.** Setting each light's bit in an integer `RGBA32UI` target can't use GL
  blend (there's no bitwise-OR blend), and overlapping casters of one light must **OR**, not add. This needs
  the pack approach (integer RT + ping-pong OR, or disjoint-bit accumulation with a per-light union) — the
  same machinery the `shadows` stream specs. It's the biggest genuinely-new piece.
- **F3 — map space (RESOLVED: WORLD-space, per-bit) — supersedes `shadows`.** Because each light is its own
  **bit**, the staleness worry dissolves: cold×cold bits sit in the static `*-cold` bitfield; anything hot sets
  its bit in the `*-hot` bitfield, **fully rebuilt each frame** (no stale bits, and no partial-shadow-split —
  bits don't blend across rects). The lit output (`lightmap-cold`) then accumulates per rect via the
  **dirty-rect method** (static + a hot light map added). This **world-space per-bit + dirty-rect** model
  **replaces** `shadows`' screen→world 4-copy — so **`shadows` is superseded**, not absorbed. (Cost to watch:
  rebuilding the world-space `*-hot` bitfield each frame over the window — budget + few hot things bound it.)
- **F4 — two separate maps, 128 (RESOLVED).** `*-cold` static + `*-hot` rebuilt each frame, OR'd at read (F3
  settled this — not one shared map). 256 = a second `RGBA32UI`; `shadow_bit_index` `u8` keeps it open.
- **F5 — uniform limits + budget granularity.** The `hot-light-work` array + hot light/prim arrays are
  uniforms (GLSL component caps ~1–4k vec4). Movers are few, and the budget bounds the work list, but size the
  arrays + the per-frame budget explicitly.

**Verdict: proceed** (F1–F4 resolved). A hot/cold split on *both* lights and prims + a work-item/budget layer +
a **world-space per-bit bitfield** output that **supersedes the `shadows` stream** (close it). The lit
consumer (`lightmap-cold` dirty-rect accumulation + a hot light map) is the downstream stage this feeds. Phase
it: P0 layouts → P1 work-item cast → P2 bitfield output → P3 hot uniforms → P4 budget → P5 combine → P6 verify.

## Relationship

Builds directly on [`cold-data-textures`](../cold-data-textures/README.md) (extends `cold_light_data`, reuses
the cold textures + the position codec + the fan). Reconciles with / likely **absorbs the output half of**
[`shadows`](../shadows/README.md) (the screen-hot→world-cold bitfield). Layouts authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md) once F1–F4 settle.
