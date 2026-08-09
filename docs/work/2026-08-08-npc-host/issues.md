# Issues — npc-host (anticipated inventory)

## I1 — one event pump, N consumers: note ONCE, fan read-only

`Bot::note` mutates harness state per event. The host must note each event exactly once, then
fan the event to every module read-only — a module that re-noted would double-apply overlays.
Watch: modules keeping per-entity maps must filter by OWNERSHIP, not by kind alone.

## I2 — ownership re-attach must come from spawn attribution, not kind scans

DISSOLVED as a fight (F4: every module is a player; each commands only its own mints — two
same-kind modules co-locate fine). What remains: on restart a brain must re-attach to ITS pawns
through the server-side spawn attribution (the spawn log), and kind-global adoption (today's
bunnies shape) must actually DIE — a leftover kind-scan would resurrect the fight. Watch the
first restart drill: the re-attached set must equal the minted set exactly.

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

## I8 — wild (unowned) pawns need a stated fate

With adoption dead (F4/I2), a pawn whose minting player never returns — or pre-restructure
pawns with no attribution — belongs to nobody: it will wander unfed and die by survival's
drains. That may be the FEATURE (the world reclaims the ownerless); state it in P0 rather than
discover it as a mystery die-off. A "steward brain adopts the ownerless" is a named successor
if the fate should be rescue instead.

## I10 — player needs storage + the login/session shape must be verified live

Two spike-first facts for P1: (a) WHERE player need rows live — players are not pawn-shard
entities; the needs sub-table shape generalizes but the address (which shard, keyed how) is new
(F8). (b) Whether a module-player's commands validate without its own anchors/subscriptions —
the host's shared world model wants ONE subscription set, but mint/intent paths may require the
SENDER's session to be zone-connected. Both get a live probe before the host architecture
hardens around an assumption.

## I9 — anchor moves race in-flight intents

Re-anchoring releases old zones on the client while owned pawns still walk there; their
StateGone/state events may drop with the subscription. The module must treat "pawn no longer
streamed" as re-locate-later, not death — the radii overlap during migration keeps this rare,
but the drill should move an anchor far enough to see churn once.
