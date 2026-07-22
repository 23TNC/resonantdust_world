# Work — 2026-07-21-shadow-bitfield (world-space per-light shadow bitfield, dirty-rect gather)

_Opened 2026-07-21. Move the shadow cast off "coloured triangles to screen" onto a persistent
**world-space toroidal `shadow-cold` `RGBA32UI` bitfield** where **each cold light is one bit**.
Rebuilt by a **dirty-rectangle gather**: a single fragment pass per (grouped) rectangle loops the
cold lights that reach it, tests shadow, and ORs their bits — **no ping-pong**. A debug
`/overlayRT` decodes the bits to up to 128 per-light colours (overlap = OR of colours). The bitfield
is the **per-light mask** the lighting pass will consume later. Component: `client/webgl`._

## The idea (the user's design, 2026-07-21)

Hold a **`shadow-cold`** buffer as a **world-space toroidal `RGBA32UI`** (128 bits). **Every bit
represents one cold light** in `cold_light_data`. Maintain it by **dirty tiles** on the same toroidal grid we already use for the G-buffer. The
**fixed-layout redesign (2026-07-21, F5/F6)** settled the culling + update model:

1. **Fixed slots.** `N = 128` lights, `≤ 256` shadow casters/light, `u16` def/prim indexes. Four cold
   textures consolidate to `light_data` (128×33 — row 0 lights, rows 1–32 the LUT), `prim_data`
   (256×128, 2/px), `prim_definition_data` (256×256) — deleting the old `cold_light_prim_data`. See
   [F6](forks.md#f6).
2. **`light_presence_cold`** (128×64, window+overscan) — one tile/px, **each bit = a light reaching
   that tile** (distance cull, CPU-set on light change). The gather reads its tile's presence px and
   iterates only the **set bits** — a popcount, not a scan of 128. This *is* the cull ([F5](forks.md#f5)).
3. **Dirty-tile bitfield** (64 `uvec4` uniform, one bit/tile) — a light **or** caster change marks
   tiles dirty. **One full-window pass**; each fragment reads its tile's dirty bit and **`discard`s
   if clean** (persistent RT untouched). No rect grouping ([F3](forks.md#f3) dropped).
4. For a dirty pixel, loop the tile's present lights; per light walk its `caster_count`-bounded
   casters, box-cull, test point-in-silhouette, **OR** its bit into a register, write the `u128` once.
5. **No ping-pong** anywhere — the gather is not a read-modify-write (bits OR'd in-register); the RT
   is updated in place via `discard`; the cold data is CPU-maintained + region-uploaded.
6. The **`/overlayRT` overlay** (debug) colours each pixel by its set bits — up to 128 colours,
   overlap combines them. The bitfield is later the **per-light mask** for lighting.

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

**Rect-accumulation dropped — the redesign obsoletes it.** My earlier plan wanted a shared helper to
group dirty squares into pass rectangles. The **dirty-tile bitfield + single full-window `discard`-
gated pass** (point 3) removes any grouping: one pass, per-tile gate, done. `SquareCache`'s per-square
bake is a *different* problem (scatter, one draw/prim) that this trick doesn't cover, so the shared
helper is decoupled from shadows — revisit for `SquareCache` only if it ever pays off there.

**Watch-items:** box cull = *survive iff within radius in x **and** y*; bit = the light's **index** in
`light_data` (0..127); the inner **caster loop** is the one perf cliff (a tile under many lights, each
with a long list) — `caster_count` bounds it and the dirty-bit **budget** (P6) caps a big-change spike;
overlay decode moves `overlayShader` `OVERLAY_BITS` from float-mod on the RED byte to a real
`usampler2D` decode of all 128 bits. **One pin before `VARIABLES.md`:** the `caster_count` home
([F6](forks.md#f6)).

**Verdict: proceed.** Phase it: P1 consolidate the cold textures to the fixed layout + allocate
`shadow-cold` → P2 `light_presence_cold` (per-tile light cull) → P3 dirty-tile tracking + the dirty
uniform → P4 the gather cast (single `discard`-gated window pass) → P5 the `usampler2D` per-bit
`/overlayRT` overlay → P6 budget + verify.

## Scope

**In:** consolidating the cold textures to the fixed layout (F6, deleting `cold_light_prim_data`);
the `shadow-cold` `RGBA32UI` world-space toroidal buffer; `light_presence_cold` (per-tile light cull);
dirty-tile tracking + the dirty-bit uniform; the single-pass `discard`-gated fragment **gather**
(present lights → `caster_count` casters → point-in-silhouette → OR bits); the `usampler2D` per-bit
`/overlayRT` overlay; an optional dirty-bit **budget**.

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
