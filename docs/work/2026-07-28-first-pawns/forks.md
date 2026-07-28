# Forks — first-pawns

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · New `pawn` shard vs renaming `data_shard`

**Resolved: new shard class; `data_shard` stays as the catch-all.** `TABLES.md` and
`world-storage` both write "`data_shard` (→ `pawn`)", and shard-tables' follow-on says "move
wolves to a `pawn` table — OFF `data_shard`" — a new stamp, not a rename. The worker's
`_ => Shard::Data` arm keeps unknown types landing somewhere sane; ripping `data_shard` out now
would couple this stream to a routing audit it doesn't need. Retirement (delete, don't
deprecate) is a recorded follow-on once nothing routes there.

## F2 · Who steps the tiles — npc hops vs worker `MOVE_TO` chain

**Resolved: the npc issues adjacent-tile hops; the self-queueing chain stays tabled.** The
user's brief is explicit: "the npc container will drive its movement between tiles … in
anticipation of more advanced commands coming from npc." The worker's current one-step `MOVE_TO`
is *correct* for an adjacent hop (one step toward dest = dest), so no worker change is needed
for movement. The documented multi-tile chain + queue-at-future-tic + sparse promote cadence
(`ACTIONS.md` §Movement) is future intent we build TOWARD — it arrives with advanced commands,
and nothing in this stream forecloses it.

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
