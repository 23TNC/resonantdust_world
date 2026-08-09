# Forks — survival

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — `geo_label` is a thing-def string, defaulted from the name {#f1}
_2026-08-08 · resolved at plan time_

**Chosen.** `[[thing]]` gains optional `geo_label = "…"`; absent = the
name's FIRST CHARACTER, uppercased (bunny → B, logs → L, debug_torch → D).
Multi-character strings are legal (the field is a string per the user) and
render scaled-to-fit; authoring collision disambiguation (bunny vs bear)
is a content decision made when it bites, not a uniqueness law.

**Rejected — tiles too**: the user said objects (things); ground tiles are
identified by terrain color and never confused the way flat squares were.
**Rejected — auto-disambiguation (Bu/Be)**: invents labels nobody authored;
the default is a default, the field is the override.

## F2 — the glyph rasterizes client-side, once per (glyph, color) {#f2}
_2026-08-08 · resolved at plan time_

**Chosen.** An offscreen-canvas rasterize of the label (plain bold sans,
high-contrast against the geo tint — white with a dark outline so it reads
on any color), cached per (label, size-bucket), drawn CENTERED over the
geo/placeholder box — "printed in the center of the albedo" implemented at
the geo bake seam, so the letter participates in the same draw the box does
and vanishes the moment real art arrives.

**Rejected — authoring glyph textures through the art pipeline**: a letter
is exactly what canvas text is for; the art pipeline is for art.
**Rejected — DOM overlays**: the label must live IN the world draw (zoom,
occlusion, lighting) or it reads as UI.

## F3 — the condition applies a DEPLETION MODIFIER to the need {#f3}
_2026-08-08 · the user's simplification of this fork's first draft, adopted_

**Chosen.** The condition's need-modifier carries **`deplete`** — the SAME
field, units and meaning the need itself authors (tics full→empty): "while
this condition is active, the need depletes at this pace."

    needs = [ { need = "corpus", deplete = 3600 } ]

No new vocabulary (the first draft invented a `drain` lane — the user's
review collapsed it: this IS just a depletion modifier). Multiple active
sources combine as RATES SUMMING (starving + dehydrated = faster than
either alone). The piecewise lazy eval extends in ONE place; the re-stamp
law already covers grant/expiry; band-derived conditions enter exactly as
today. Corpus keeps its own `deplete 0` — "only events and condition
depletion move it", stated in VARIABLES. The trait per-level modifier form
inherits the field for free.

**The constraint that forced ANY new field**: the existing `rate` is a
MULTIPLIER on the need's base deplete, and corpus's base is 0 — a
multiplier alone can never move it, and giving corpus a base would tick
for healthy pawns. **Rejected — a worker tick that decrements corpus**:
violates the lazy-value law root and branch.

## F4 — the zero-crossing scheduler: a re-validating re-stamp, not a timer {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** Death today fires from the need-write sweep when a WRITE leaves
corpus ≤ 0. A lazy drain crosses zero silently — so after any event that
changes a pawn's corpus TRAJECTORY (a SET_NEED write, a mint, and grant/
expiry re-stamps), the worker computes the pawn's corpus zero-crossing via
the shared `next_crossing_tic` machinery and queues a SET_NEED RE-STAMP at
that tic (+ the barrier margin, the standing queue_at law). The re-stamp
RE-VALIDATES at fire — it writes the freshly-evaluated value, so a pawn
that ate meanwhile just gets an honest re-stamp and a NEW crossing gets
scheduled; a pawn still at ≤ 0 triggers the EXISTING sweep → `can_die` →
meat + remove. One mechanism, no new verbs, idempotent by content.

**Rejected — a worker-side wall-clock watch list**: worker memory dies with
the process; the queue is durable. **Rejected — the client/npc reporting
crossings**: clients speculate, the server decides — the authority law.

## F5 — dev-scale drains, authored on the conditions {#f5}
_2026-08-08 · resolved at plan time — numbers are the user's to retune_

**Chosen.** `starving` authors a corpus drain, `dehydrated` likewise —
watchable in a drill (minutes, not hours: full corpus → dead in ~3600 tics
≈ 10 min under either; both active = twice the pace by the summing law).
Authored on the CONDITIONS, so every species with the bands inherits the
mortality — wolves, bunnies, humans — with zero per-species content. The
drill pens target wolves and bunnies per the user; humans are asserted
inheriting (the corpus rows and bands are shared) without their own pen.

**Rejected — per-species drain tuning now**: nothing needs it yet; the
leveled-trait machinery is sitting right there the day it does.
