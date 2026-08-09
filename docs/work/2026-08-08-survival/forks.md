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

## F3 — conditions gain a per-need `drain` lane, summing, in the ONE eval {#f3}
_2026-08-08 · resolved at plan time — the user's "decrease the corpus need over time"_

**Chosen.** `NeedModifier` (conditions now; the trait per-level form
inherits the field for free) gains `drain` — authored in DEPLETE UNITS
(tics full→empty contributed by this source alone; the need's own
`deplete` keeps meaning the base). Active drains SUM (two sources drain
twice as fast). The piecewise lazy eval in `needs_eval` extends in ONE
place; the re-stamp law already covers rate changes at grant/expiry, and
band-derived conditions (starving IS one) enter the eval exactly as today.
Corpus keeps `deplete 0` — the design's "only events move it" softens to
"only events and authored drains", stated in VARIABLES.

**Why not the multiplicative `rate`**: rate × base-deplete is zero forever
on a deplete-0 need — the additive lane is the only way starving can move
corpus without giving corpus a base drain that would tick for HEALTHY
pawns. **Rejected — a worker tick that decrements corpus**: violates the
lazy-value law root and branch.

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
