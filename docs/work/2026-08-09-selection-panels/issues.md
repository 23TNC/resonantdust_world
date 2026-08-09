# Issues — selection-panels (anticipated inventory)

Written 2026-08-09 at planning, before any code. Each is a thing we expect to hit; resolutions
land here as we hit them.

## I1 — the cards need `pointer-events: auto` inside a click-through panel

`ConditionCards` handles clicks (expand / collapse the strip) and hover (the tooltip). Under
[F2](forks.md#f2) the conditions panel's root is `pointer-events: none`, which its children
inherit — so every card must set `auto` explicitly, exactly as `TITLEBAR_CSS` / `ACTIONS_CSS` /
`TABS_CSS` already do for the `PixiPanel` case. Miss it and the cards go dead in a way that looks
like the panel isn't receiving the selection, not like a CSS inheritance problem.

## I2 — the tooltip must escape a one-row-tall panel's clip

The everything-tooltip IS the read surface for a condition (emotions F6) — the card carries no
text. A panel docked along the bottom is short, and `PANEL_CSS` sets `overflow: hidden`, so a
tooltip parented inside it is clipped to a sliver. It has to render outside the panel (the host,
as the strip does today) while the CARDS move inside. Note the irony to avoid over-correcting:
[F3](forks.md#f3) deletes the overlay for the cards; the tooltip is the one part that genuinely
still wants to escape.

## I3 — more conditions than fit the width

"Extend left to right inside the panel" is satisfied by a full-width panel for any plausible
count, but the overflow behaviour still has to be decided rather than discovered: horizontal
scroll, wrap to a second row, or continue to rely on the existing collapsed/expanded modes (top 4
maximized, the rest minimized). The existing collapse is probably the answer — it was built for
exactly this — but it was sized against the details panel's ~355px width and its constants
(`CARD_H`, `PAD_BOTTOM`, the 8-square fit note) assume that, so they want re-reading against a
full-width panel.

## I4 — a click-through, title-bar-less panel can only be moved from EDIT MODE

With `pointer-events: none` on the root and no title bar, there is no grab handle: the body passes
clicks through and there is no chrome. The way back is edit mode, which forces title bars visible
— behaviour that was only *made true* on 2026-08-09 (panel-grid: the class doc had always claimed
it, `refreshChrome` never did it, and entering edit mode also has to re-place the panel so the
body doesn't shift). So this stream leans on a very fresh guarantee. Verify on camera that the
conditions panel can be selected, moved and restored from edit mode BEFORE shipping it bar-less by
default; if it can't, the panel keeps its bar and the user hides it by hand.

## I5 — z-order for two new panels

`Z_TIER_INFO` (40) holds details and inventory. The conditions panel sits over the world but must
not cover the panels a user reads — and being click-through changes the calculus, since it can
overlap harmlessly. Intentions is a sibling of details and belongs at `Z_TIER_INFO`. Pick both
tiers deliberately against bug-sweep F1's table rather than defaulting to `Z_TIER_TOOLS`.

## I6 — two new corpus entries, or the panels open wherever

Every panel's geometry now comes from `defaults.json` cells → `defaultCell` → the grid
(panel-grid F3). Two new panels mean two new corpus entries plus authored `defaultCell`s, and the
conditions panel additionally needs `background: "none"`, `clickThrough: true` and
`titleBarHidden: true` authored — the whole point of its default. Miss the corpus entry and the
panel still opens, just at whatever its constructor says, which is easy to mistake for the cell
rect being wrong.

## I7 — details' reserves must go, not just its content

Removing the tenants is two edits, not one: the body's `padding-bottom: CARD_H + PAD_BOTTOM * 2`
(the band reserved for cards that were never its children) and the `STRIP_W = 40` left shift.
Leaving either behind gives details a permanent empty margin that looks like a layout bug. Its
`minWidth` / `minHeight` (220 / 150, now cell floors) also want re-reading once it is only
name + tile + a button.

## I8 — `sameCards` / re-render identity across the move

`ConditionCards` compares incoming cards to decide whether to re-render, and `DetailsPanel` drives
it from a 500 ms poll plus the selection event. The conditions panel needs the same cadence and
the same identity check; a naive "re-render on every tick" would rebuild the pie DOM continuously
and kill the tooltip's hover state mid-read.

## I9 — the background option must reach THREE surfaces

`CHROME_BG` is applied in `TITLEBAR_CSS`, `BODY_CSS` and `FOOTER_BG`. A background option that
only repaints the body leaves an opaque title bar and footer floating over a transparent panel —
which will read as a rendering bug. Decide explicitly whether `none` clears all three (probably)
and what `PixiPanel`'s existing `transparent` body override becomes once the option exists (it
should stop being a private override and start being a default).
