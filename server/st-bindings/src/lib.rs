//! Generated SpacetimeDB SDK bindings for the rebuild's modules, shared by every SDK client
//! (master / orchestrator / worker, and eventually the edge) so the bindings live in one place
//! instead of a 5000-line copy per crate.
//!
//! **Generated — do not edit.** Regenerate a module's bindings with `spacetime generate --lang rust
//! --out-dir server/st-bindings/src/<module> --module-path server/spacetime/server/modules/<module>`.
//! Each `<module>/` is self-contained (its own `DbConnection` / `RemoteModule` / `Reducer`).
pub mod event_shard;
pub mod data_shard;
pub mod index;
