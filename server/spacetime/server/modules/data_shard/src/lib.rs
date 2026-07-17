//! data_shard — the composition slots (`state_log`) and the client-visible latest (`state`).
//!
//! The whole module is the shared [`resonantdust_codec::tick_pipeline!`] machinery: the `clock` /
//! `state_log` / `state` tables and the `init` / `bump` / `claim` / `write` / `gc` reducers. The
//! orchestrator calls `claim`, the worker calls `write`, the master calls `bump` / `gc`. This module
//! and `event_shard` never call each other. Shapes: `docs/TABLES.md`. Flow:
//! `docs/intent/spacetime-again/`. The cold `tile`/`thing` shards invoke the same macro over their
//! baseline.

use spacetimedb::Table as _;

resonantdust_codec::tick_pipeline!();
