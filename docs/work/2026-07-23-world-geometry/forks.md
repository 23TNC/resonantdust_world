# Forks — world geometry

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Where the canonical model lives {#f1}
**2026-07-23 — open (P0).** This stream is *flowing* state; the geometric model is **durable truth** and
must outlive it. Options: (a) a `design/` doc under the client component (it's a rendering model —
"the shape"); (b) a top-level cross-cutting doc alongside `VARIABLES.md` / `TABLES.md`, since geometry
spans everything that draws or casts. Lean **(a)** — it's a client-rendering model, not a data layout —
with this stream's README linking to it, not restating it (a layout/model must live in exactly one place).

## F2 · The north-offset `0.5` — wedge half-depth, or conform to parallel-to-view? {#f2}
**2026-07-27 — RESOLVED: conform to `H·cos(tilt)` (option b). Shipped as `cardLean = 1.0`.**
Settled by arithmetic, not taste. In an oblique view at θ a world displacement (`north = n`, `up = z`)
projects to a screen-vertical extent of `n·cosθ + z·sinθ`. For a sprite DRAWN `H` tall:

| lean | north | up | screen height |
|---|---|---|---|
| **1.0** | `H·cos55 = 0.574H` | `H·sin55 = 0.819H` | `0.574·0.574 + 0.819·0.819` = **`1.000·H`** ✓ |
| 0.5 | `0.287H` | `0.819H` | `0.287·0.574 + 0.819·0.819` = **`0.836·H`** ✗ |

Only 1.0 is self-consistent: the caster card occupies exactly the screen height of the sprite the player
actually sees. The `0.5` cast shadows from a card ~16 % SHORT on screen — it did not compensate for a
draw/maths mismatch, it *created* one (user hypothesised the reverse; the arithmetic inverts it).

**The wedge-half-depth hypothesis below is refuted.** The design's ½ is half an average *opaque extent*
measured on the depth axis and derived per sprite from a presence map; this `0.5` multiplied the *lean*
(`H·cosθ`) and was a hardcoded constant. Both are halves; they are not the same half.

Verified: corridor↔brute BIT-IDENTICAL at lean 1.0 (67 437 nonzero texels, 0 differing) — the shader's
`uCardLean` and `buildCasters`' TILT are routed through one `cardLean` field so they cannot drift, and
that is what the identity run proves. Live dial `__lean(x)` retained for re-testing.

_Original 2026-07-23 framing, kept for the reasoning:_
The caster elevation already matches (`H·sin65`). The ONLY difference is the north offset: `shadowCover`
uses `0.5·H·cos65`; strict parallel-to-view is `H·cos65`. Options: (a) **keep `0.5`** — plausibly the
shadow-design "wedge" `±depth` half, i.e. deliberate; the shadows are tuned and read correctly; zero churn;
(b) **conform to `H·cos65`** — matches the strict parallel-to-view model, but lengthens/re-shapes the
projected shadow and needs re-tuning + a corridor↔brute re-check. Low-stakes either way (elevation
unchanged), and testable in-browser by flipping the factor. Recommend leaving `0.5` unless the eye or the
prim-shadow work shows a real inconsistency — it's a look call, not a correctness one.

## F3 · `SHADOW_LIFT` — keep it {#f3}
**2026-07-23 — CORRECTED: keep it (it's sprite-padding, not a card symptom).** `SHADOW_LIFT = 3` seats the
shadow on the sprite's *visible* base past its transparent padding ([issues I-2](issues.md#i2)) — real
regardless of the projection. It will NOT trend to 0. A cleaner long-term fix is to project from the
sprite's content-bottom (a def-anchor change), out of scope here.

## F4 · One tilt constant {#f4}
**2026-07-23 — yes.** `65.0` is currently written as a literal inside shader math in more than one place.
Name it once (shared constant + the model doc) so a future change is one edit, and so the value is
visibly *the* world tilt rather than a magic number per shader.

## F5 · Scope — align before or after prim shadows? {#f5}
**2026-07-23 — REVISED: no hard dependency.** Original call was "align first," on the belief the caster ran
a ~4.7×-off vertical model. The audit corrected that: the **elevation already matches** (`sin65`), so a
receiver's true elevation already shares the caster's frame. [`shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md)
can proceed **now**. The only lingering shared question is F2's north-offset `0.5`, which affects the
caster's ground *placement*, not the elevation the receiver work needs — settle it whenever, in either
stream.
