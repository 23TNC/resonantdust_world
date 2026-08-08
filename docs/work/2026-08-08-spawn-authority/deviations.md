# Deviations — spawn authority

## D1 — pawns mint DIRECTLY in the request arm, not through a queued CREATE

F1's original wording had the worker validate and then "queue the worker-only
CREATE". The implementation calls `pawn.reducers().spawn(...)` directly from the
SPAWN_REQUEST arm, keyed by the REQUEST event through the same spawn ledger —
one fewer event round-trip, and I5's replay window narrows to the one the ledger
already guards (a queued CREATE would have minted under a FRESH event reference,
so a replayed request would double-mint; keying by the request dedupes). F6's
fork entry records the router shape; this line records that it deviates from
F1's first wording and why. CREATE itself survives unchanged for the compose
path.
