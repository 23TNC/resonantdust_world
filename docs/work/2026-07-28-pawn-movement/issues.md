# Issues — pawn-movement

## I7 · npc deadline re-issue DUPLICATES a live chain (OPEN — observed in the flesh)

`MOVE_TO` chains have no identity: a chain only ends when its object reaches ITS dest (or a
queue fails). The npc's deadline re-issue assumes the old chain is dead — re-issuing while it
is ALIVE spawns a second chain, and two chains with DIFFERENT dests fight over the pawn
(tug-of-war: the trailing chain's `d` hovers at 1 so its "final" hops promote EVERY slot —
per-hop fan-out is back — and neither dest is reached on time, so more deadlines fire and
chains ACCUMULATE). Observed live 2026-07-28 during the I6-deploy restart transition
(19:27–19:30: five re-issues, landing errors up to 10 tiles, 12-tic-spaced promotes); it
self-drained once dests were reached and the steady state is clean — but the hazard fires
whenever a deadline underestimates a slow server. Recommended fix (sketch): chain
SUPERSESSION — the seed stamps a trip-serial into the pawn's `data` low bits (facing owns the
top 2), continuations become a worker-only arity-3 step verb carrying the serial, and a hop
whose serial no longer matches dies silently — at most one live chain per pawn, and a NEW
intent cancels the old chain by construction. Until then: npc deadlines carry generous slack
(re-issue only when the chain is near-certainly dead), and re-issues keep the SAME dest
(same-dest duplicates converge and both die).

## Observation · hidden browser tabs freeze speculation (expected, recorded)

`MoverLayer.tick` rides the render loop; a hidden tab fires no rAF, so specs never advance
and every landing logs `e ≈ trip length` (measured 12/12 trips at integer errors with
`visibilityState: "hidden"`). Not a defect — nobody sees a hidden tab, authoritative rows
keep the position truthful, and specs resume on visibility. Do NOT read landing-error stats
from an unfocused-tab soak.

## I6 · The seed promoted `start+1`, opening every trip with a one-tile snap (user, post-delivery)

Observed by the user watching the browser: land, pause (npc think time — random wander is by
design), SNAP one tile, then glide. Cause: the intent event's `MOVE_TO` STEPPED before its
seed `PROMOTE`, so the anchor fanned the post-step position while every client still held the
previous landing — also the source of the soak's persistent mid-trip `spec reseed e≈1` lines
(the seed row correcting the armed spec by one tile). The user's proposed fix, implemented
(worker `apply`): a `MOVE_TO` on a `PROMOTE_EVENT`-carrying program promotes the CURRENT
position unchanged (facing still turns toward the path); the first step lands on the first
continuation. The client mirror needs NO change — the fractional glide crosses tile `k` at
exactly `k · tics_per_tile`, which is when the server now writes it. The npc deadline gains
the extra hop-slot (`hops + 1`). Stepping rules were AUDITED while diagnosing: worker and
`walkGreedy` both step diagonally (both axes per hop) — no divergence there. FIXED
(2026-07-28), verified AT THE WIRE: sampling pawn `entity_state` across a live trip boundary
shows the landing row and the next trip's seed row at the IDENTICAL position (macro 98 /
micro 16128), the seed advancing only `tic` + facing (128→192, turned toward the new dest);
npc trip walls match the `hops + 1` cadence exactly (3 hops = 7.0 s, 10 hops = 24.3 s).
Deploy note: the restart transition itself briefly produced duplicate chains — that hazard is
recorded as I7.

## I1 · MoverLayer drops live intents on two paths (the snap-tween-snap's likely core)

