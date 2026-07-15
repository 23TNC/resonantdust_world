//! resonantdust **headless client** — the game client as a library.
//!
//! There is no rendering here. This crate owns the *client logic* — talking to
//! the gateway and a world server, holding the session — and exposes it through a
//! host-agnostic command/event API ([`api`]). A *host* drives it:
//!   * `pixijs` will (via a wasm/FFI shim) become a dumb display over this client;
//!   * another Rust program (or the bundled [`headless` binary]) can drive it too.
//!
//! The shape is the same for every host: build a [`Client`], hand it an
//! [`EventSink`], send [`Command`]s in, render the [`Event`]s out.
//!
//! ```no_run
//! # async fn demo() {
//! use client::{Client, ClientConfig, Event};
//! let client = Client::spawn(ClientConfig::from_env(), |event: Event| {
//!     println!("{event:?}");
//! });
//! client.login("Alice").unwrap();
//! # }
//! ```
//!
//! [`headless` binary]: ../headless/index.html
//!
//! ## Login flow
//! [`Command::Login`] runs the two-hop handshake described in `docs/components/server/gateway/intent/gateway.md`:
//! ask the **gateway** for a world server, then connect to that server's
//! WebSocket and authenticate. See [`engine`] for the runtime and [`protocol`]
//! for the wire format.
//!
//! ## Layout
//! - [`api`] — the pure [`Command`] / [`Event`] / [`EventSink`] contract.
//! - [`config`] — [`ClientConfig`]: which gateway to ask.
//! - [`protocol`] — the client ⇄ world-server JSON frames.
//! - [`zones`] — the anchor-driven zone subscription manager (host-agnostic).
//! - `gateway` — the gateway round-trip; its parse is host-agnostic, the
//!   `reqwest` round-trip is `native`-only.
//! - `engine` / `web` — the runtime + transport, one per host: native (`native`,
//!   tokio + tungstenite + reqwest) or browser (`web`, `ws_stream_wasm` +
//!   `gloo-net` + `spawn_local`). Both expose the same [`Client`] handle.

pub mod api;
pub mod clock;
pub mod config;
pub mod gateway;
pub mod protocol;
pub mod zones;

#[cfg(feature = "native")]
mod engine;
#[cfg(feature = "web")]
mod web;

pub use api::{Command, Event, EventSink, ServerInfo};
pub use config::ClientConfig;
pub use zones::AnchorRadii;

#[cfg(feature = "native")]
pub use engine::{Client, SendError};
#[cfg(all(feature = "web", not(feature = "native")))]
pub use web::{Client, SendError};
