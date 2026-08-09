# Survival — geo glyphs, and hunger/thirst that kill — 2026-08-08

**What** (user, 2026-08-08): two features. (1) A string in the objects TOML
used when a thing renders as GEO — printed in the CENTER of the albedo;
default = the first character of the name (B for bunny, L for logs). (2) The
starvation path becomes lethal: the starving state DECREASES THE CORPUS NEED
OVER TIME, the death interaction fires when corpus reaches zero, and the
SAME for thirst — so wolves and bunnies must eat and drink to remain alive.
Humans inherit the mechanism (they share the needs and conditions); the
drills target wolves and bunnies (the user's call).

**Why**: the geo tier is anonymous tinted squares — placeholder-vs-real-art
misreads have burned sessions twice; a glyph makes every box self-naming.
And the survival loop closes the food chain: eating and drinking currently
relieve discomfort but nothing dies of neglect — the pressure that makes
wolves hunt and bunnies forage MATTER is the corpus drain.

**Design stance** (the mechanism, grounded in what exists):
- `geo_label` on `[[thing]]` ([F1](forks.md#f1)), default = the name's first
  character uppercased; the client rasterizes the glyph once per
  (glyph, color) and centers it on the geo/placeholder box
  ([F2](forks.md#f2)).
- The user's "starve interaction" maps to the existing STARVING CONDITION
  (the hunger band already grants it, derived at eval): conditions gain a
  per-need **`drain`** lane ([F3](forks.md#f3)) — authored in deplete units
  (tics full→empty from this source alone), summing across sources,
  evaluated in the ONE piecewise lazy eval. `dehydrated` authors the same
  on the thirst side. Corpus keeps `deplete 0`; only conditions move it.
- Death already fires when a corpus WRITE lands ≤ 0 (the need-write sweep +
  `can_die` → meat + remove). A lazily-draining corpus reaches zero WITHOUT
  a write — the missing piece is the **crossing scheduler**
  ([F4](forks.md#f4)): the worker queues a re-validating re-stamp at the
  predicted zero-crossing tic (the shared eval's `next_crossing_tic`
  machinery), and the existing sweep does the killing.
- Content tuning ([F5](forks.md#f5)): dev-scale drains (watchable drills),
  authored on the conditions so every species that starves/dehydrates
  inherits them — wolves, bunnies AND humans; the drills pen wolves and
  bunnies.

**Exit**: geo boxes carry glyphs (B/L/… verified in a geo-tier capture); a
food-denied wolf starves to death on camera leaving meat; a water-denied
bunny dehydrates likewise; a fed+watered control stays alive; the crossing
math is unit-tested in the shared eval; docs+memory truth pass; **the
user's eyes close the stream**.
