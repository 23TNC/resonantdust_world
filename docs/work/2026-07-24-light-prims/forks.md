# Light prims — forks

_Decision points. Order: chronological append. Leans are recommendations; F1/F2 change
[`VARIABLES.md`](../../VARIABLES.md) bands → ratify with the user before building
([`blockers.md`](blockers.md))._

## F1 — Is `light_data` the light's placed record, or does a light need a `billboard_data` record too? (2026-07-24, reframed by the vocab)
The ratified vocab answers most of this: `light_data` (rows 128–191: position in G, colour/intensity in B,
z/reach/emitter/hot/cast in A) is the **LIGHT presentation's placed record**, symmetric to `billboard_data`
for the billboard presentation (each carries its own position + presentation data). So:

- **A pure light writes ONLY `light_data`** — its position (G) + props/def-index. It rides the free-list +
  scoped cascade via the **light** path (`markLightDirty`), not `billboard_data`. No redundant record, gather
  read unchanged. (This supersedes the earlier "dual-record for every light" framing, which came from the
  old confusion of treating a light as a billboard that needed a `billboard_data` record just for position.)
- **A primitive that presents as BOTH** (an emissive sprite — torch + glow) writes `billboard_data` **and**
  `light_data`, sharing the one placement position; a move compare-writes both (a static one still emits no
  command).

**Open sub-point (the only real decision left):** for the both-presentations case, is position a single
source in one record **derived** into the other, or **written to both** from the shared primitive placement?
**Lean: written to both from the placement** — each presentation's record stays authoritative for its own
read path (no hot-loop indirection), and compare-write keeps static ones command-free. ([issues.md#i3](issues.md#i3).)
A later full-merge (retire `light_data`, read light props from a def in the gather) is possible but rewrites
the gather's light read (`fetchLin(LIGHT_BASE…)`) — not worth it now.

## F2 — Where do a light's static properties live (the "light def")? (2026-07-24, OPEN)
Colour / reach / emitter_radius / z-height / hot / cast_shadows — per placed light, or shared by type?

- **(a) Dedicated light-def band** (parallel to `billboard_definition_data`): "torch" / "moonlight" are defs;
  a placed light references one + carries only a position (+ overrides). Matches the prim→def
  indirection; shared types cost one def. New band in `VARIABLES.md`.
- **(b) Reuse `billboard_definition_data` with a light flag.** One def band for both. But the vocab makes
  this a category error: a billboard def is an **atlas frame** (frame_x/y/lod/nudge — geometric); a light
  def is **radiometric** (colour/reach) — different presentations, disjoint fields, awkward sharing.
- **(c) No def — props inline on the light record** (as `light_data` is today). Simplest; no def reuse;
  every placed light carries full props (no "torch type" sharing). 

**Lean: (a)** — a `light_definition_data` band **parallel to `billboard_definition_data`** is the symmetric
model the vocab implies (billboard: def + data; light: def + data). Keeps the two presentations cleanly
separate and gives shared light types for the many-lights future ([preserve-future-intent]). **(c)** is the
honest minimal start (inline == a def-of-one, a strict subset of (a)), so shipping (c) and adding the def
band when a second light type exists is low-regret. The vocab **rules (b) out**.

## F3 — How does `tick()` receive the lights? (2026-07-24, OPEN)
Today `tick(standing, resolver, win)` gets `standing = SquareCache.standingPrims()` (zIndex ≥ 1,
`SquareCache.ts:350-354`). Lights need a delivery list.

- **(a) Parallel `SquareCache.lightPrims()`** + a second `tick` arg: `tick(standing, lightPrims, …)`.
  Clean separation; a position-only (invisible) light is natural; lights don't pollute the caster set.
- **(b) Lights ARE standing prims with a light aspect** — one list; `tick` splits by a light flag on
  `Primitive`. Natural when a light rides a visual prim (torch); one delivery path.

**Lean: (a)** — a clean parallel selector, with (b)'s co-located case handled by a prim that appears in
BOTH lists (it's a caster AND a light). Pairs with F1-(a).

## F4 — Removal/global-change dirtying (2026-07-24, OPEN, low-stakes)
The two force-all fallbacks (`tick:1396`) — caster **removal** and a **scoped-rect-less light change** —
and the tuning-knob `forceColdDirty` sets are the manual surface P3 replaces.

- **Placement dirtying** (add/move/remove a light-prim) → the scoped `markLightMove`/`pendingRects`
  cascade; removal queues the light's **last reach box before freeing** (so no orphan force-all).
- **Genuinely-global re-bakes** (`__tilt`/`__pitchnormal`/`__worldlight`/`__nsfactor`/`__elevk` — a
  shader *constant* changed, affecting every tile) legitimately want force-all. Fold these into ONE
  explicit `rebakeAll()`, kept distinct from placement dirty. Not a placement concern → not deleted,
  just named. (Detail, not a hard fork — recorded so P3 doesn't over-reach and delete the legit path.)
