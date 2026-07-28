# Forks — first-pawns

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · New `pawn` shard vs renaming `data_shard`

**Resolved: new shard class; `data_shard` stays as the catch-all.** `TABLES.md` and
`world-storage` both write "`data_shard` (→ `pawn`)", and shard-tables' follow-on says "move
wolves to a `pawn` table — OFF `data_shard`" — a new stamp, not a rename. The worker's
`_ => Shard::Data` arm keeps unknown types landing somewhere sane; ripping `data_shard` out now
would couple this stream to a routing audit it doesn't need. Retirement (delete, don't
deprecate) is a recorded follow-on once nothing routes there.

## F2 · Who steps the tiles — RESOLVED BY THE USER (2026-07-28): the worker chains

**The npc asks A→B; the worker works out how.** First plan draft had the npc issuing
adjacent-tile hops with the chain left tabled; the user corrected it: the npc issues one
`MOVE_TO obj dest`, the worker steps a tile per hop and self-queues the continuation
(queue-at-future-tic), the **intent** is broadcast to clients once (`PROMOTE_EVENT`), individual
steps are NOT fanned out, clients **speculate** position from the intent on their tic estimate,
and authoritative `state` corrects them at the destination — plus whenever the object's position
is resolved as part of working out other events. This un-tables `ACTIONS.md` §Movement (rewritten
in prefix vocabulary as part of P2). The end goal is a pathfinding move command; v1's step fn is
a greedy straight line behind an explicit seam so pathfinding replaces one function, not the
chain.

## F3 · How the npc learns the minted id

**Resolved: adopt from the state fan-out.** `CREATE` returns nothing on the wire (`QueueOk`
carries only the cid), and `PROMOTE_EVENT` — the intent-announcement channel that could echo it
— is tabled. The wolf brain therefore watches its anchored zone for the first `StateObject`
whose `definition_reference` is the wolf def and whose id it doesn't know, and adopts it.
Correct for one brain spawning one wolf at a time (this stream's scope); a brain spawning many
pawns concurrently needs the event echo — noted with the §Movement follow-on. Rejected:
returning the id through `QueueOk` (widens the wire for one caller; the spawn-log already holds
the truth server-side).

## F4 · Spawn mechanism on the shard

**Leaning (resolve at build): a single idempotent `spawn` reducer.** Mint + `entity_state_log`
write + `spawn_log` record in one transaction, keyed `(event_reference, index)`; replay finds
the spawn-log row and returns the recorded id without re-minting. Rejected: worker-side mint
(can't — a 24-bit `object_reference` can't be derived from a 32-bit `event_reference`, per
`ACTIONS.md`); a two-step mint-then-`write` (a mid-compose round-trip and a torn state if the
worker dies between the calls).

## F5 · Where the wolf def id comes from for npc

**Leaning (resolve at build): the DSL content corpus, same as clients.** npc already links
`client/core`; the def↔name mapping is content (`things.rd` order), and hardcoding a constant in
codec would re-create the phantom `KIND_WOLF` drift this stream is deleting. If pulling the
corpus into the headless npc is heavy, fall back to one shared constant exported next to
`TYPE_PAWN` — but content is the authority either way.

## F6 · How the npc container is supervised

**Leaning (resolve at build): extend `bin/sim`.** One supervisor for every headless sim process
(`rd-master`, `rd-worker`, `rd-orchestrator`, `rd-npc-<brain>`) beats a second bespoke compose
runbook; `bin/sim` already owns build-image, cargo cache, detached-run, logs, ps. The npc builds
from `client/` rather than `server/`, so the crate list needs a path map — if that turns
`bin/sim` inside out, fall back to a `run`-service in `client/npc/compose.yml` plus an `rd npc`
wrapper.

## F7 · Where `tics_per_tile` (speed) comes from

**Leaning (resolve at build): a constant behind a per-def seam.** `ACTIONS.md` says speed is
per-kind content — but the worker links no DSL and holds no content corpus (the same gap
torch-thing I2 recorded for the `thing` module), and the CLIENT needs the identical value to
speculate at the right rate. v1: one `tics_per_tile(definition_reference) -> u16` in
`shared/codec` returning a constant, called by worker AND client — one authority, trivially
replaced by a content lookup when the corpus-plumbing follow-on lands. Rejected: hardcoding at
each call site (drifts — the speculation would walk at a different speed than the server).

**Author speeds in WALL-TIME units, convert at the seam.** The user set `TIC_HZ` to **6**
(2026-07-28; was 2 — 0.5 s command-to-visible floor at the +3 barrier, still ~10× per-tic
headroom vs typical logic loops; "if it bites us we can drop down"). So the seam's inner truth
is tiles/second, converted via `TIC_HZ` — a future rate change must never rescale the world's
movement. Corollary: `TIC_HZ` needs ONE authority reachable by master/orchestrator/worker AND
the speculating client (today it's three independent env defaults + the client's estimate) —
home it in `shared/codec` alongside `tics_per_tile` when this fork is built.

## F8 · Speculation correction policy

**Resolved for v1: snap, and record the error.** When authoritative `state` disagrees with the
speculated position, the client snaps to truth (and reseeds speculation if an intent is still
live); `ZoneClosed` drops the speculation outright. Smooth error-blending and the
re-anchor-every-N promote cadence are knobs that need DATA first — every correction logs its
error magnitude so the follow-on tunes against measurements, not guesses. Rejected for v1:
blending toward truth (hides real drift while we still need to see it).
