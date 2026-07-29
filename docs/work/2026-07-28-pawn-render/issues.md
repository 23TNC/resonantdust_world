# Issues — pawn-render

> Follow-on: the user observed lighting not tracking the mover 1:1 — root-caused and FIXED by
> [2026-07-28-hot-sync](../2026-07-28-hot-sync/README.md) (this stream's per-move dirty
> originated in `buildCasters`' change-detection at the record's own cadence; hot-sync
> unified it into ONE mover dirty).

## I1 · The edge's index REGISTRATION doesn't self-heal (found at P0 standup)

The P0 browser baseline hit gateway 503 "no server available": the edge's index
connection (its `set_server` registration + heartbeat path) died during
movement-hardening's daemon-restart drill and never re-registered — `servers` was EMPTY
while the edge itself served its existing WS sessions happily (the npc never noticed).
The registration path is an unconverted-uplink surface, exactly the class sim-self-heal
fixed elsewhere. Restored by an edge restart. FIX BELONGS to a follow-up (an uplink +
re-register-per-generation on the edge's index conn, the same shape as the master's
`set_orchestrator` re-stamp); recorded here because this stream found it.

## I2 · A pawn resting past HALF THE TIC RING loses its base — next touch composes from DEFAULT (found at P0)

Commanding the long-parked leftover wolf (`0x30800001`, resting since ~tic 15k) at tic ~53.7k
teleported it to position 0/macro 0: `base_row` filters log rows by `tic_before(r.tic, tic)`,
and 38,750 tics of rest exceeds `TIC_WINDOW` (32,767) — the old tic reads as serially FUTURE,
no base qualifies, the compose runs `or_default()` (the ghost-at-zero disease, now with a
timer). The shard `gc` deliberately keeps each entity's latest CLEAN row forever ("every
future tic's base") but never refreshes its TIC, so every parked pawn's base expires ~90 min
after its last write at 6 Hz. FIX (this stream, immediately — it corrupts any long-resting
pawn): the macro `gc` RE-STAMPS the kept latest-clean row to the horizon tic as it sweeps
(delete + re-insert — the uid encodes the tic), so a resting base stays serially near
forever. Same-family note: `entity_state` (visible) also carries the stale tic, but nothing
serial-compares it server-side today.

_Defects found during execution land here. Known inputs: the STALE
[2026-07-24-shadows-onto-prims](../2026-07-24-shadows-onto-prims/README.md) folder (its
receiver machinery got built by later streams — reconciled in P5), and `MoverLayer.ts:6-11`'s
doc comment overclaiming "lighting + shadow all handled by the shared pipeline" (aspirational
— corrected as the pipeline actually starts handling them)._
