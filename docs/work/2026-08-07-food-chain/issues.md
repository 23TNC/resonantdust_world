# Issues — food chain (anticipated; logged before they bite)

## I1 — the pawn module redeploy WIPES live pawns {#i1}

The pawn shard has no `remove` reducer — adding one changes the module, and a module
redeploy clears its (transient) tables: every live pawn vanishes and re-mints. Dev-
scale acceptable, but sequence it deliberately: redeploy pawn module → restart master
(orchestrator assignment) → re-mint the drill cast (/spawn + npc adoption). The
subscription SQL is a string — the new reducer needs the LIVE check (build-gates law).

## I2 — the corpus mint must read the EFFECTIVE max {#i2}

`mint_sidecars` quantizes `np.max` (the authored 2) — wolves would be right and
bunnies WRONG (minted 2/1 health). The mint computes the effective cap from the
kind's OWN trait list (already in hand in the same function) under the F2 highest-
wins law. Unit-test the mint: bunny mints 1, wolf mints 2, both encode on 0..2.

## I3 — death must be IDEMPOTENT and race-calm {#i3}

Two observers may both fire death (the npc and a player click). The worker validates
corpus ≤ 0 (fine, both pass) — the SECOND execution must find the pawn gone and no-op
the whole effect (no double meat). Order inside one execution: validate → spawn meat
→ remove; the remove reducer deletes by entity id and succeeds silently on absence.

## I4 — the npc thing mirror is composed, not raw {#i4}

Baseline ⊕ overlay with kind-0 suppression — eaten meat and felled trees must vanish
from `nearest_thing` or bunnies walk to ghosts. Mirror the client bridge's compose
rule exactly; a stale scan self-corrects on arrival (the offer re-validates) but
should be rare, not constant.

## I5 — the forage spawn cell asks the SAME probes as movement {#i5}

"Empty adjacent" = no composed thing AND tile pathable (water is not a shelf). Reuse
the worker's pass-level `cell_pathable` + `thing_kind_at` — a second emptiness rule
would drift (the two-copies class).

## I6 — five registry appends + six consumers {#i6}

bunny/meat/plant_matter (things) + four traits + four interactions + two needs + three
stats + three affordances are REGISTRY appends — master seeds on boot; all sim crates
+ edge + wasm rebuild (the I11 stale-loader law); golden re-blessed; browser wasm
cache-busted. One checklist pass, not vibes.

## I7 — death mid-queue {#i7}

A dying pawn may hold a walking chain + parked intents. The remove clears the
ephemeral queue and fans an empty QUEUE_STATE; the chain's next hop finds no pawn row
and dies silently (scratch absent → the hop no-ops). Verify the strip empties and no
hop warns.

## I8 — hunger's numbers are provisional {#i8}

hunger 0..100, deplete 43200 (2 h wall — slower than thirst), bands hungry/starving
mirroring thirsty/dehydrated with emotions (uncomfortable, scared) and eat magnitudes
+30. All TOML — tuning is a data edit, not a design point.

## I10 — the trigger's 4-tic window and cascade bound {#i10}

The user's "same turn" rides the event pipeline: the triggered order queues at
`master + 4` (the barrier floor), so ~0.7 s separates the fatal write from the death
execution — during which the fire-time re-validation makes the window CORRECT (a heal
inside it saves the pawn; that is a feature, stated). Cascades (a triggered execution
writing needs, sweeping again) terminate because every hop re-validates and death
removes its holder; if a future content loop appears, a per-event sweep-depth guard
is the fix — note it, don't build it.

## I11 — FOUND+FIXED: in-flight chains resurrect the removed (2026-08-07) {#i11}

Seen live: a removed wolf's queued MOVE_STEP fired after death, `base_row` fell back
to entity_state_LOG history, and the compose WRITE re-inserted a live row — a zombie
with no payload. Fix: a PAWN write target with no LIVE `entity_state` row drops out
of the compose (its chain completes as a no-op); minted pawns are safe because
`spawn` writes their first row before any hop targets them.

## I9 — the self-carried interaction's carrier resolution {#i9}

Death rides the PAWN's OWN kind (`interactions` on the bunny/wolf/human defs) but the
carrier probes read COLD things — a pawn is a HOT entity. The resolve needs the
self-carrier case: when the interaction names location `self`, the carrier IS the
target pawn (kind from its definition_reference), no cold probe. A new location rule,
small and explicit.
