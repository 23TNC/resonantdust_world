# Forks — live-edit

## F1 — the live preview gets a SPIKE before it gets a design

2026-08-09. The user asked for "a live preview… that will function as a viewport with zoom and
pan". There are two ways to build that and they differ by about a week, so this stream **measures
before choosing**.

`Viewport` is not a light object. It owns a `Renderer` (and therefore its own **WebGL2 context**),
the white/empty `Texture`s, a glyph-frame cache, a `MaterialRegistry`, and a `TextureResolver`
that is explicitly bound to *its* GL context. Two consequences: a second instance means a second
context (browsers cap these, and the cap is not generous), and it cannot share the first's
textures — every atlas page resident twice.

The two candidates:

- **A: a second `Viewport` instance.** Zoom and pan come free (`Camera` already does both), the
  world renders correctly by construction, and the code is nearly nothing. Costs a GL context and
  duplicate texture residency.
- **B: a purpose-built object preview.** Draw the selected object's prims at a chosen scale onto
  a small canvas — much cheaper, but it re-implements a slice of the renderer and will drift from
  it (the thing this codebase has spent streams removing).

**The spike decides it**: instantiate a second `Viewport`, measure context creation, texture
residency and frame cost against the existing one, and read the numbers. A is preferred if it is
affordable — it is less code and cannot drift — and B is the fallback. Nothing downstream is
scheduled until the spike reports, because the tabs do not depend on it and the preview's shape
changes what "top center" even contains.

Rejected: choosing A now (a second GL context is exactly the kind of cost that is obvious in
hindsight and invisible in a plan); choosing B now (re-implementing rendering to avoid a cost
nobody has measured is how you get two renderers and one bug).

## F2 — trait and need COLOUR are authored, which makes them schema work

2026-08-09. The user: _"Each traits color will be defined in the toml"_ and _"The color of the bar
will be defined in the need toml."_ Neither exists. Emotions author `color` (required) and
conditions inherit theirs through the emotion pie — which is precisely why tabs 3 and 4 are cheap
and 1 and 2 are not.

Each colour is therefore the full authoring ritual: a field on the def in
`shared/content/src/loader.rs`, a wasm accessor beside `emotion_color`, a corpus pass over
`content/`, and the golden fixture re-blessed. Done for traits and needs alike, and done the same
way both times so the second is mechanical.

Colours are authored as `#rrggbb` strings like every other colour in the corpus (`emotions.toml`,
the interaction `queue` blocks), parsed to `u32` at load. Unauthored is not an error — it falls
back to a neutral, so a half-coloured corpus renders rather than refusing.

Rejected: deriving a trait's colour from a hash of its name (stable and free, but the user said
"defined in the toml", and generated colours cannot be made to mean anything); reusing the
emotion palette for traits (traits are not emotions, and the coincidence would break the moment
either list changed).

## F3 — the need's live RATE is derived, and it is the point of the tab

2026-08-09. The TOML authors `deplete` — a BASE, in tics from max to min. The rate a player cares
about is that base scaled by the product of `rate` multipliers contributed by active conditions
and traits (the corpus header spells this out: _"`rate` multiplies depletion (multipliers form a
product)"_). Nothing in the wasm surface exposes it.

It gets a first-class accessor returning the need's live `(value, min, max, rate)` together,
because those four are read as one thing and computing them separately invites three of them
being from one tic and the fourth from another. Sign convention follows the user: **negative =
losing, red; positive = gaining, green** — so the accessor returns a signed per-unit-time rate,
not a raw tic count, and the panel formats rather than computes.

This is the number that justifies the tab. A bar alone says a pawn is at 40% thirst; the rate
says whether that is fine or an emergency, and it is the only figure on the panel the client
cannot derive from data it already holds.

Rejected: showing the authored `deplete` (it is the base — it would read the same for a pawn
under a ×3 penalty as one without, i.e. exactly wrong when it matters); computing the product
client-side from condition rows (a second implementation of a corpus rule, which conditions F3
already ruled against for ordering).

## F4 — the panel binds to the selection, and re-binds when it changes

2026-08-09. `/edit` opens the panel against **the currently selected object**, and the panel then
follows the selection model like the other three selection surfaces
(`2026-08-09-selection-panels` F5) rather than freezing on whatever was selected when the command
ran.

Following is the better default: the panel is an inspector, the user is clicking around, and a
frozen inspector that silently describes something else is a trap. A pinned mode ("inspect THIS
one while I click elsewhere") is a plausible successor and is deliberately not built — it wants a
visible pin affordance, and inventing UI for it now would be guessing.

Non-pawn selections show the panel with empty tabs rather than refusing to open, matching how
details/conditions/intentions already behave.

Rejected: freezing at open (a stale inspector is worse than an empty one); refusing to open
without a pawn (`/edit` should always do something visible, or it reads as broken).

## F5 — ONE eval snapshot per refresh, sliced four ways

2026-08-09. Traits, needs, conditions and emotions all come from the same pawn at the same tic
through the same wasm eval. Four tabs each fetching independently would run the eval four times a
poll and could show four different instants in one panel.

So the panel takes one snapshot per refresh and hands slices to the tabs. Only the ACTIVE tab
renders; the others update on activation. That also bounds the cost of the heaviest tab (needs,
with a derived rate per need) to when it is actually visible.

Rejected: per-tab polling (four evals, four instants); rendering all four tabs always (the user
sees one).

## F6 — read-only this stream, and the panel is shaped so writing can arrive later

2026-08-09. Everything the user described is a display: squares, bars, labels, signed numbers.
The name (`/edit`, `live_edit`) plainly anticipates mutation, and the panel is laid out so it can
arrive — a preview that shows the consequence, tabs that already know how to address a specific
trait/need/condition — but nothing in this stream writes.

The reason is not caution for its own sake: a write surface over a read surface that is subtly
wrong produces edits the user did not intend, and the read surface here contains a derived rate
and an effective clamp that are both easy to get quietly wrong ([I3](issues.md#i3)). Get the
inspector right, watch it against a live pawn, then make it an editor.

Rejected: building set-a-need-value now (needs the SET_NEED verb, a permission story, and a
correct read surface first — three streams' worth in one).
