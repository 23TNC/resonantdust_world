# Completed — selection-panels

## 2026-08-09 — P0: the paper

`design/panel-layout.md` grew two sections and three invariants.

**"Appearance and interaction are per-panel options"** names a third category beside the layout /
scale split that stream already established: layout comes from the projection, scale from
`--ui-row`, appearance and interaction from per-panel options. It records that `background` is a
named string (`chrome` / `dim` / `none`) reaching all three chrome surfaces, that `clickThrough`
is `pointer-events: none` on the root with children needing to opt back in, that the two are
independent and must stay so, and the consequence that matters operationally — a bar-less
click-through panel has no grab handle and depends on edit mode forcing bars visible.

**"The three selection surfaces"** is the table: details = identity, intentions = the queue,
conditions = the cards, each subscribing to the selection model itself. It records *why* the split
exists rather than just asserting it — the strip used to live outside its panel, positioned
against another panel's rect, because `overflow: hidden` clipped anything wider than its parent —
and notes the one thing that still escapes its panel: the condition tooltip, since the card
carries no text.

Invariants 8–10 added: `background` reaches all three surfaces together; a click-through panel's
interactive children set `pointer-events: auto` explicitly or go silently dead; no selection panel
feeds another.

Verified: `bin/rd docs-check` clean (706 files, the 5 standing warnings).

## 2026-08-09 — P1: the two options

`BackgroundMode` (`chrome` / `dim` / `none`) and `clickThrough` land as per-panel options through
the full ritual: option → JSON field → field + read → getter/setter → storage → popup row +
`rowsByKey` → `serializeState` → `resetToDefaults`. `PanelSettingKey` gained both keys, which is
where the compiler caught the last step. One resolver, `backgroundCss`, is the only place a mode
becomes a colour — so a future `#rrggbb` value is an additive change there and nowhere else.

`applyBackground` writes to the **title bar, body and footer together** (I9). Painting only the
body would leave an opaque bar over a transparent panel, which reads as a rendering fault rather
than a setting.

**Verified live**, edit mode, on a real panel through the popup's own controls:

- Cycling Background `Chrome → Dim → None` turned the bound panel's title bar **and** body to
  `rgba(0, 0, 0, 0)` together, left every other panel at `rgba(20, 22, 30, 0.96)`, and persisted
  as `worldViewport.background = "none"`.
- Click through set the root to `pointer-events: none`, kept the title bar at `auto`, and a
  synthetic hit test at the body's centre resolved to `#app` — the click genuinely leaves the
  panel.
- Both survived a reload, and on a panel with a **visible** bar (chat) the decisive pair held
  together: `BODY_PASSES_THROUGH: true` **and** `TITLE_BAR_STILL_GRABBABLE: true`. That is the
  property the whole design leans on — a click-through panel is not stranded.

Two earlier hit tests read as failures and were **invalid measurements**, not defects: the world
viewport and details both carry `titleBarHidden`, so their bars have a zero-size rect and the
probe point landed on the taskbar. Worth recording because it is the second time in two streams
that a `[0,0,0,0]` rect has impersonated a bug.

### The plan error (I10)

"`PixiPanel`'s private `transparent` body override retires in favour of the option" is **void**:
there is no `PixiPanel`. It died with the pixijs client on 2026-07-28, leaving **39 stale
comment references** across four files describing a class that has not existed for weeks — and
those comments were the evidence this stream's README cited for "click-through already exists,
undeclared". The pattern they describe is real and is exactly what F2 generalizes; only the actor
is fictional. `ViewportPanel` shows the world by hosting the viewport's own `<canvas>` inside its
body, so nothing overrides a background anywhere (`grep 'background = "transparent"'` → nothing).

The ten references in `DomPanelStyles.ts` were rewritten to describe the **option**, because they
are the standing justification for the `pointer-events: auto` this stream depends on and leaving
them is how the next session repeats the mistake. Comment text only. The remaining references in
`DomPanel.ts`, `PanelManager.ts` and `PanelSettingsPopup.ts` are flagged for a separate sweep.

## 2026-08-09 — P2/P3/P4/P5: the split lands

These four phases are one commit because the file moves make them inseparable: once
`ConditionCards` and `IntentStrip` move out of `details/`, details cannot compile until it stops
importing them.

