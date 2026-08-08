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

## I8 — /spawn's variant hints only apply to multi-part kinds {#i8}

The worker composes PART entries only when the kind's corpus authors ≥2 mover
parts (the human shape); single-part kinds (wolf, bunny) mint bare and IGNORE
variant hints — same as today's client behavior, now enforced in one place.
