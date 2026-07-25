# Light prims — forks

_Decision points. Order: chronological append. Leans are recommendations; F1/F2 change
[`VARIABLES.md`](../../VARIABLES.md) bands → ratify with the user before building
([`blockers.md`](blockers.md))._

## F1 — How does a placed light populate `light_data`? (2026-07-24, OPEN)
The gather reads a light from the `light_data` band (`VARIABLES.md` rows 128–191: position in G,
colour/intensity in B, z/reach/emitter/hot/cast in A). If a light becomes a prim, where does the record
come from?

- **(a) Dual-record — light-prim writes `prim_data` (position + light-def) AND `light_data`.** The
  `prim_data` record gives the light a placed position that rides the free-list + `markPrimChange`
  cascade (`coldShadowData.ts:360-391`); `light_data` is **derived** from the prim's position + its
  light-def each build. Gather read path unchanged. A light can also BE a caster/visual (a torch sprite
  + a light) since it's a real prim. Cost: position lives in two records → derive keeps them in sync
  ([issues.md#i3](issues.md#i3)).
- **(b) Derived-only — keep just `light_data`, no `prim_data` for a light.** "Prim" is conceptual: the
  light is delivered via a prim list (F3) but its sole record stays `light_data` (position already in G).
  Minimal churn, no redundant record, gather unchanged — but a light is NOT a real prim (can't also be a
  caster/visual; doesn't ride the prim free-list — needs its own).
- **(c) Full-merge — retire `light_data`; light props live in a light-def, gather reads prim→def.**
  Most unified (one record shape for everything placed), but rewrites the gather's light read
  (`GATHER_FRAG`/`LIGHT_FRAG` `fetchLin(LIGHT_BASE…)`) and the presence→light indirection.

**Lean: (a).** It makes "a light is a prim" literally true (rides the caster free-list, eviction, and
`markPrimChange` cascade for free) and lets an emissive thing be one object with both a sprite and a
light — the general model the downstream effects want. Keep `light_data` as the derived read target so
the bake is untouched. (b) is the minimal-viable if lights must stay position-only for now; (c) is a
later unification, not worth the gather rewrite yet.

## F2 — Where do a light's static properties live (the "light def")? (2026-07-24, OPEN)
Colour / reach / emitter_radius / z-height / hot / cast_shadows — per placed light, or shared by type?

- **(a) Dedicated light-def band** (parallel to `prim_definition_data`): "torch" / "moonlight" are defs;
  a placed light references one + carries only a position (+ overrides). Matches the prim→def
  indirection; shared types cost one def. New band in `VARIABLES.md`.
- **(b) Reuse `prim_definition_data` with a light flag.** One def band for casters + lights. Fewer
  bands, but overloads the def: a caster def is an **atlas frame** (frame_x/y/lod/nudge — geometric); a
  light def is **radiometric** (colour/reach) — disjoint fields sharing 4 texels awkwardly.
- **(c) No def — props inline on the light record** (as `light_data` is today). Simplest; no def reuse;
  every placed light carries full props (no "torch type" sharing). 

**Lean: (a)** for the documented future (many light types — [preserve-future-intent]) — a dedicated
light-def keeps the caster-def clean and gives shared light types. **(c)** is the honest minimal start
if only a handful of ad-hoc lights exist; it's a strict subset of (a) (inline == a def-of-one), so
starting (c) and adding the def band later is low-regret.

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
