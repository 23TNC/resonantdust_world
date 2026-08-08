# Forks — spawn authority

## F1 — SPAWN_REQUEST is a fixed-arity pure request; CREATE stays the one mint path {#f1}

`SPAWN_REQUEST def position variants` — three Imm operands, writes nothing (no
write set, no claim). The worker's arm validates and queues the real
`PROMOTE CREATE …` program worker-side, so `CREATE`'s existing spawn LEDGER
(`(event_reference, index)` idempotency) remains the single mint gate and replay
protection. `variants` packs the debug hints: `body:4 | head:4` (0 = default), room
above for future lanes. Rejected: making SPAWN_REQUEST mint directly — it would
duplicate the ledger; and reusing EXECUTE_INTERACTION — spawning has no actor, no
carrier, and no corpus interaction, so the fit is false.

## F2 — refuse, never nudge {#f2}

An invalid request (unknown/non-pawn def, out-of-world or IMPATHABLE position) is a
LOGGED REFUSAL, whole. Nudging to a nearby cell would make the server guess intent
and make refusals invisible; a debug tool wants loud edges. This moves the
lake-mint guard (attack I2 — the stranded mid-lake wolf) into the authority where
it can never be forgotten by a new client. Client pathable-picks stay as
politeness, not protection (I2).

## F3 — the server composes parts, traits, and needs {#f3}

The worker already composes TRAIT + need sidecars (`mint_sidecars`); PART entries
now compose there too (`mint_parts`): for a multi-part kind, one PART entry per
slot with the def's variant nibble substituted from the request's hints (validated
u4). The chat handler's opcode packing and the npc's payload argument are DELETED
(delete-don't-deprecate). The payload wire format is untouched — only WHO writes
it changes.

## F4 — CREATE goes worker-only {#f4}

Leaves `CLIENT_VERBS` (the movement-hardening seam): a client CREATE is now
refused at the edge door. The npc and webgl both switch to SPAWN_REQUEST in the
same stream, so nothing legitimate breaks; anything else composing CREATE was
already outside the design.

## F5 — the teleport hunt is evidence-first {#f5}

The bug-sweep I11 discipline: instrument, reproduce, correlate — then fix. Two
probes: (1) SERVER truth — consecutive `entity_state_log` rows per pawn whose
position delta exceeds what one hop stride can carry (the smoking gun for a bad
resolve/write); (2) CLIENT truth — per-frame render-vs-authoritative divergence
beyond the chase cap's explainable envelope (the smoking gun for bad speculation).
Traffic: npc wander + long player trips + mid-trip interrupts + zone crossings
(the resolve/supersession paths). The suspects, ordered: the mid-chord resolve
writing a position from a MISMATCHED chord (it recomputes chords that may differ
from the walking chain's), interrupts re-anchoring from stale bases, event
replays re-arming client speculation (`lastIntentTic` edge cases), and the chase
snap teleporting the RENDER while auth is fine. Conditions land in issues.md with
row dumps; cheap causes get fixed here, structural ones get named successors.
