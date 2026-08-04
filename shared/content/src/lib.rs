//! resonantdust-content — the CONTENT loader that turns `content/*.toml` into a
//! runnable [`Bundle`] (the crate name predates the 2026-08-04 TOML cutover and
//! renames to `resonantdust-content` with it — work `2026-08-04-toml-content`).
//!
//! Pure Rust, no wasm dependency, so the same code builds and tests on every
//! consumer: the server links it directly as an rlib (worldgen + `/content`
//! serving + worker speeds), and the webgl client reaches it through the
//! `resonantdust-shared` wasm bundle. Same content, same EXPLICIT ids, both
//! sides — the id scheme lives in the corpus itself (`docs/VARIABLES.md`).
//!
//!   - [`toml_loader`] — parse the per-category TOML tables (id law enforced).
//!   - [`loader`] — the materialized [`Bundle`]: every registry + flat table the
//!     consumers query, plus the declarative biome classifier (`generate`).
//!   - [`needs_eval`] — THE needs→conditions→mood evaluation (npc + client share it).
//!   - [`content`] — read the on-disk `content/` tree into loader sources
//!     (native/server only).

pub mod content;
pub mod loader;
pub mod needs_eval;
pub mod toml_loader;

pub use loader::{load, Bundle, LoadError};
