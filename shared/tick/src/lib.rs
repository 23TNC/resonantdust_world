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
//! The **priority-DAG is gone** (divergence #9, 2026-07-14): ordering is explicit `await`s +
//! the read rule, with cross-entity reads at ≤ T−1 → a DAG by tic → deadlock-free. `priority` /
//! `actor_read_tic` had no callers left. `Phase` is the other half of #9 and is **still here** —
//! `resolve_events` composes by it, so it goes when the word-DSL lands (P2), not before.

pub mod domain;
pub mod read_rule;
pub mod vm;

pub use domain::{
    action_phase, action_reads_actor, apply_event, hp, pack_damage, pack_move, pack_spawn,
    resolve_events, Domain, EntityState, Event, Phase, Spatial, ACTION_ACK, ACTION_DAMAGE,
    ACTION_MOVE, ACTION_RECEIVE, ACTION_SPAWN, ACTION_TRANSFER,
};
pub use read_rule::resolved_through;
