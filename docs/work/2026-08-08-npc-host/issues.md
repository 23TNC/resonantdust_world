# Issues — npc-host (anticipated inventory)

## I1 — one event pump, N consumers: note ONCE, fan read-only

`Bot::note` mutates harness state per event. The host must note each event exactly once, then
fan the event to every module read-only — a module that re-noted would double-apply overlays.
Watch: modules keeping per-entity maps must filter by OWNERSHIP, not by kind alone.

## I2 — same-kind modules must not fight over pawns

Today adoption is kind-global (bunnies adopt any bunny). Two bunny modules — or a module plus a
pre-existing wild bunny population — need minted-by tracking + in-area claims (F4). The failure
smell: one pawn receiving interleaved commands from two minds, oscillating between wander goals.

## I3 — distant modules grow the UNION of subscribed zones

The one client subscribes the UNION of all module neighborhoods, each zone once (F2 — no
per-module replication). But modules in distant corners necessarily enlarge that union: the
client must stream both areas to see them at all, and the edge's shard connects scale with it.
Not a blocker at v1 scale — record the zone count per configuration in the drills.

## I4 — one tick loop serves all modules

`run_brain`'s single ticker becomes the host's loop over modules. A module that blocks (an await
on a fetch, a heavy scan) starves the others. Ticks stay non-blocking (the corpus fetch happens
once at host start, not per module), scans stay bounded to the area.

## I5 — THE debug measurement harness must survive

mover-perf/torch-perf run `NPC_BRAIN=debug NPC_KIND=… NPC_COUNT=…` as the standing perf
platform. The host keeps that env as a one-module fallback (F5) — re-run one measurement row
after the restructure to prove the numbers still land.

## I6 — the area clamp must not fight the worker's refusals

The worker refuses impathable destinations, and brains that re-pick a refused target oscillate
(the drowned-meat lesson, food-chain). Area clamping shrinks the candidate set — near a lake a
small area may contain NO pathable water/food. The policy must fall through to wander (and the
module log should say the area is starved) rather than spin on the same refusal.

## I7 — survival's drains make stage-1 AI load-bearing

Since 2026-08-08-survival, starving/dehydrated DRAIN corpus to death. The keep-alive policy is
what stands between ambient wildlife and extinction: the standing acceptance for this stream is
the survival control arc re-run under the HOST (population stable ≥30 min, no forced bands).

## I8 — N hosts per realm: arbitration deferred

Two hosts whose areas overlap could double-adopt wild pawns of the same kind. v1 ships with
distinct NPC_NAMEs and DISJOINT areas by configuration; cross-host claim arbitration (server-
visible ownership, or a claim registry) is a named successor when a second host actually runs.

## I9 — anchor moves race in-flight intents

Re-anchoring releases old zones on the client while owned pawns still walk there; their
StateGone/state events may drop with the subscription. The module must treat "pawn no longer
streamed" as re-locate-later, not death — the radii overlap during migration keeps this rare,
but the drill should move an anchor far enough to see churn once.
