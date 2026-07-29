# hot-sync — ONE hot dirty, so the wolf and its lighting move as one

_Work stream, opened 2026-07-28, on pawn-render's heels. Component: `client/webgl`
(`WorldScene`, `Viewport`, `MoverLayer`, `SquareCache`, `shadowGather`, `coldShadowData`).
User's brief: the light does not stay linked 1:1 with the moving wolf. Suspicion: two
separate dirty signals — albedo/normal vs lighting — where HOT needs ONE combined dirty so
they stay in sync; cold lighting and cold albedo can legitimately stay independent.
Assumption to confirm: the wolf is drawn via the hot maps as intended._

## The assumption holds — and the suspicion is confirmed in code

The wolf IS on the intended path: warm G-buffer channels (albedo/normal/surface/zdepth)
composited per pixel in the blit, lit exclusively through the HOT light/shadow maps
(pawn-render P2/P3). So this plan applies. And the desync is structural — the mover's visual
and its lighting are driven by THREE independently-gated dirty systems, none sharing a
cadence, plus a frame-order skew:

1. **Frame order** — `WorldScene.update()` runs `panel.tick()` (warm bake → shadow/light
   passes → blit) BEFORE `moverLayer.tick()` (`WorldScene.ts:107-109`): every frame renders
   the PREVIOUS frame's mover state, and the dirty signals the mover raises land AFTER the
   passes that should consume them — each pipeline picks the change up on its own schedule.
2. **The warm G-buffer dirty** — MoverLayer re-bakes the sprite past `SPEC_APPLY_EPS`
   (1/32 tile) into `SquareCache`'s square-dirty, drained by `bakeDirty(BAKE_BUDGET = 128)`
   (warm has priority but is still budgeted — `Viewport.ts:391-393`).
3. **The hot light/shadow dirty** — `buildCasters` detects the RECORD change, but the
   record's position quantises to UNITS (1/16 tile, `encodePosition`), so lighting re-bakes
   on 1/16-tile crossings while the sprite re-bakes on 1/32-tile crossings — a structural
   2:1 cadence mismatch — and the record's unit-snapped position never exactly equals the
   sprite's fractional pixels.
4. **The receiver-map dirty** — a THIRD channel (`receiverPending`), re-baked same-frame but
   separately gated; the light pass reads the receiver map to find the wolf, so a skew here
   moves the lit silhouette independently of both.

Net effect: sprite, lit-body, and carved-shadow each step on their own grid at their own
moment — exactly the observed "light not 1:1 with the wolf."

## The design (user, ratified as [F1](forks.md#f1))

**For HOT, one dirty.** A mover's visual change and its lighting change are the SAME event:
one call site, one position snapshot, one extent (old ∪ new), fanning atomically into (a)
the warm cache's square dirty, (b) the hot class's light/shadow rects, (c) the receiver
rects, (d) the shadow-record rewrite. Nothing hot re-derives the change independently.
**Cold stays decoupled** — a static tree's albedo streaming and its baked lighting genuinely
have different lifecycles; the split-dirty architecture is CORRECT there and untouched.

## Non-goals

The hot-pass render budget (hot-shadows' scope); any change to cold dirty channels; the
render-chase itself (movement-hardening P2.5 — its eps crossing becomes the ONE cadence
both consumers share, not a thing to redesign).

## Verification surface

Browser `:5174/?user=Claude&focus=100,50&zoom=1&cb=area1`, tab VISIBLE; `__bakes()`,
`__movers`, `__gather`; per-frame instrumentation comparing the rendered anchor against the
hot-map response (P0 defines the metric, P2 asserts it); the npc soak + posed walks; fps
guard at zooms 1 and 0.25.
