# Issues — spawn authority (anticipated; logged before they bite)

## I1 — the NEW-VERB checklist, in full {#i1}

The attack stream's lesson, now a list: a new codec verb needs (1) the event-shard
MODULE redeployed (async-invisible rejects otherwise), (2) the ORCHESTRATOR
rebuilt (`unframable program — UnknownAction(N)` silently parks events), (3) the
edge rebuilt (the ws door frames programs), (4) the worker rebuilt, (5) the wasm +
webgl bundles for the client sender. Miss one and the failure is silent somewhere
specific — check each explicitly.

## I2 — client pathable-picks demote to politeness {#i2}

The npc's pathable spawn pick (attack I2's fix) STAYS — it avoids pointless
refusals — but the server's gate is the protection. Do not delete the client
picks; do not trust them either. The npc must also HANDLE refusal: its
CREATE-once latch (`created = true`) would wedge forever on a refused request —
the adopt-or-create window must retry (a new pick) rather than assume the mint
landed.

## I3 — refusals are async and the chat can't see them {#i3}

`/spawn` returns its echo immediately; a server refusal lands only in the worker
log. Acceptable for a debug tool THIS stream — the chat echo says "requested",
not "spawned". A refusal-to-chat channel is a recorded successor, not scope.

## I4 — def validation reads the definitions MIRROR, not the corpus alone {#i4}

Clients send REGISTRY def refs; the worker validates against the `definitions`
table it already subscribes (attack P3) — id exists, `type_name != "gameplay"`,
and the KIND resolves in its corpus as a pawn-capable thing (has `walks`/needs?
No: the gate is "the corpus knows the kind" + "the def's type is pawn" — spawning
a meat pile via the debug door stays legal; it just mints a thing-kind pawn row
the way CREATE always allowed). The seed/registry agreement is guarded by the
master now; a mismatch here logs loudly rather than minting garbage.

## I5 — SPAWN_REQUEST replays must not double-mint {#i5}

The request arm queues ONE CREATE per request event; CREATE's ledger dedupes by
`(event_reference, index)` of the QUEUED event. A re-assigned/replayed REQUEST
would queue a SECOND create event (fresh reference → fresh mint). Guard: the
request arm runs in the same assigned-event pass as EXECUTE (each event
processed once per assignment); a worker-bounce replay of an ASSIGNED-not-
COMPLETED request is the residual window — same as every intent today, accepted
and stated (a dev-scale double pawn, visible and killable).

## I6 — the npc's adoption flow is untouched, verify it anyway {#i6}

Brains adopt from the state fan (CREATE never returned ids) — unchanged. But the
wolves' single-CREATE latch and the bunnies' `created < count` counter both
assume a queued CREATE mints; with refusals possible, both need the I2 retry
posture. Drill: a bunny request aimed at water (forced) refuses, the brain
re-rolls, the warren still fills.

## I7 — the teleport probe must separate AUTH jumps from RENDER jumps {#i7}

"Teleporting" conflates two failures: authoritative rows jumping (a server
compose/resolve bug — the serious one) and the render diverging/snapping (a
speculation bug — cosmetic but jarring). The probes tag each event AUTH or
RENDER with the row pair / the belief-vs-auth pair, so the verdict names which
pipeline is lying before anything gets patched.

## I8 — the variant tail maps by the corpus part declarations {#i8}

Nibble i = declared part i's variant (F3): a ≥2-part kind (the human) composes
PART entries; a single-part kind (wolf, bunny) takes nibble 0 into its OWN def
variant — its coat — and mints bare. Nonzero nibbles beyond the declared count
refuse. The wolf gaining a requestable coat is a small behavior GAIN over
today's client (which always minted wolves at variant 0) — stated, intended.

## I10 — thing/tile spawns are COLD writes: the request completes before the SET lands {#i10}

The pawn branch mints in the same pass (the spawn reducer); the thing/tile
branches QUEUE a SET that composes a tic later — the request's "success" log
precedes the visible thing by the barrier window. Harmless, but the drill must
wait for the overlay row, not the request log. And an occupancy RACE (two
requests naming one cell in adjacent tics) resolves by compose order — the
second SET overwrites; dev-scale accepted, stated (the interactive door is a
debug tool).

