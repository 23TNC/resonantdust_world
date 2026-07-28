# Issues — movement-hardening

## I1 · Module/edge input hashes MISSED `shared/codec`/`shared/dsl` (found + fixed in P1)

`rd redeploy` hashed only each module's own directory, but every module compiles
`shared/codec` in (path dep) and the edge compiles codec + dsl — so adding `MOVE_STEP`
reported "nothing changed" and the LIVE `event_shard` validator would have rejected action 8
forever. Same blind-spot family as the subscription-SQL gotcha. FIXED (2026-07-28):
`RD_INPUTS` for every module gains `$SHARED_DIR/codec`, the edge gains codec + dsl.
Consequence, accepted: a codec change republishes every shard (data-wiping in dev) — correct,
since the change genuinely rebuilds every module, and sim-self-heal makes the wipe painless.

## I2 · A shard wipe leaves clients ADOPTING ghosts (found live in P1)

Republishing the pawn shard wiped the wolf, but the npc's adoption survives removals BY
DESIGN (the first-pawns I4 workaround: `StateGone` is untrustworthy, so removals are
ignored). The npc then drove a nonexistent pawn: seeds composed against a DEFAULT payload
(the ghost re-materialized at position 0 / macro 17) and the claim pipeline wedged (I3).
Resolution is SEQUENCED IN THIS STREAM: P2's edge suppression makes `StateGone` trustworthy,
after which the npc can HONOR real removals — drop the adoption, fall back to
adopt-or-CREATE. P2 gains that npc item.

## I3 · A stale DIRTY claim row wedges its entity's composition FOREVER (found live in P1)

The orchestrator's claim stamps `entity_state_log` rows dirty; the worker's write clears
them. If the claimed event vanishes (here: wiped/settled while the worker was behind), the
dirty row is never cleared — and the worker's BLOCK gate defers every later tic for that
entity (at `debug!`, so silently), while `gc` deliberately skips dirty rows. Measured: rows
at tics 3784–4434 `dirty=true` froze the wolf while the queue grew 14 deep. Fix (this
stream): the `entity_tables!` gc also reaps DIRTY rows past the horizon — a claim can't be
legitimately pending once its tic is beyond the freeze barrier; reaping it un-wedges the
entity at the next compose.

_Defects found during execution land here. The stream's INPUT issues live in their home
folders: [pawn-movement I5/I7](../2026-07-28-pawn-movement/issues.md) ·
[first-pawns I4](../2026-07-28-first-pawns/issues.md) — stamp those closed from here when
their fixes land._
