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
//! (shard / chat / players each carried their own `valid_at` pair, and the
//! `zone_id → region_id` mask was duplicated between the `index` module and the
//! native server with a "keep the two in lockstep" comment). One definition, no
//! drift.

pub mod biome;
pub mod event_word;
pub mod object;
pub mod packed;
pub mod refs;