**Folders follow the split.** `git mv` put `ConditionCards` under `panels/conditions/` and
`IntentStrip` under `panels/intentions/`, beside their new owners. Leaving them in `details/`
would have kept the hijack in the directory tree after removing it from the code.

**`ConditionCards` is a child.** Its `CardsHost` contract went from eight members to **one**
(`storageKey`, for the expanded flag). Deleted: the rect / focus / minimize / open subscription
quartet, the window-resize listener, the mirrored z-index, the visibility predicate, the
`position: fixed` placement, `INTENT_STRIP_W`, `EDGE_MARGIN`, and a 45-line `reflow` that chose
between "slide left so the strip still fits" and "become a viewport-wide scroller". `reflow` is
now one line — a visibility flip — because a child is laid out by its parent.

**`ConditionsPanel`** subscribes to the selection model itself, polls at the same 500 ms details
always used, and guards the rebuild with an identity key over `(id, remaining, magnitudeSum)` so
a poll that changes nothing cannot tear down a tooltip mid-read (I8). **`IntentionsPanel`** takes
`IntentStrip` unchanged and carries the `CANCEL_INTENT` sender.

**`DetailsPanel` is identity only**, and both reserves went with their tenants (I7) — the
`CARD_H + PAD_BOTTOM * 2` bottom band and the `STRIP_W` left shift. The scene builds ONE
`selectionProviders` object shared by details and conditions; that is the scene's data access,
not a panel feeding a panel.

**Verified live**, clean profile, five real pawns in the world:

- Selecting a pawn fills all three: details `bunny tile 123, 79`, conditions **4 cards** at
  34×34 running left-to-right (x = 8, 48, 88, 128), intentions showing its queue circle.
- The conditions panel measures `0,778,1862,83` — **full width**, `pointer-events: none`,
  body background `rgba(0, 0, 0, 0)`, no title bar.
- **The decisive pair holds**: a click on a card resolves inside the panel
  (`CARD_IS_CLICKABLE: true`, hit stack `DIV(in conditions)` → `CANVAS`), while a click on the
  empty area beside the cards reaches the world (`gapHitIs: "CANVAS"`). Transparent and
  click-through with live cards — the thing F2 argued could not be expressed by one flag.
- Card counts track the pawn: 4, 2, 2 and 0 across the five movers. The zero is a real pawn with
  no active conditions, not a wiring failure.

**Two smaller things found by running it.** The panel-strings schema keys the title-bar text as
`label`, not `title`; my first locale entries used `title` and the panels rendered their raw keys
(`conditionsPane…`) until corrected. And a card-click probe first read as a failure because the
**Build** panel (z 480001, spanning x 0–289) covers the left end of the card band — panels are
allowed to overlap (panel-grid F11), and conditions sits on `Z_TIER_INFO` beneath the tool tier,
so where they overlap the cards are hidden. Worth the user's eye when they position it.

## 2026-08-09 — P2/P3/P4/P5 remainder: tooltip, over-width, wash, taskbar

**The tooltip already escapes (I2)** — no change needed. Hovering a card raised
`"Sprinting +3 Motivated  priority 45  60t remaining"` at z 640011 with `parentIsPanel: false`:
it was always a fixed-position element appended to the host, the `IntentStrip` pattern. The
irony noted at planning held exactly — F3 deleted the overlay for the *cards* while the
*tooltip* is the one part that genuinely still wants to escape a short panel's clip.

**Over-width is WRAP, not scroll (I3).** Pinning 60 synthetic cards through the project's own
`__cards(n)` debug hook: all 60 stayed inside the panel across **2 rows**, with
`scrollWidth === clientWidth` — no horizontal overflow at all. Wrap is also the only answer that
works here: a click-through panel has no pointer events with which to grab a scrollbar, so a
scroller would be unreachable by construction. At full width a row holds ~46 maximized cards, so
the collapsed mode (top 4 maximized) has nothing left to solve — its constants were sized against
details' ~355px and are simply slack now.

**The emotion wash composes with the option (F6/F1).** Measured on the same pawn over both
backgrounds: `rgba(106, 58, 216, 0.16)` present over `chrome`, and still present over `none`
with the body and title bar at `rgba(0, 0, 0, 0)`. The coloured-ghost reading F1 predicted is
what it does.

