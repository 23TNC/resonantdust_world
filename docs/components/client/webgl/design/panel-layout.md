# Panel layout — the 58×33 cell grid as the UI coordinate system

The client's UI placement floor. **Every panel, both taskbars, every title bar and the whole
text scale are expressed in cells of one grid**; pixels are a projection of that grid against
the live viewport, never a stored quantity. Authored 2026-08-09 (work
[`2026-08-09-panel-grid`](../../../../work/2026-08-09-panel-grid/README.md)); the decisions
behind each clause are that stream's `forks.md` F1–F9.

## The grid

- **58 columns × 33 rows, over the FULL viewport** — not the area between the taskbars. The
  bars are *in* the grid, which is the only way they can be one row tall.
  - **row 0** = the top taskbar, **row 32** = the bottom taskbar,
  - **rows 1..31** = the **field**: the 31 rows panels may occupy.
- The count is **fixed on every aspect**. Cells stretch; they do not stay square. 58:33 = 1.758
  against 16:9's 1.778, so cells are square within 1.2% on a normal display and progressively
  wider off it. Nothing may size itself `1 cell × 1 cell` expecting a square — size off the
  **row**, which is the smaller axis in practice.
- `GRID_COLS` / `GRID_ROWS` are named constants in one module, with **no derived duplicates
  anywhere**. The pair is the compensation knob: when something measures wrong — chrome too
  light, a row too short for a title bar — the first lever is the cell *count*, one line, not a
  special case.

## Edges are an integer table

The grid publishes two tables rebuilt from the live viewport:

```
edgeX[i] = round(i * innerWidth  / 58)     i = 0..58
edgeY[j] = round(j * innerHeight / 33)     j = 0..33
```

A cell rect's pixels are **differences of table entries**, never `origin + n * step`. So every
boundary is one integer shared by both neighbours (no seams, no overlaps), cells vary by at most
1px, and the tables tile the viewport exactly: `edgeX[58] === innerWidth`,
`edgeY[33] === innerHeight`.

There is no float `stepX`. The float-step form is what produced the drift visible in the
pre-grid corpus (`"top": "56.3295px"`, `"bottom": "31.8977px"`) — rounding happened per gesture
instead of per edge.

## The cell is the persisted unit

A panel stores **four integers**: `col`, `row`, `cols`, `rows`. Placement is one function,
`project(cellRect) → {left, top, width, height}`, read off the edge tables and written in a
single pass with `right`/`bottom` set to `auto`.

A viewport resize therefore **re-projects**; it does not reflow, round, or drift. The cell rect
is untouched, so a resize round-trip is exactly identity. This is the property the whole design
exists to buy, and it is why the cell count may not adapt to aspect (that would make a resize a
reflow) and why no minimum cell pixel size exists (the viewport would stop dividing into 58×33).

Falls out of this:

- **`anchor` is the resize pivot only** — which corner stays put during a resize gesture, and
  therefore which corner carries the grab handle. It no longer participates in placement; the
  projection owns all four CSS edges in one pass.
- **`snap`** (glue to a corner) is a placement *preset* expressed in cells: `top-left` is
  `col 0, row 1`; `bottom-right` is the far corner of the field.
- **Grid snap is the law, not a mode.** Every drag and resize quantizes to cells. There is no
  unsnapped path.

## The stored rect is the BODY's; the title bar is the row above it

The title bar is **exactly one row**, and the persisted cell rect describes the **body**. A
visible title bar is chrome occupying the row immediately above the body:

```
outer.rows = body.rows + (titleBarVisible ? 1 : 0)
outer.row  = body.row  - (titleBarVisible ? 1 : 0)
```

So **toggling the title bar mutates no persisted geometry and leaves the body's bounding rect
bit-identical** — the body neither moves nor resizes. Consequences: a titled panel's body starts
at **row ≥ 2** (its bar cannot occupy row 0, the taskbar); an untitled body may sit at row 1;
"hide the title bar to reclaim the top row" is a real move.

Minimums and height presets are in cells too: minimum body **6 × 3**; `heightMode`
full/half/quarter are row counts over the field; `auto` rounds **up** to whole rows and
re-applies only when the row *count* changes (a pixel-level re-apply oscillates).

## The chrome is derived, never constant

Both taskbars measure one row (`edgeY[1] - edgeY[0]` and `edgeY[33] - edgeY[32]`) and re-height
on every grid change. There is no taskbar height constant — the old `PanelTaskbar.HEIGHT = 32`
was the tail wagging the dog: it *defined* the grid. The reserved strips the field sits inside
are likewise derived, one row each.

## The UI scale

Text scales with the grid, derived from the **row height** — never the column width, since cells
are not square off 16:9 and column-derived text would inflate on an ultrawide.

The ratios are a **discovery, not an invention**: measured against the pre-grid 32px bar, the
chrome was already authored in eighths of a row — the 12px font is 3/8, the 4px gaps and small
paddings 1/8, the 8px padding 2/8, the 24px taskbar entries and glyph buttons 6/8, the 32px
action buttons 8/8, the 100px entry minimum 25/8 (the 14px secondary font is the one half-step).
The scale names the rhythm the chrome already had, which is why a 1080-tall viewport lands back
on exactly 12px.

**Mechanically: JS writes one custom property, CSS derives the rest.** On each grid recompute
the grid sets `--ui-row` on `document.documentElement` and nothing else. The derived set lives
once, as `calc()` in the `:root` block of `index.html`:

