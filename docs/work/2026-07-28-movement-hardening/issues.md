# Issues — movement-hardening

## I1 · Module/edge input hashes MISSED `shared/codec`/`shared/dsl` (found + fixed in P1)

`rd redeploy` hashed only each module's own directory, but every module compiles
`shared/codec` in (path dep) and the edge compiles codec + dsl — so adding `MOVE_STEP`
reported "nothing changed" and the LIVE `event_shard` validator would have rejected action 8
forever. Same blind-spot family as the subscription-SQL gotcha. FIXED (2026-07-28):
`RD_INPUTS` for every module gains `$SHARED_DIR/codec`, the edge gains codec + dsl.
Consequence, accepted: a codec change republishes every shard (data-wiping in dev) — correct,
since the change genuinely rebuilds every module, and sim-self-heal makes the wipe painless.

_Defects found during execution land here. The stream's INPUT issues live in their home
folders: [pawn-movement I5/I7](../2026-07-28-pawn-movement/issues.md) ·
[first-pawns I4](../2026-07-28-first-pawns/issues.md) — stamp those closed from here when
their fixes land._
