# Forks — selection-panels

## F1 — background is a per-panel option over NAMED values, not a free colour

2026-08-09, user: _"provide background color as an option"_. Delivered as a cycling select over
named values — `chrome` (today's `rgba(20,22,30,0.96)`), `dim`, `none` — persisted like every
other per-panel setting, with `none` meaning genuinely transparent.

Named rather than free, for three reasons that all point the same way. The popup's entire
vocabulary is toggles and cycling selects; a colour input would be the first of its kind and would
need its own row type, serialization and reset semantics. The corpus (`defaults.json`) is
hand-edited JSON, and `"background": "none"` survives review where `"#1a1c22f5"` does not. And
what this stream actually *needs* is one value: transparent, for the conditions panel. A palette
is a theming feature; shipping the enum now leaves the free-colour door open (the option is a
string, so a future `#rrggbb` value is an additive change to one resolver) without inventing UI
nobody has asked to use yet.

Note it composes with the emotion wash (F6): the wash is a translucent overlay ON the background,
so `none` + wash is a coloured ghost rather than nothing. That is the correct reading of both
features and wants a look on camera.

Rejected: a colour picker per panel (a theming system, disproportionate to "make this one
transparent"); hardcoding transparency for the conditions panel only (leaves `PixiPanel`'s
existing override as a second, private path to the same thing — exactly the duplication this
option removes).

## F2 — click-through is its own option, NOT implied by a transparent background

2026-08-09. The user's sentence couples them — "without a background so we can click through it"
— and the conditions panel's defaults will indeed set both. They are still two options, because
they are two mechanisms and this stream needs them decoupled inside a single panel.

The conditions panel must pass world clicks through **while its own cards remain clickable**
(a card expands the strip and raises a tooltip). That is `pointer-events: none` on the panel root
with `auto` on the cards — precisely the pattern `PixiPanel` already runs, where the body falls
through to the canvas while the title bar, tabs and action buttons stay live. If transparency
implied click-through there would be no way to say "transparent but interactive", and no way to
say "opaque but click-through" either; both are coherent panels.

Rejected: one `transparent` flag doing both (cannot express the panel this stream is for);
deriving click-through from `background === "none"` (a hidden coupling between two settings the
user sees as separate rows, and it would make the cards' `pointer-events: auto` look like a bug).

## F3 — the conditions panel is FULL-WIDTH along the bottom

2026-08-09, user: _"I'll position the conditions panel along the bottom… so that the conditions
can extend left to right inside the panel without issues."_ The default cell rect spans the
field's full width on the bottom rows.

This is the fork that retires the sibling hack. `ConditionCards` exists outside its panel *only*
because a strip wider than the details panel would be clipped by `PANEL_CSS`'s `overflow: hidden`;
give the strip a panel as wide as the screen and the clip stops being a constraint. The cards
become ordinary children, and the rect/focus/minimize/open subscriptions, the mirrored z-index and
the hand-rolled visibility predicate all delete — a panel already knows all of that about itself.

Positioning is the USER's (they said "I'll position"), so the default is a starting point, not a
law; it is authored in cells like every other panel and they can drag it.

Rejected: keeping the overlay and merely re-parenting its owner (preserves the whole subscription
quartet for no gain); a fixed-height strip docked outside the panel system (a fourth way to place
a rectangle, in a codebase that just spent a stream getting to one).

## F4 — the intentions panel keeps the vertical column for now

2026-08-09. `IntentStrip` renders a vertical column of circles with an SVG progress ring driven
per frame from the learned tic estimate — proven code with a subtle timing contract (the ring is
COMPUTED every frame, never incremented, so a hidden tab snaps to truth). It moves into the new
panel unchanged.

A strip that reflows to its panel's aspect (horizontal in a wide panel, vertical in a tall one)
is the obvious successor and is deliberately deferred: it changes the layout of a surface whose
correctness currently depends on that per-frame ring maths, and this stream is a re-housing. Ship
the move, then reflow.

Rejected: rewriting the strip horizontally now (couples a layout change to a re-housing, so a
regression in either is hard to attribute).

## F5 — each panel subscribes to the SELECTION MODEL directly

2026-08-09. Both new panels take `SelectionModel` + the providers they need and subscribe
themselves, rather than being fed by `DetailsPanel`. Details is losing tenants precisely because
it was the wrong owner; making it the *broadcaster* for its former tenants would keep the coupling
while removing the evidence of it. Each panel also owns its own refresh cadence (details polls at
500 ms because the selection event fires on selection changes, not on a pawn's motion).

Rejected: a shared "selection view" mediator (a third object to explain, for two subscribers);
details re-emitting to its former children (keeps the hijack, hides it).

## F6 — details keeps identity AND the emotion wash

2026-08-09. After the move, `DetailsPanel` is name + tile + the [Inventory] button + the emotion
wash. The wash stays: it is a property of the *selected object* rather than of its conditions
list, it is the panel's only non-textual identity signal, and moving it would leave details a bare
two-line text box.

It does need naming here because it is the one place F1 gets interesting: the wash is a
translucent colour laid over the panel background, so it composes with whatever `background`
resolves to. `background: none` + wash is a coloured ghost — deliberate, and worth a look.

Rejected: moving the wash to the conditions panel (the wash is the ACTIVE emotion, an argmax over
the pawn — it is identity, not a list); dropping it (it survived two streams as the panel's only
colour).
