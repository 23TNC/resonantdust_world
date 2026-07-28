//! pawn — the hot mover shard: pawns (`TYPE_PAWN`), one row per entity.
//!
//! The whole module is the shared [`resonantdust_codec::entity_tables!`] machinery — the same
//! stamp as `data_shard` (which stays the hot catch-all; first-pawns F1): the `clock` /
//! `entity_state_log` / `entity_state` tables and the `init` / `bump` / `claim` / `write` / `gc`
//! reducers. The orchestrator routes `TYPE_PAWN` claims here, the worker composes + `write`s
//! here, the master `bump`s / `gc`s, the edge subscribes `entity_state` per zone. A `spawn_log`
//! (server-minted ids for `CREATE`) joins in first-pawns P1. Shapes: `docs/TABLES.md`. Flow:
//! `docs/intent/spacetime-again/`.

use spacetimedb::Table as _;

resonantdust_codec::entity_tables!(data: u8);
