//! resonantdust-tick — the pure core of the simulation pipeline.
//!
//! Everything here is plain integer logic: no `spacetimedb`, no wasm, no I/O. The
//! worker (`server_simulation`) reads events and state rows off its SpacetimeDB
//! subscription, projects them into the small [`domain`] types, and drives
//! resolution through [`read_rule`]. Keeping this SDK-free is what
//! lets the worker (and, later, the module) agree by construction — the resolution
//! order is a function of the log and the tic, nothing else.
//!
//! The load-bearing rule mirrored here:
//! - **read rule** ([`read_rule`]) — readiness is the *absence* of pending work at or
//!   below the read tic, never the presence of a resolved value.
//!
//! **Divergence #9 is closed** (2026-07-14): ordering is explicit `await`s + the read rule, with
//! cross-entity reads at ≤ T−1 → a DAG by tic → deadlock-free. Both halves are gone — the
//! priority-DAG (`priority`/`actor_read_tic`) and the `Phase` band (`Phase`/`action_phase`,
//! which re-ordered a tic's events inbound → data → outbound). Neither had callers: the worker
//! runs [`vm::run`] over each row's word program and folds in `event_reference` order.

pub mod domain;
pub mod read_rule;
pub mod vm;

pub use domain::{
    action_reads_actor, apply_event, hp, pack_damage, pack_move, pack_spawn, resolve_events,
    Domain, EntityState, Event, Spatial, ACTION_ACK, ACTION_DAMAGE, ACTION_MOVE, ACTION_RECEIVE,
    ACTION_SPAWN, ACTION_TRANSFER,
};
pub use read_rule::resolved_through;
