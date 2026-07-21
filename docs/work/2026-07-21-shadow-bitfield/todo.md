# Todo — 2026-07-21-shadow-bitfield

_Newest-first. Component: `client/webgl`. Design + assessment in [`README.md`](README.md);
open decisions in [`forks.md`](forks.md)._

## P5 · Budget + verify — 2026-07-21

- Per-frame **square budget** on the gather (like `SquareCache.bakeDirty`) — cap rects/squares
  cast per frame, priority-ordered (centre-out), `log()` any deferral rather than silently capping.
- Verify in-browser: overlapping cold-light shadows show **combined bit-colours** in `/overlayRT`;
  the field is **stable on pan** (toroidal wrap correct, no aliasing off-window); moving a light
  re-casts only its dirtied rects.

## P4 · Per-bit overlay (`usampler2D`) — 2026-07-21

- Extend `overlayShader` `OVERLAY_BITS` from float-mod on the RED byte to a real **`usampler2D`**
  decode of all 128 bits of `shadow-cold`; each set bit contributes its light's colour, **overlap =
  OR of colours**. Debug only, enabled via `/overlayRT`.
- Wire `shadow-cold` as an overlay-able target in `Viewport` (`/overlayRT shadow-cold`).

## P3 · Shared rect-accumulation helper — 2026-07-21

- Factor a **rect-accumulation** utility over the toroidal grid: coalesce dirty squares into larger
  pass **rectangles**, and **swallow clean squares** into a group when one larger pass beats many
  small ones (cost heuristic). Toroidal `mod`-wrap aware; grid-square granularity shared with the
  G-buffer. Consumers: `shadow-cold` (now) and `SquareCache` (later — it bakes 1 square at a time).

## P2 · The gather cast — 2026-07-21

- One fragment pass per pass-rectangle. Per fragment: loop cold lights (`cold_light_data` via
  `texelFetch`); **box-radius early-out** (cull if `|Δx|>r` **or** `|Δy|>r`); for survivors, walk the
  light's LUT run of casters (`cold_light_prim_data`→`prim_definition_data`+`cold_prim_data`) and run
  a **point-in-projected-silhouette** test (reuse `shadow-projection` math per-fragment); **OR** the
  light's bit (`1u << lightIndex`) into the accumulator; write the full `u128` once. No ping-pong.
- Retire the fan-scatter draw in `shadowCaster.ts` (its projection math moves into the fragment).

## P1 · `shadow-cold` buffer + dirty marking — 2026-07-21

- Allocate a **world-space toroidal `RGBA32UI`** buffer sized/windowed like a `SquareCache` channel
  (same `cols×rows`, `slotPx`, `mod`-wrap window, wrap-apron) — but integer, not baked from prims.
- **Dirty marking** on the existing square grid: a cold light's or caster's add/move/remove dirties
  the squares its **radius reach** covers (box), queued with a centre-out priority like `bakeDirty`.