**Details' min size re-read (I7)**: 220×150 → 160×90. The old floor was sized around the intent
column and the card band; neither is its content now. Both are a last-resort floor regardless —
the real minimum is the cell law's 6×3 body.

**Taskbar entries — a real miss, caught by testing.** Neither new panel appeared in the bar.
Cause: I seeded both corpus entries from `details`, which carries `pin: "none"` — and `pin` is
what routes an entry into a bar, so details has no entry either. For a panel with **no title
bar** that is not cosmetic: a closed conditions panel would have had no way back at all. Fixed to
`pin: "bottom-left"`; verified ☯ and ⚙ both present, and a taskbar cycle restores the panel to
exactly `0,778,1862,83`.

**The progress ring is unobserved, not verified.** `IntentStrip` moved byte-identical (a `git mv`
plus its new panel), and the strip demonstrably renders and tracks selection — 1 circle for each
pawn holding 1 queue entry, 0 for pawns with none, across all five movers. But every live entry
in the world right now is a walking NPC's `move_to`, and none carries an authored progress ring:
`ringPercent()` returned `null` for all five and no `<svg>` was emitted. So the per-frame ring
maths could not be exercised. Recorded as inspection, not observation — the same treatment
panel-grid's I7 got — and it wants a look the next time a pawn runs a duration interaction.

## 2026-08-09 — P6: the exit

**I4 answered YES — the panel ships bar-less.** Entering edit mode on the conditions panel
reveals its title bar (`display: flex`, `pointer-events: auto`) AND grows the outer box upward by
a row (`0,778,1862,83` → `0,750,1862,111`) so the body holds still — the panel-grid body-invariant
law doing its job on a panel that did not exist when it was written. Dragging by that revealed bar
moved the panel from `0,750` to `0,528` and persisted `col=0 row=20 cols=58 rows=3`. So a
transparent, click-through, bar-less panel is reachable and movable, and the default authored in
the corpus is safe.

Worth knowing: the bar is only hittable where no *other* panel covers it. The first probe landed
under Chat (z 480002, spanning x 0–514) and read as a failure. Which is the second half of the
same observation recorded at P2 — see below.

**The full exit, on camera** (capture in this session; clean profile, defaults reset):

- Selecting a wolf fills three panels: **details** shows `wolf` / `tile 116, 59` with the active
  emotion wash tinting it purple, **intentions** shows its queue circle in its own titled panel,
  and **conditions** shows four emotion-pie cards along the bottom with the forest visible
  between and around them — no background at all.
- Both new taskbar entries (☯ ⚙) present; the conditions panel restores to `0,778,1862,83` after
  a taskbar cycle.
- World clicks pass through the conditions panel's empty area to the canvas while its cards stay
  clickable and their tooltips render outside the panel's clip.

**One thing left for the user's judgement, not fixed here.** Conditions sits on `Z_TIER_INFO`
(40), below the tool tier (48) that Chat and Build occupy — and both of those default to the
bottom-left, directly over the left end of the full-width condition band. Where they overlap, the
cards are hidden behind an opaque panel. Overlap is explicitly allowed (panel-grid F11) and the
user said they would position the conditions panel themselves, so this was left as authored
rather than silently re-tiered. If it wants changing, the question is whether a full-width bottom
HUD belongs *above* the tool panels — a one-line `zOrder` change in `ConditionsPanel`.

## 2026-08-09 — post-review: the `outline` option, and the minimum-size complaint

**`outline` per-panel option** (user request). Same ritual as `background` / `clickThrough`, so
`PanelSettingKey` again caught the last step. `PANEL_OUTLINE` is extracted as the colour, and
`applyOutline` swaps it for **`transparent`** rather than setting `border: none` — deliberately.
The panel is `box-sizing: border-box` with its outer size projected from cells, so `none` would
hold the outer rect but silently give the BODY 2px in each axis, moving every rect-mirroring
consumer (the world canvas) on what is meant to be a cosmetic toggle. Verified: border cycles
`rgb(58,58,74)` → `rgba(0,0,0,0)` → back with the panel's rect **bit-identical** throughout.

