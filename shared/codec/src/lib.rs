//! resonantdust-codec — the shared bit-packing layer.
//!
//! Every wire/storage bit-layout the game relies on lives here, in [`packed`],
//! so the server and the client encode and decode them with literally the same
//! code. Pure integer math, no wasm dependency: the spacetime modules and the
//! native server link it as an rlib (reachable via the shared bind-mount —
//! `../../../shared/codec`), and the pixijs client reaches it through the
//! `resonantdust-shared` wasm bundle, which re-exports this surface.
//!
//! It replaces the per-module `packed.rs` copies the foundation rebuild inlined
//! (the `zone_id → region_id` mask was duplicated between the `index` module and
//! the native server with a "keep the two in lockstep" comment). One definition,
//! no drift.

// `event_word` — the DSL word format (`op_code:4 | reserved:12 | server_reference:16 |
// payload:32`) — was deleted 2026-07-15. It was the wire unit of the tick pipeline's
// `actions: Vec<u64>`, and its only callers were `shared/tick`'s vm, the worker, and the edge's
// move/spawn handlers — all removed with the shard. It outlived them only because it lives here
// rather than in the deleted crates. The rebuild (`docs/intent/spacetime-again/`) still wants an
// RPN `actions: Vec<u64>`, but has not specified a word format; that's its call to make, not a
// shape to inherit. Recover the old one from `git show checkpoint/pre-shard-rebuild`.
pub mod action;
pub mod biome;
pub mod object;
pub mod packed;
pub mod refs;
pub mod status;
pub mod tic;
pub mod uid;

// The `tick_pipeline!` macro (crate-root via `#[macro_export]`) — the shared tic-composition
// machinery SpacetimeDB shards stamp out. Inert here (only expands where invoked); references
// `spacetimedb::`, which codec doesn't depend on, purely in its emitted tokens.
mod pipeline;
