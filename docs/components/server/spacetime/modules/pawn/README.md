# Module — `pawn`

_Path: `server/spacetime/server/modules/pawn`. Deploys as `resonantdust-<env>-pawn-0`.
Last updated: 2026-07-28 (first-pawns)._

The **hot mover shard**: pawns (`TYPE_PAWN`), one row per entity. The core is the shared
`entity_tables!(data: u8)` stamp — the same shape as `data_shard` (which stays the hot
catch-all; first-pawns F1): `clock` / `entity_state_log` / `entity_state` and the
`init`/`bump`/`claim`/`write`/`gc` reducers. Who calls what: the orchestrator routes
`TYPE_PAWN` claims here (hot routing is read OFF the target's type nibble — unlike a cold row),
the worker composes + `write`s here, the master `bump`s/`gc`s, the edge subscribes
`entity_state` per zone (with an on-apply snapshot replay — a RESTING pawn's row would
otherwise never fire a callback).

On top of the stamp: the **spawn machinery** for `CREATE` — the idempotent `spawn` reducer
(mint + first `entity_state_log` row + promote, one transaction) and its `spawn_log` replay
ledger keyed `(event_reference, index)`. Minted ids start at `SPAWN_BASE` (the top half of the
24-bit object space) so they never meet the legacy client-minted band. Shapes:
[`TABLES.md` §pawn](../../../../../TABLES.md); the verb contract:
[`ACTIONS.md`](../../../../../ACTIONS.md) §`CREATE` + §Movement.

Operational rule (first-pawns I1): **republishing this module orphans connected SDK clients
silently** — restart the sim trio (`bin/sim run master|orchestrator|worker`) after any publish.