**"I cannot reduce the size of the intentions panel — something is enforcing a minimum."**
Correct, and it was mine: panel-grid put a global 6 × 3 cell floor on every panel, enforced in
**three** places at once —

1. `PANEL_CSS.minWidth: calc(var(--ui-row) * 6.25)` — a CSS floor, which no amount of setting
   `style.width` can beat and which cannot be overridden per panel;
2. `MIN_COLS = 6` / `MIN_ROWS = 3` static constants feeding `clampCell`;
3. the resize helper's px floors — which also computed the X floor as `MIN_COLS × rowHeight`,
   multiplying a COLUMN count by a ROW height. Cells are not square off 16:9, so that number was
   wrong as well as too big.

At ~32px cells the floor made every panel at least ~190px wide, which is exactly why a narrow
vertical queue strip could not be narrow. Fixed by making the floor **per-panel** (`minCols` /
`minRows` options) with the default dropped to **2 × 2**: a floor should stop a panel collapsing
to nothing, not decide its shape. The CSS floor is deleted outright — the cell clamp is the law,
and a global CSS minimum silently won every argument with it. The px floors now project from the
panel's own cell floor, using the column width for X.

Verified: the intentions panel resizes from 4 × 11 cells down to **2 × 2** (64 × 83px), and the
computed CSS `min-width` reads `0px`.

## 2026-08-09 — the minimum becomes a SETTING, defaulting to 1×1

User: _"Make the minimum settable in settings, and default to 1x1. I havent the foggiest why you
pulled 2x2."_ Fair — 2 × 2 had no justification. It replaced panel-grid's global 6 × 3 (which was
"roughly the old 200 × 100px floors") with a different arbitrary number, and both were the code
deciding the user's layout.

**1 × 1 is the only non-arbitrary floor**: one cell is the smallest rect the grid can express.
Anything above it is an opinion, so it now belongs to whoever holds the opinion — the panel that
genuinely needs room says so in its constructor, and the user overrides either from the settings
popup.

`minCols` / `minRows` became mutable per-panel state through the full option ritual (option →
JSON → `defaultedInt` read → getters + `setMinSize` → storage → popup row + `PanelSettingKey` →
`serializeState` → `resetToDefaults`). `setMinSize` clamps to `[1, GRID_COLS]` / `[1, FIELD_ROWS]`
and — the one non-obvious bit — **re-places the panel when the new floor exceeds its current
size**, so a panel can never sit below its own stated minimum.

The popup row reuses the existing stepper vocabulary: `◀ ▶` for columns, `🞃 🞁` for rows, with the
live value between them (`Min size  ◀ ▶ 2×2 🞃 🞁`).

**Verified live**: with the default floor, the intentions panel resizes down to `1,1` cells
(32 × 28px) — the complaint that started this. The stepper walks `1×1 → 2×1 → 2×2`, clamps at
`1×1` when driven down, and persists `minCols` / `minRows` per panel.

## 2026-08-09 — the intent strip fits a ONE-CELL-WIDE panel

User: _"The intentions panel will be 1 grid wide, so the icons it displays need to fit inside of
it without creating scroll bars."_ It could not: `IntentStrip` claimed a fixed
`width: 40px; flex: 0 0 40px` column with a 34px circle box (26px dot + 3px ring + 2), all sized
for living inside the details panel. A one-cell panel is ~31px of content, so both overflowed and
the body's `overflow: auto` grew a bar — which in a 31px panel is most of the panel.

**The strip is fluid now.** It takes `width: 100%` instead of a fixed column, and a circle is
`width: min(100%, calc(var(--ui-row) * 1.2 * size)); aspect-ratio: 1`. The `min()` is the whole
fix: the circle takes the panel's width when the panel is narrow and its authored size when there
is room, so it can never exceed its container by construction rather than by a measurement that
has to be redone on every resize. The TOML's `queue.size` now scales the **cap** rather than an
absolute pixel count.

Everything inside the circle moved to a fixed **100 × 100 viewBox** scaled by CSS, preserving the
old proportions exactly (26/34 dot, 3/34 stroke). That keeps the ring's maths resolution-
independent, and the per-frame driver is untouched — it reads `stroke-dasharray` off the attribute
and writes `stroke-dashoffset`, both now in viewBox units, so the contract that the ring is
COMPUTED every frame rather than incremented still holds. `STRIP_W` is deleted.

