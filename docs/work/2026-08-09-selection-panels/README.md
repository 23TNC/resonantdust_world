# selection-panels — intentions and conditions get their own panels; background becomes an option

_User (2026-08-09): "We are going to provide background color as an option. I've been hijacking
details as the main UI element. So, we are going to make an intentions panel that will display
the intentions of a selected object. We are going to make a conditions panel that will display
the conditions of the selected object. We will remove intentions and conditions from the details
panel. I'll position the conditions panel along the bottom without a background so we can click
through it but also so that the conditions can extend left to right inside the panel without
issues."_

"Hijacking details" is the diagnosis, and the code agrees with it in writing.

## What this dissolves (surveyed against the delivered state)

- **The condition strip is a SIBLING of the details panel, not a child** — and its own header
  says why: _"`PANEL_CSS` sets `overflow: hidden` on the panel root, so anything parented inside
  `DetailsPanel` is clipped at the panel border… The user asked for cards that 'actually draw
  past the details', so this element is appended to the panel's HOST and positioned against the
  panel's rect."_ That is a panel escaping its own panel. It costs a rect/focus/minimize/open
  subscription quartet, a mirrored z-index, and a manual visibility predicate
  (`isOpen && !isMinimized && style.display !== "none"`) — all of it re-deriving, by hand, things
  a real panel gets for free. A full-width conditions panel deletes the entire mechanism: cards
  laid out left-to-right *inside* a panel that is already as wide as the screen never need to
  escape a clip.
- **The details body pays rent for both tenants.** It reserves a bottom band
  (`padding-bottom: CARD_H + PAD_BOTTOM * 2`) so its text scrolls to a stop above cards that
  aren't its children, and its content is shifted right by `STRIP_W = 40` for the intent column.
  Both reserves are pure consequence of the hijack; both go.
- **Details' stated job is already the smaller one.** Its header records the user's earlier
  ruling — _"I just need to know what something is"_ — and the panel is documented as
  IDENTIFICATION: name + tile. Everything else in it arrived because there was nowhere else to
  put it.
- **Background is a hardcoded constant.** `CHROME_BG` is baked into `TITLEBAR_CSS`, `BODY_CSS`
  and `FOOTER_BG`; `PixiPanel` already has to *override* the body to `transparent` to show the
  canvas. So "transparent background" exists as a private hack for one subclass and is unavailable
  to the corpus. Making it an option generalizes what one panel already does.
- **Click-through already exists too, undeclared.** `TITLEBAR_CSS`, `ACTIONS_CSS` and `TABS_CSS`
  each carry `pointerEvents: "auto"` with a comment explaining that `PixiPanel` sets
  `pointer-events: none` on the panel root. The chrome-stays-live-while-the-body-falls-through
  pattern is built and proven — it just isn't a setting.

## The stance

- **Two new panels, each subscribing to the selection MODEL directly** (F5) — not chained through
  details. Removing a tenant from details must not be able to break the tenant.
- **Background is a per-panel option with a `none` value** (F1). Named values rather than a free
  colour picker: the popup's vocabulary is toggles and cycling selects, the corpus is JSON, and
  what this stream actually needs is *transparent*. The free-colour alternative is recorded, not
  built.
- **Click-through is a SEPARATE option from background** (F2). The user coupled them in one
  sentence and they will be coupled in the conditions panel's defaults — but they are two
  mechanisms, and the proof is inside this very stream: the conditions panel must be click-through
  *while its cards stay clickable*. Transparent-but-interactive and opaque-but-click-through are
  both coherent; one flag couldn't express either.
- **The conditions panel is full-width along the bottom** (F3), which is what makes
  "extend left to right without issues" true by construction rather than by escaping a clip.
- **The intentions panel keeps the proven vertical column** (F4) — its layout adapting to the
  panel's aspect is a named successor, not this stream's work.
- **Details keeps identity**: name, tile, the [Inventory] button, and the emotion wash (F6 —
  the wash is the selection's *identity* tint, and it is also the one thing that has to compose
  with a configurable background).

## Watch

A click-through panel cannot be grabbed by its body, and the conditions panel wants no title bar
either — so the only way to move it is **edit mode**, which forces title bars visible ([I4](issues.md#i4)).
That is a real dependency on behaviour this project only just made true (panel-grid, 2026-08-09),
and it wants checking on camera before the panel ships with no bar by default. The card tooltip
is the entire read surface for a condition (emotions F6) and must escape a one-row-tall panel's
clip ([I2](issues.md#i2)). And both new panels need corpus entries and cell rects, or they open
wherever the grid's fallback puts them ([I6](issues.md#i6)).

## Not in this stream (named, not built)

A free-colour picker per panel (F1); the intentions panel reflowing to its panel's aspect (F4);
moving the emotion wash out of details (F6); any change to what a condition or an intent *is* —
this is a re-housing of existing surfaces, not a gameplay change.

## Exit

Selecting a pawn fills three panels: details says what it is, intentions shows its queue,
conditions shows its cards left-to-right along the bottom. The conditions panel has no background
and passes clicks through to the world while its own cards and tooltips stay live. Details no
longer reserves a bottom band or a left column, and `ConditionCards` no longer subscribes to
another panel's rect. Background and click-through are per-panel options that survive a reload.
The user's eyes close the stream.
