//! resonantdust-tick — the pure core of the simulation pipeline.
//!
//! Everything here is plain integer logic: no `spacetimedb`, no wasm, no I/O. The
//! worker (`server_simulation`) reads events and state rows off its SpacetimeDB
//! subscription, projects them into the small [`domain`] types, and drives
//! resolution through [`priority`] and [`read_rule`]. Keeping this SDK-free is what
//! lets the worker (and, later, the module) agree by construction — the resolution
//! order is a function of the log and the tic, nothing else.
//!
//! The design is in `docs/simulation.md`; the load-bearing rules mirrored here are:
//! - **priority-DAG** ([`priority`]) — a total order over the unified entity key that
//!   breaks same-tic dependency cycles without ever detecting them.
//! - **read rule** ([`read_rule`]) — readiness is the *absence* of pending work at or
//!   below the read tic, never the presence of a resolved value.

pub mod domain;
pub mod priority;
pub mod read_rule;

pub use domain::{
    action_reads_actor, apply_event, hp, pack_damage, pack_move, resolve_events, EntityState,
    Event, ACTION_ACK, ACTION_DAMAGE, ACTION_MOVE, ACTION_RECEIVE,
};
pub use priority::actor_read_tic;
pub use read_rule::resolved_through;
