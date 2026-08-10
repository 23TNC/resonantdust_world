# Issues — live-edit (anticipated inventory)

Written 2026-08-09 at planning, before any code.

## I1 — a second GL context may not be available at all

[F1](forks.md#f1)'s candidate A assumes a second WebGL2 context can be created. Browsers cap live
contexts (commonly ~8–16) and drop the OLDEST when the cap is hit — which would kill the *world*
viewport to show a preview of it. The spike must check creation success and what happens on
repeated open/close of the panel, not just frame cost: a leak here is invisible until the eighth
`/edit` blanks the game.

## I2 — no accessor lists a pawn's active traits

`pawn_conditions` exists; there is no `pawn_traits`. The traits tab needs the pawn's active trait
rows resolved to `(reference, label, colour)` — the same shape `pawn_conditions` returns for
conditions, and it should mirror that function deliberately so the two read alike. Note the trait
row is the ONE u64 shape (`dead:16 | data:16 | reference:32`) whose data half is reserved-zero
today, so the accessor reads identity only.

## I3 — the bar's bounds and the rate must come from the SAME evaluation

The authored `min`/`max` are the need's domain, but conditions and traits can **narrow the
effective clamp** ("min/max narrow the effective clamp" — the corpus header), and the live rate is
a product over the same modifiers. Drawing the bar from authored bounds while taking the rate from
live modifiers gives a panel that is individually plausible and jointly wrong: a bar reading 100%
full while the effective max is 60. [F3](forks.md#f3)'s single `(value, min, max, rate)` accessor
exists to make that mistake unavailable.

## I4 — sign convention has to survive a need whose domain is inverted

The user's rule is "− red, + green" — losing is bad, gaining is good. That is right for thirst and
hunger (satisfaction depleting toward `min`), but the corpus explicitly allows a negative `min`
and says "min may be negative, that IS the sign treatment", and `inventory` is a need whose
"satisfaction" is FREE SLOTS. Decide once, in the accessor, what the sign of the returned rate
means (recommendation: sign of the change in SATISFACTION, so "going down is red" holds for every
need by construction) and let the panel colour it blindly.

## I5 — trait/need colour is a corpus-wide edit, and the golden fixture will move

[F2](forks.md#f2) adds an authored field to two def families. Every trait and need in `content/`
wants a colour, the loader gains two fields, and the golden fixture that guards table shapes
re-blesses. Per the standing rule that re-blessing is deliberate: the diff should be *only* the
new colour columns, and anything else moving in it is a bug worth stopping for.

## I6 — the preview needs something to look AT for a non-pawn selection

Tiles and things are selectable. A "live preview with zoom and pan" of a tile is coherent (it is a
world position) but of a streamed-out thing is not. [F4](forks.md#f4) says the panel opens with
empty tabs for non-pawns; the preview needs the same courtesy — show the world at that position,
or show nothing gracefully, but not a half-initialised renderer.

## I7 — the panel is big, and the grid's minimum is now 1×1

`live_edit` wants real estate: a preview above and a tabbed sub-panel below. It should author a
generous `defaultCell` and a `minCols`/`minRows` that reflects what it genuinely cannot work
below — this is exactly the case the per-panel minimum was introduced for (2026-08-09), and the
first panel that has a real reason to raise its floor above the 1×1 default.

## I8 — "sub panel with tabs" is the panel's OWN tabs, not a nested DomPanel

`DomPanel.addTab` is mutually exclusive with `setBody`. The preview + tab strip layout therefore
either (a) uses the panel's tabs with the preview living inside every tab's content, which
duplicates it, or (b) uses `setBody` with a hand-rolled tab strip beneath the preview. (b) is
almost certainly right — the preview must persist across tab switches — but it means the tab strip
is this panel's own widget rather than the built-in one, and it should look like the built-in one
so the app stays coherent.