## I9 — CREATE's data word must carry the rotation without breaking the serial {#i9}

The minted row's `data` packs `facing | trip_serial` (`pack_pawn_data`). The
spawn currently writes `data: 0`; seeding the requested facing must leave the
serial lane 0 (no chain exists yet) and the movement rewrite path untouched —
the MOVE_TO stamp overwrites facing on the first order, which is correct (the
spawn facing is the RESTING pose).

## I12 — THE TELEPORT VERDICT, part 1 (client, FIXED): zone re-subscribes replay history into fresh movers {#i12}

Reproduced in minutes with the P5 probes: a long cross-zone trip + a mid-trip
interrupt. A zone crossing re-subscribes the event stream; the `event` table
replays HISTORY (no retention — first-pawns I2), and the crossing also
RECREATES the client's mover, whose `lastIntentTic` is null — so a minutes-old
MOVE intent passed both staleness guards and RE-ARMED speculation at the old
destination (the same `tic=399` intent armed three times, minutes apart, in the
console record). The spec walked the old dest while the server walked the new
one; reseed errors grew 0.27 → 6.34 tiles until the chase snapped — the
user-visible teleport. STATE rows replay the same way and dragged `authX`
backwards. FIX (MoverLayer): the mover tracks `authTic`; an intent more than 16
tics behind the freshest authoritative row is rejected as replayed history
(verified live — "intent REJECTED as replayed history" fires on every crossing
now), and a strictly-older state row is skipped.

## I13 — THE TELEPORT VERDICT, part 2 (server, FIXED): a cross-zone order executes once PER work-group {#i13}

The worker log showed every cross-zone `move_to` executed TWICE (effect_tic n
and n+1 — e.g. 4143/4144), and the orchestrator log showed WHY: two work-groups
assigned for the SAME tic (`assigned work-group tic=4138 … entities=1` and
`… entities=0`), each carrying the event — the orchestrator assigns by zone
footprint, a cross-zone order's footprint splits, and the arm ran once per
group. Two overlapping chains one tic apart leapfrog the authoritative rows:
7–9-tile strides per 32-tic re-anchor against a ~10-tics/tile pawn — the AUTH
jumps the probe caught. FIX (worker): a session-scoped
`executed_interactions: HashSet<(event, instruction)>` — each
EXECUTE_INTERACTION executes once; a duplicate assignment logs
"duplicate work-group assignment — SKIPPED". Post-fix orders execute exactly
once (the pre-fix double is on record; the skip-warn itself hasn't been caught
live yet since the group split depends on runtime zone activity — stated
honestly). The GROUPING itself still duplicates the event; the worker guard is
the cheap fix, the orchestrator-side single-owner assignment is the structural
successor.

## I14 — THE TELEPORT VERDICT, part 3 (server, NAMED): orders can execute MINUTES late {#i14}

The first soak's orders (queued ≈ tic 2830) executed at tics 3889 and 4138 —
~1000 tics (nearly 3 minutes) after queueing, then the pawn set off for a
destination the player had long forgotten: the "pawn walks off on its own"
class of teleport report. The delay correlates with the event's zones being
OUTSIDE any active work-group until other traffic (a wandering wolf, a bunny)
activated them. Structural — assignment/activity gating is the orchestrator's
design seam, not a worker patch. Named for a successor stream; the probes
(`__teleportProbe`, the AUTH watcher) make it observable when it recurs.

## I11 — FOUND LIVE: an unimported const in a match arm is a catch-all binding {#i11}

The worker's `SPAWN_REQUEST => {…}` arm compiled with the const NOT in the
`use` list — Rust silently made it a BINDING pattern that captured EVERY verb
reaching the spawn pre-pass. Every 3-operand instruction (MOVE_STEP hops,
QUEUE_STATE fans — their pawn word misread as `x:16|y:16`, hence a wall of
`x=12416 … position out of world` refusals) fell into the arm; real spawn
requests still worked, so the drills passed WHILE the log screamed. Diagnosed
by decoding a refused event's program from the event shard. The lesson joins
I1's checklist: after adding a verb arm, grep the build for the
`unreachable pattern` / unused-variable warnings a shadowing binding emits —
the compiler DID warn, the warning was skimmed past among pre-existing ones.
