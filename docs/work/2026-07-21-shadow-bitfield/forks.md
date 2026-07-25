# Forks — 2026-07-21-shadow-bitfield

_Decision points + options + which we chose + why._

---

## F1 · Bit write: gather vs scatter (ping-pong) — 2026-07-21 (RESOLVED)

Setting each light's bit in the integer `RGBA32UI` `shadow-cold`. Options:

- **(a) Gather** — one fragment pass per rectangle; each fragment loops the reaching cold lights,
  tests shadow, ORs their bits **in a register**, writes the full `u128` once. No blend, no
  ping-pong; overlap is just multiple bits.
- **(b) Scatter + ping-pong OR** — rasterize each light's projected fan, OR its bit into the target;
  overlapping fans are a read-modify-write GL can't bitwise-blend → ping-pong (the old hot-shadows
  F2 (a)).

**Decided (user, 2026-07-21): (a) gather.** "I do not believe this requires a ping/pong because we
are not performing a read/modify/write, just updating all lights that affect a rectangle in a single
pass." Strictly cleaner than (b); **retires hot-shadows F2**.

**Cost justification (why gather, not marginally — 2026-07-21).** With `A` = dirty-rect area,
`L` = cold lights, `L_reach` = lights reaching a pixel/rect (post box-cull), `C` = casters/light,
`T` = one point-in-silhouette test:

- **Gather:** `A · (L·cull + L_reach·C·T)`, **one pass**, no blend. Scales with **dirty area × lights
  reaching × casters-each**.
- **Scatter:** `L · (A copy + covered·raster) + L swaps`. The rasterization is cheap; the tax is that
  a correct **128-bit integer** bitfield forces **O(L) passes** — you can't bitwise-OR into an integer
  target, so every variant (per-light ping-pong copy+OR, stencil dedup, float additive-disjoint) pays
  per-light work + precision limits. **No cheap single-pass scatter into a 128-bit integer target.**

The regime decides it. Scatter wins for **few lights / huge silhouettes / whole-screen rebuild**
(hardware coverage beats per-pixel testing, small `L` → cheap ping-pong). **We are the opposite on
every axis:** dense many-lights (finite radius → tiny `L_reach`, but scatter's pass count = `L`, so
it's punished by the very thing we scale up); dirty-rect incremental (gather cost ∝ dirty area;
scatter re-pays `L·A` copies regardless); few casters/light (small inner term); world-space
persistent (gather recomputes a rect in isolation; scatter can't *subtract* a bit from an OR, so an
incremental change clears + re-scatters the rect's whole light set anyway — gather's work as N
passes). Gather's honest costs — testing whole-rect area incl. ultimately-unshadowed pixels, and warp
divergence — are bounded by the box-cull, the **per-rect light pre-cull** (F5), and modest,
spatially-coherent rects. **Verdict: gather, decisively.**

## F2 · Where the shadow-shape test lives (consequence of gather) — 2026-07-21 (RESOLVED)

Gather forces the shape test into the fragment. Options:

- **(a) Per-fragment predicate** — each fragment, per reaching light, loops the light's casters and
  runs a point-in-projected-silhouette test against the cold prim textures (`texelFetch`). Reuses
  `shadow-projection`'s math as the predicate.
- **(b) Keep the rasterized fans** — impossible under gather without reintroducing scatter+ping-pong.

**Decided (2026-07-21): (a).** The projection formula is reused per-fragment; the fan-scatter draw in
`shadowCaster.ts` retires. Cost is per-fragment × per-reaching-light × per-caster — bounded by the
box cull, LUT-bounded casters, the **per-rect light pre-cull** (F5), and the per-frame budget (P5).

## F3 · Dirty-rect grouping granularity + clean-square swallow — 2026-07-21 (SUPERSEDED)

_Was: how to group dirty squares into pass rectangles (AABB vs greedy strips)._

**SUPERSEDED 2026-07-21 by the dirty-tile bitfield + single full-window pass (F6).** No grouping at
all: one pass over the whole window, each fragment reads its tile's **dirty bit** (a 64-`uvec4`
uniform, one bit/tile) and `discard`s if clean — leaving the persistent `shadow-cold` pixel
untouched. So the rect-accumulation machinery is unnecessary for shadows. Budgeting survives trivially
(send a subset of the dirty bits per frame). The shared rect-accumulation idea is now **decoupled
from shadows** — it would only ever serve `SquareCache`'s per-square bakes, a separate question.

## F4 · Bit capacity + bit assignment — 2026-07-21 (RESOLVED for cold-only)

`RGBA32UI` = 128 bits. Cold-only: **bit = the light's index** in `cold_light_data` (0..127) —
**no `shadow_bit_index` field** needed. 256 lights = a 2nd `RGBA32UI` later; the explicit `u8`
`shadow_bit_index` stays a **hot-tier** concern ([`hot-shadows`](../hot-shadows/README.md), tabled).