Inherited defect, found at plan time (`client/webgl/src/game/world/MoverLayer.ts::onMoveIntent`):
an intent for a mover not yet in the map is dropped (`if (!m) return`) — the code comment
claims the seed `State` "reseeds everything" but the seed only PLACES the pawn; the dropped
intent never arms a spec, so the trip renders as two snaps. Likewise an intent arriving before
the tic clock anchors (`d === null`) is dropped. A later stale replay (I2 below) can then arm
a WRONG spec — old dest, old tic — producing "snap somewhere, tween, snap somewhere else".
FIXED (P3, 2026-07-28): the `pendingIntents` buffer holds both cases and `tick()` re-evaluates;
verified by the 14/14-arm soak.

## I2 · `event` table retention (inherited: first-pawns I2)

FIXED (P3, 2026-07-28): `event_shard::gc` reaps settled `event` rows past the same serial
horizon the hot shards use; the master calls it on its gc cadence (`tic − 64`, every 20 tics).
Verified: table bounded at 2 rows under continuous trips; zero stale arms on a fresh
subscribe. Client guards stay as defense-in-depth. Closes first-pawns I2.

## I5 · The master's durable tic runs at 5.41 Hz, not `TIC_HZ` = 6 (measured)

`index.master_clock` advanced 541 tics in 100.04 s (2026-07-28, healthy stack, WSL2 docker) —
a ~35 tics/min deficit against the authored rate. Everything server-side keys on tics so the
sim is CONSISTENT, just ~10% slower than authored wall speed; the casualty is any client that
extrapolates at exactly `TIC_HZ` (I4). Suspects: WSL2 timer overshoot per `interval` tick
(but tokio's default Burst behavior should catch up), or the bump→subscription→read round trip
dropping increments. Not diagnosed further this stream — the CLIENT must track the observed
rate regardless (F6), because no fix pins the true rate exactly. Recorded for a master-side
pacing pass later.

## I4 · The wall↔tic estimate LEADS the server by ~25 tics (found in P2 verification)

Live measurement (2026-07-28, fresh page load): intents arm with `d ≈ 28` tics when true
elapsed-since-event is ~3–5 tics, so speculation starts ~2 tiles along its line — a visible
forward JUMP at arm, then a correct-rate glide that arrives early and holds. Authoritative
tics are exact (landing 9102 − intent 9042 = 60 = 5 hops × 12), so the lead is in the CLIENT
estimate, not the server. Suspect: the estimator's max-implied-current-tic anchor observing a
row whose tic is not "now" (a replayed event off the retention-less table whose serial
wraps ahead, or a queued-at-future-tic row observed as if current). Diagnose in P3 alongside
the baseline measurement; the pending-intent buffer must not mask it.

DIAGNOSED (same day): the true tic rate is 5.41 Hz (I5). `TicEstimate` extrapolates at exactly
`TIC_HZ` = 6 and its max-anchor rule ignores every "lagging" arrival — with a slow server rate
the estimate RATCHETS ahead unboundedly (measured climbing d=5.5 → 16.1 over one session,
27.7 → 35.0 in the next). Knock-on: once the lead exceeds `(span+2)·tics_per_tile`, the
finished-long-ago guard starts dropping LIVE intents — the snap-tween-snap worsens with page
age, which is part of inherited I3's "flakiness". Fix = F6 (rate-tracking estimator).
FIXED (P3, 2026-07-28): arm `d` flat at 3.8–5.0 tics across a 3-min soak; landings converge
to e ≤ 0.22 tiles once the rate learns (~60 s from a cold page).

## I3 · Intent delivery flakiness (inherited: first-pawns I3)

RESOLVED BY MEASUREMENT (P3, 2026-07-28): with the edge event-sub `on_applied` replay
(sim-self-heal), retention (I2), and the client fixes (I1/I4) in place, a 2.9-min soak
delivered **14 intents for 14 trips — every trip armed exactly one spec, none absent, none
doubled**. The historical "flakiness" decomposes into the now-fixed causes: dropped arms
(I1), estimate-lead guard kills (I4), replayed duplicates (I2 — the serial dedup already
caught most), and the missing snapshot replay (edge, fixed in sim-self-heal). Closes
first-pawns I3.
