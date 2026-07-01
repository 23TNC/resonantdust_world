//! resonantdust-dsl — the definition language that turns `content/*.rd` into a
//! runnable [`Bundle`].
//!
//! Pure Rust, no wasm dependency, so the same code builds and tests on every
//! consumer: the server links it directly as an rlib (the `:data` side — mapping
//! tile names to the `def_id`s it packs into a zone), and the pixijs client
//! reaches it through the `resonantdust-shared` wasm bundle (the `:visual` side —
//! mapping `def_id`s back to colours to paint).
//!
//! The pipeline mirrors the previous game's, trimmed to what the current content
//! needs:
//!   - [`parser`] — lex/parse `.rd` source into a block tree (defs, facets,
//!     code-bodied hooks). Game-agnostic.
//!   - [`vm`] — execute a hook body against an in-memory [`vm::Store`]: a postfix
//!     stack over a tree of [`vm::Cell`]s.
//!   - [`loader`] — index `<tile>` defs (merging `:data` + `:visual` facets),
//!     assign append-stable `def_id`s, and answer the name⇄id⇄colour queries the
//!     server and client run.
//!   - [`content`] — read the on-disk `content/{data,visual}` tree into loader
//!     sources (native/server only).

pub mod content;
pub mod loader;
pub mod parser;
pub mod vm;

pub use loader::{load, Bundle, LoadError};