## F5 · The light cull — CPU per-rect list vs per-tile presence bitfield — 2026-07-21 (RESOLVED)

The cost win (F1) hinges on the fragment looping only a **few** lights, not all 128. Options:

- **(a) CPU pre-cull → per-rect light-index list** (uniform array). My original lean; fine but CPU
  work + a variable-length uniform per rect.
- **(b) `light_presence_cold` — a per-tile light bitfield texture.** A `128×64` (window+overscan)
  `RGBA32UI`, one px per tile, **each bit = a light that reaches that tile** (distance cull, set
  CPU-side when a light changes). The gather fragment reads its tile's presence px and iterates only
  the **set bits** — a popcount, not a scan of 128. Data-driven, computed once per light-change.

**Decided (user, 2026-07-21): (b) `light_presence_cold`.** Strictly better than (a) — no per-rect CPU
list, the cull is a texture read. Maintenance: presence changes **only** on a light add / move /
radius change (pure reach — prims don't touch it), distinct from the dirty-tile set (F6), which
changes on a light **or** caster move. Size to **window + `OVERSCAN`** so apron tiles get bits (else
edge tearing on pan).

**Caster-move dirtying (user, 2026-07-21).** When a caster moves we know its **index**, so the CPU maps
index → the lights that reference it (a reverse lookup it maintains), and each light's presence tells
which tiles it reaches → **mark those tiles dirty**. This **over-approximates** (dirties the lights'
whole reach, not just the caster's actual shadow footprint) — acceptable short-term. Tighter option
(track the tiles a caster's shadow occupies) is deferred — "gets messy quick."

## F6 · Fixed-size texture consolidation + single-pass dirty update — 2026-07-21 (RESOLVED)

Fix everything to constant slots (the user: simplifies alloc + pre-shapes warm/hot as bit-partitions,
not reallocations). **N = 128 lights**, **≤ 256 shadow casters/light**, `u16` def/prim indexes ⇒
65 536 ceilings. The four cold textures consolidate (deletes the old `cold_light_billboard_data` LUT):

| texture | size (`RGBA32UI`) | contents |
|---|---|---|
| `light_data` | **128×33** | col = light. **row 0** = light record; **rows 1–32** = 256× `u16` prim indexes (the LUT, `lut_index` now **implicit** = column x) |
| `billboard_data` | **256×128** | all cold prims, **2/px** (`u64` each): `u32` position; `u8 z \| u2 rot \| u16 def_index \| u6 rsvd` |
| `billboard_definition_data` | **256×256** | one def/px (4 `u32`), keyed by `u16 def_index` |
| `light_presence_cold` | **128×64** (window+overscan) | one tile/px; bit = a light reaching it (F5) |
| dirty-tile bits | **64 `uvec4`** uniform (or a tiny texture) | one bit/tile; gates the single pass |

**Update model: single full-window pass, `discard`-gated.** One pass over `shadow-cold`; each fragment
reads its tile's dirty flag (from `shadow_dirty`, an **`R8UI` texture** — decided over a uniform, to
keep the fragment-uniform budget free for warm/hot) and `discard`s if clean (persistent RT untouched →
**no ping-pong on `shadow-cold`**). CPU maintains `light_data`/`billboard_data`/`billboard_definition_data`/
`light_presence_cold`/`shadow_dirty` and uploads changed regions (cold changes rarely; **no GPU
ping-pong** for the data — CPU is the writer). Budget = mark only a subset of tiles dirty per frame
(optional).

**Sizes:** ~1 MB defs + 512 KB prims + 128 KB presence + 66 KB light_data ≈ **1.7 MB** VRAM. Cheap.

**RESOLVED (user, 2026-07-21) — no `caster_count`; sentinel index 0.** The caster LUT run is a **dense
list of `u16` prim indexes terminated by a `0`** (prim index 0 reserved as the sentinel). So the
fragment loops until it hits 0 or 256; no count field — the freed light-record A-channel stays
reserved. Cost: prim index 0 is unusable → **65 535** usable prim slots (1..65 535), not 65 536. This
unblocked the authoritative `VARIABLES.md` layout (written 2026-07-21). `shadow_dirty` = an `R8UI`
texture (nonzero = dirty), not the uniform.

**Nit resolved:** `billboard_data` is **256×128** (256×126×2 = 64 512 would under-address `u16`).

## F7 · Def-index width (u16) overrun — 2026-07-21 (RESOLVED, watch)

`u16 def_index` ⇒ 65 536 distinct `(type,subtype,kind,variant)` definitions; the game's `u32` id space
could theoretically exceed it. **In practice we won't** (65 k distinct object variants is implausible
before other limits bite). If ever hit: bump `def_index` `u16→u20` (the prim's `u6 reserved` covers
it) + grow `billboard_definition_data`, or add eviction. Not now.
