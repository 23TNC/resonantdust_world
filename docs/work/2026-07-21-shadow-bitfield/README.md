# Work — 2026-07-21-shadow-bitfield (world-space per-light shadow bitfield, dirty-rect gather)

_Opened 2026-07-21. Move the shadow cast off "coloured triangles to screen" onto a persistent
**world-space toroidal `shadow-cold` `RGBA32UI` bitfield** where **each cold light is one bit**.
Rebuilt by a **dirty-rectangle gather**: a single fragment pass per (grouped) rectangle loops the
cold lights that reach it, tests shadow, and ORs their bits — **no ping-pong**. A debug
`/overlayRT` decodes the bits to up to 128 per-light colours (overlap = OR of colours). The bitfield
is the **per-light mask** the lighting pass will consume later. Component: `client/webgl`._

## The idea (the user's design, 2026-07-21)

Hold a **`shadow-cold`** buffer as a **world-space toroidal `RGBA32UI`** (128 bits). **Every bit
represents one cold light** in `cold_light_data`. Maintain it by **dirty rectangles** on the same
toroidal grid we already use for the G-buffer:

1. **Mark** rectangles whose cold shadowing changed (a light or caster moved / was added).
2. **Group** dirty rectangles into a larger rectangle and update it in one pass; **swallow clean
   squares** into the group when one larger pass is cheaper than many small ones.
3. For each pass rectangle, **calculate all cold lights against it in a single fragment pass**. Per
   fragment, loop the cold lights; **box-radius early-out** any light that can't reach in x **or** y
   (Chebyshev — cheaper than a distance test, and the lighting pass corrects the corner error);
   for a surviving light, test shadow and **set its bit**.
4. **No ping-pong** — this is a *gather*, not a read-modify-write: each fragment computes its full
   128-bit value locally (ORs bits in a register) and writes once. Overlap is just multiple bits set.
5. The **`/overlayRT` overlay** (debug) colours each pixel by its set bits — up to 128 colours,
   overlap combines them. The bitfield is later used as a **per-light mask** for lighting.
6. The **rect-accumulation** (merge dirty rects, swallow clean squares) is a **shared utility** for
   *every* dirty-rect world-space toroidal texture we keep (the G-buffer `SquareCache` can adopt it).

## Assessment (feedback, per the request)

**Endorsed — this is the right model and it supersedes the ping-pong I had penciled in.** The
earlier plan (hot-shadows [F2](../hot-shadows/forks.md)) treated the bit-write as a **scatter**:
rasterize each light's projected fan and OR its bit into the integer target — which *forces*
ping-pong, because overlapping fans are a read-modify-write on a target GL can't bitwise-blend. The
**gather** inverts it: one fragment loops the reaching lights and ORs their bits **in a register**
before its single write. No blend, no ping-pong, and the overlap-OR problem disappears. Strictly
cleaner → **retire F2's ping-pong** (see [F1 here](forks.md#f1)).

**The one consequence to name (proceeding on it):** the gather moves the shadow *shape* out of
rasterized geometry and **into the fragment**. `shadowCaster.ts` today rasterizes projected triangle
fans (scatter); the gather instead makes each fragment, per reaching light, loop that light's casters
and run a **point-in-projected-silhouette** test against the cold prim textures we already built
(`texelFetch`). The [`shadow-projection`](../shadow-projection/README.md) projection math is **reused
as the per-fragment predicate** — not thrown away — but the draw shape changes from "N instanced
fans" to "one quad per rect, heavy fragment." Right trade for a bitfield ([F2 here](forks.md#f2)).

**We're half-way on the shared dirty-rect utility.** `SquareCache` already owns the world-space
toroidal window + per-square dirty queue + wrap-apron + budgeted `bakeDirty`. Two real gaps:
- It bakes **one `SQUARE` (16px) at a time** (`bakeSquare`) — it does **not** coalesce adjacent dirty
  squares into larger rectangles. The user's group-and-swallow is genuinely new work → factor a
  shared **rect-accumulation** helper both `shadow-cold` and `SquareCache` can consume.
- Its unit is that fixed grid square — exactly what `shadow-cold` should reuse for dirty marking
  (same grid ⇒ same window / apron / `mod`-wrap), with the accumulator merging runs into pass rects.

**Watch-items:** box cull = *survive iff within radius in x **and** y* (cull on failing **either**);
bit = the light's **index** in `cold_light_data` (0..127 — no `shadow_bit_index` field for the
cold-only tier; that explicit `u8` stays hot-tier); cost is per-fragment × per-reaching-light ×
per-caster, so keep a per-frame square **budget** like `bakeDirty`'s — grouping amortizes setup, not
the inner loop; overlay decode moves `overlayShader` `OVERLAY_BITS` from float-mod on the RED byte to
a real `usampler2D` decode of all 128 bits.

**Verdict: proceed.** Phase it: P1 the `shadow-cold` `RGBA32UI` toroidal buffer + dirty marking on
the existing grid → P2 the gather cast (fragment loops lights + casters, box-cull, OR bits) → P3 the
shared rect-accumulation helper (group + swallow) → P4 the `usampler2D` per-bit `/overlayRT` overlay
→ P5 budget + verify (overlapping shadows show combined bit-colours; no aliasing on pan).

## Scope

**In:** the `shadow-cold` `RGBA32UI` world-space toroidal buffer; dirty marking on the existing
square grid; the single-pass fragment **gather** (loop cold lights, box-cull, per-caster
point-in-silhouette, OR bits); the shared **rect-accumulation** helper; the `usampler2D` per-bit
`/overlayRT` overlay; a per-frame square budget.

**Out (deferred / tabled):** the **hot** tier ([`hot-shadows`](../hot-shadows/README.md) — tabled,
"more thought"); the **lit** output (ambient + `lightmap-cold` accumulation) — this stream produces
the **mask only**, lighting consumes it next; a 2nd `RGBA32UI` for 256 lights.

## Relationship

Builds on [`cold-data-textures`](../cold-data-textures/README.md) (reuses the 4 cold `RGBA32UI`
textures + position codec + LUT via `texelFetch`) and reuses the toroidal window/dirty/apron of
`SquareCache`. Reuses [`shadow-projection`](../shadow-projection/README.md)'s projection math as the
per-fragment predicate. **Supersedes** the screen→world 4-copy of [`shadows`](../shadows/README.md)
(per hot-shadows [F3](../hot-shadows/forks.md)) — world-space per-bit + dirty-rect gather replaces
it. Retires the ping-pong of hot-shadows F2. Layouts stay in [`docs/VARIABLES.md`](../../VARIABLES.md).