| property | value | at a 1080-tall viewport |
|---|---|---|
| `--ui-row` | written by the grid | 32.7px |
| `--ui-font` | `max(9px, calc(var(--ui-row) * 0.375))` | 12px |
| `--ui-font-lg` | `calc(var(--ui-font) * 1.1667)` | 14px |
| `--ui-font-xl` | `calc(var(--ui-font) * 1.3333)` | 16px |
| `--ui-pad` | `calc(var(--ui-row) * 0.25)` | 8px |
| `--ui-pad-sm` | `calc(var(--ui-row) * 0.125)` | 4px |
| `--ui-gap` | `calc(var(--ui-row) * 0.125)` | 4px |
| `--ui-btn` | `calc(var(--ui-row) * 0.75)` | 24px |

No component computes a font size. One `setProperty` per resize re-styles every element with no
traversal. Hairline borders stay literal `1px` — a hairline is not a scale.

**The font has a 9px floor; the cell has no floor.** These are different calls, not an
inconsistency. A cell floor breaks the layout invariant (the viewport stops dividing into 58×33
and a resize stops being a re-projection). A font floor breaks nothing — glyphs do not
participate in the grid arithmetic — it only means that on a small viewport text is relatively
larger and eventually tight in its row. Legible-and-tight beats unreadable-and-proportional.
There is no upper clamp: a 4K viewport gets a 65px row and 24px text, which is the UI scaling
correctly.

## Appearance and interaction are per-panel options

Two axes the grid deliberately does **not** own, added by work
[`2026-08-09-selection-panels`](../../../../work/2026-08-09-selection-panels/README.md) (F1/F2).
They join the layout/scale split above as a third category: *layout* comes from the projection,
*scale* from `--ui-row`, and **appearance/interaction from per-panel options**.

- **`background`** — a named value, not a colour literal: `chrome` (the default
  `rgba(20,22,30,0.96)`), `dim`, `none`. It reaches **all three** chrome surfaces — title bar,
  body and footer — because a panel whose body alone goes transparent keeps an opaque bar floating
  over nothing, which reads as a rendering fault rather than a setting. `none` means genuinely
  transparent; it is what lets a panel sit over the world without occluding it.

  Named rather than free because the settings popup speaks toggles and cycling selects, and the
  corpus is hand-edited JSON where `"background": "none"` survives review. The value is a
  **string**, so an arbitrary `#rrggbb` remains an additive change to one resolver if it is ever
  wanted.

- **`clickThrough`** — `pointer-events: none` on the panel root, so the body passes clicks to
  whatever is beneath (the world canvas). Chrome keeps `pointer-events: auto`, and so must any
  interactive content: **children inherit `none`**, so a click-through panel's own controls have
  to opt back in explicitly.

**These two are independent, and must stay so.** Transparency is not click-through: a panel can be
transparent and interactive, or opaque and click-through, and both are coherent. The proof lives
in the conditions panel, which is transparent AND click-through AND has clickable cards inside it
— one flag could not express that. The pattern predates the options: the **deleted** `PixiPanel`
(gone with the pixijs client on 2026-07-28) set `pointer-events: none` on its root with `auto` on
the title bar, tabs and action buttons. The options turn what was one class's private arrangement
into a declared per-panel setting. Nothing in `client/webgl` mirrors that class today — the only
`DomPanel` subclass that mirrors its own body rect is `ViewportPanel`, which hosts the viewport's
canvas and re-sizes it on `rectChange`.

**Consequence worth knowing before you ship a panel with both.** A click-through panel with no
title bar has **no grab handle at all** — its body passes clicks through and there is no chrome to
drag. The way back is UI edit mode, which forces title bars visible (see the title-bar section).
Any panel authored bar-less *and* click-through depends on that being true.

## The three selection surfaces

A selected object is described by three sibling panels, not one. This is a division of labour, and
the reason it exists is that the previous single panel could not hold them:

| panel | shows | why separate |
|---|---|---|
| **details** | identity — the object's TOML name and tile, plus the active emotion wash | the user's standing rule: "I just need to know what something is" |
| **intentions** | the intent QUEUE — the pending/active entries with the active one's progress ring | a queue is a list that grows; it was a column squeezed against details' left edge |
| **conditions** | the condition CARDS — one emotion-pie square per active condition | cards run left-to-right and need the full width, which is why they used to escape their panel |

**Each subscribes to the selection model directly.** None is fed by another. The conditions strip
previously lived *outside* the details panel — appended to the host and positioned against
details' rect, because `overflow: hidden` clipped anything wider than its parent — and that cost a
rect/focus/minimize/open subscription quartet plus a mirrored z-index to re-derive what a panel
already knows about itself. A full-width panel removes the reason for all of it.

The one thing that still escapes its panel is the **condition tooltip**: the card carries no text,
so the tooltip is the entire read surface, and a panel docked one row tall would clip it.

## Invariants a change must not break

1. `edgeX[58] === innerWidth` and `edgeY[33] === innerHeight` — the tables tile exactly.
2. `nearestCell(project(r)) === r` for every cell rect in the field — the projection round-trips.
3. A resize round-trip leaves every panel's four integers unchanged.
4. No panel occupies row 0 or row 32; a titled panel's body starts at row ≥ 2.
5. Toggling a title bar leaves the body's `getBoundingClientRect()` bit-identical.
6. Placement writes all four CSS edges in **one** pass, and fires `rectChange` on every path —
   grid change, migration, title toggle, drag end, resize end, snap, reset. Consumers that
   mirror a panel's rect (the world canvas) have no other notification.
7. No stored panel geometry is a pixel string, and no component hardcodes a pixel font size.
8. `background` reaches title bar, body and footer together — never one surface alone.
9. A click-through panel's interactive children set `pointer-events: auto` explicitly; they
   inherit `none` from the root and go silently dead otherwise.
10. Each selection surface subscribes to the selection model itself. No selection panel feeds
    another.
