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