The panel's holder is `overflow: hidden`, deliberately: vertical overflow (more queued intents
than the panel is tall) is a sizing choice, not something to grow a bar for.

**Verified live** at 1 cell wide (panel 32px, content 31px): the strip measures 31px, the circle
20px, and `scrollWidth === clientWidth` on **both** the body and the holder — no scrollbar in
either axis. At 4 cells (127px strip) the circle stays 20px rather than stretching, so the cap
still governs when there is room.

**One thing the user will want to decide.** At one cell the title bar is unusable — it reserves
2.5 rows (70px) on its right for the minimize/close buttons, which is wider than the whole panel.
The corpus currently authors `titleBarHidden: false` for intentions; at this width it wants
`true`, with edit mode as the way back (the same arrangement the conditions panel uses, proven at
P6). Left as authored rather than changed unilaterally.

## 2026-08-09 — layer becomes a NUMBER, and starts working

User: _"Please add layer as a number to the settings. I tried changing layer but was unable to
get the panels to behave as expected."_ The readout was the smaller half; the control was broken
in two ways that together made it look inert.

**What it did.** `_zOrder` (the tier) was `readonly`, fixed at construction. `layerUp` /
`layerDown` nudged `style.zIndex` by ±1 **within** that tier's 10000-wide band, clamped to it, and
persisted **nothing**. So:

1. **Any change was reverted by the next click.** `bringToFront` re-seats a focused panel with
   `nextTierZ(this._zOrder)`, which recomputes from the band base — so focusing any panel threw
   the nudge away. You could only ever see the effect until you touched something.
2. **It could not cross a tier.** A panel authored on 40 was clamped to `[400000, 409999]`, so no
   number of clicks could raise it above one authored on 48. This is exactly the case flagged at
   P6: conditions (40) buried under Chat and Build (48), unfixable from the UI.
3. **No feedback.** Two arrows and no value — nothing to tell you whether a click had done
   anything, or where a panel sat relative to another.

**What it does now.** The layer IS the tier, mutable, persisted under `<key>.layer`, and shown as
a number between the steppers. Each integer is a full band, so **one step genuinely moves a panel
above everything in its old layer** — the control is coarse on purpose rather than by accident.
Clamped to `[1, 63]`: 64 is `Z_TIER_CHROME`, where the taskbars and tooltips live, and a panel
that climbed into it would cover the chrome used to manage it. `bringToFront` still applies
focus-recency, but now within the panel's *chosen* band, so focus and layer stop fighting.

**Verified live**: stepping reads `32 → 33 → 34 → 35`, the z-index follows to `350001`, the value
persists, and it survives focusing another panel — the exact case that used to lose it. Then the
case that motivated it: setting `conditions.layer = 50` puts the conditions panel at `z=500001`,
**above** Build (`480001`) and Chat (`480002`), with its cards hittable where they were previously
buried. The P6 note about conditions being stuck under the tool tier is now a user setting rather
than a code change.

## 2026-08-09 — background becomes an OPACITY

User: _"Can we replace background with opacity instead of chrome dim and none?"_ Yes — the enum
was only ever three samples of one scale (`chrome` 96%, `dim` 55%, `none` 0%).

`backgroundOpacity` is an integer 0–100, default 96, stepped in 5s from a popup row that shows
the percentage. `CHROME_BG` split into `CHROME_RGB` (the colour) plus an alpha, so `backgroundCss`
is the single place the two meet — which is what made this a small change rather than a sweep.
`BackgroundMode`, `VALID_BACKGROUNDS`, `readBackground` and the cycling-select row are deleted.

Storage schema bumped to **v3** with `background` added to the superseded list, so the old enum
strings are swept on first boot rather than lingering as unread keys.

**Verified live**: stepping reads `96% → 91% → 86% → 81%` with the body's computed alpha tracking
exactly (`0.96 → 0.91 → 0.86 → 0.81`); it clamps at `0%` where both the body AND title bar go
`rgba(0, 0, 0, 0)` together; `5%` gives `rgba(20, 22, 30, 0.05)`. After a clean reload the schema
stamp reads `3`, no `.background` keys remain, and the conditions panel is still transparent and
click-through from its corpus entry.
