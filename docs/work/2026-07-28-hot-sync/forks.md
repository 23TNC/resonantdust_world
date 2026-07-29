# Forks — hot-sync

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · ONE hot dirty — RESOLVED BY THE USER (2026-07-28)

A mover's visual and lighting change are the same event: one call site, one position
snapshot, one extent, fanned atomically into the warm-cache dirty, the hot light/shadow
rects, the receiver rects, and the record rewrite. COLD keeps its independent channels — a
static prim's albedo streaming and its baked lighting genuinely have different lifecycles
(that decoupling is the cold tier's whole cost model), which is exactly the user's framing:
"in theory cold lighting and albedo/normal can be independent, but for hot we need one."
Rejected: keeping the change-detection in `buildCasters` as the origin (it reads the record's
unit-quantised position — the 2:1 cadence mismatch IS this path); rejected: syncing by
tightening quantisations only (two independent gates on one moving value always re-diverge;
the fix is one gate).

## F2 · Mover tick moves BEFORE the viewport tick

`moverLayer.tick()` currently runs after `panel.tick()`, so every frame renders yesterday's
mover state and the dirty signals land after their consumers ran. Flipping the order makes
the frame self-consistent (position, sprite bake, record, rects, blit — one snapshot) and
removes a full frame of baseline lag for free. Rejected: leaving the order and
double-buffering positions (complexity to preserve a defect); rejected: moving the chase
into the viewport (MoverLayer owns speculation policy — the seam stays).

## F3 · The shared cadence is the render-chase's eps crossing

The sprite already re-bakes on `SPEC_APPLY_EPS` (1/32 tile) crossings — that becomes THE hot
cadence: the record rewrites and the rects queue on the same crossing, from the same
snapshot. The record's unit encode (1/16 tile) may round the stored position; that ≤1-unit
rounding is display-invisible — what matters is that both consumers STEP TOGETHER. Rejected:
driving both from unit crossings (halves the sprite's smoothness); rejected: per-frame
unconditional rewrites (the eps gate exists to bound bake churn — keep it, share it).

## F4 · Mover squares bake unbudgeted

The warm bake keeps its budget for streaming/zoom floods, but a MOVER's own squares always
land the frame they dirty — movers are a handful, their slots are few, and a deferred sprite
under fresh lighting is precisely the desync this stream kills. Rejected: coupling the light
rect to a deferred albedo bake (synchronised lag is still lag, and the machinery to defer a
class-pass rect per-cause doesn't exist); rejected: removing the budget entirely (cold
streaming legitimately needs it).
