# Issues — survival (anticipated; logged before they bite)

## I1 — the drain must evaluate IDENTICALLY in all four consumers {#i1}

The piecewise eval is shared (wasm + npc + worker + the panel's crossing
forecast), and stat-model I2's law stands: quantize/rounding defined once or
ghost refusals appear. The drain extension lands in `needs_eval` ONCE, with
a unit test pinning a known trajectory (value, drain set, elapsed →
expected), and every consumer rebuilds (the trait-lights I6 sweep:
wasm + webgl + npc + worker + master, golden re-blessed).

## I2 — the crossing re-stamp can CASCADE {#i2}

A re-stamp is itself a corpus write → the sweep runs → a new crossing gets
scheduled. Healthy: exactly one pending crossing re-stamp per (pawn, need)
at a time, or a hungry pawn schedules a re-stamp per write and the queue
breathes garbage. The worker keys its scheduling ledger by (pawn, need) and
SUPERSEDES — the freshest trajectory owns the one slot (the ONE-chain law's
shape, lifted to crossings).

## I3 — death must not double-fire on the crossing path {#i3}

The attack path already writes corpus ≤ 0 and fires death; the crossing
path adds a second producer of ≤ 0 writes. The existing sweep + `can_die` +
the remove's idempotency are believed sufficient (food-chain I11 dropped
dead targets from compose) — but the drill must include a pawn ATTACKED
while starving, the two producers racing.

## I4 — dead pawns must stop being scheduled {#i4}

The crossing ledger holds (pawn, need) entries for pawns that may die (or
despawn) before the re-stamp fires. The re-validation covers correctness (a
write against a removed pawn no-ops — food-chain law), but the ledger needs
the removal hook or it leaks entries at dev scale and lies in logs.

## I5 — the glyph must NOT leak into real-art rendering {#i5}

The letter rides the geo/placeholder seam only. The known hazard shapes:
the outline path (food-chain F9 textureless placeholders) vs the geo TIER
(pre-texture) are two different code paths — the glyph belongs on BOTH (a
bunny's flat box is the placeholder path; a not-yet-loaded conifer is the
geo tier), and on NEITHER once a real albedo serves. The geo-flash law
(never geo between resident packs) must hold with glyphs exactly as
without.

## I6 — npc brains under mortality {#i6}

Wolves and bunnies already drink/eat on their bands — but the DRILL starves
them deliberately (penned from food/water), and the brains will thrash
trying (nearest_tile walks, refusals). Acceptable for the drill; the watch
item is the OTHER direction: after this stream, ambient wildlife must
actually sustain itself (the wolf must find enough prey, the warren enough
flora) or the world quietly depopulates. The post-stream soak asserts the
fed+watered control ARC: population stable over ≥30 min.

## I7 — humans inherit mortality with no keeper {#i7}

Humans carry the same bands; after this stream an unattended human WILL
starve (the ambling debug fixture doesn't feed itself). Stated so the first
dead human in a drill world is read as the feature working. If that's
unwanted for the dev fixture, the lever is content (a human-only drain
exemption is NOT built — the user said "I think humans too").

## I8 — the u16 tic ring under long drains {#i8}

A 3600-tic drain fits the half-window guard easily, but the crossing math
must clamp: a predicted crossing beyond the u16 half-window cannot be
queued naively (the standing tic-seam law, stat-model I6). Far crossings
re-stamp at the horizon instead — the re-validation chains the rest.
