//! The host-facing contract: the [`Command`]s driven *into* the client and the
//! [`Event`]s it emits *out*, plus the [`EventSink`] a host implements to receive
//! them.
//!
//! This is the seam that lets "stuff call into" the client — it's deliberately
//! pure data and host-agnostic (no tokio, no transport), so the same types serve
//! a native Rust driver today and the pixijs wasm bridge later. A host:
//!   1. constructs a [`Command`] and sends it in (via [`crate::Client`]), and
//!   2. supplies an [`EventSink`] (a closure, a channel, a JS callback shim …)
//!      that the client calls for every [`Event`].
//!
//! Login is the only verb so far; the enums grow as world interaction lands.

use crate::zones::AnchorRadii;

/// A server the gateway resolved for the client to log into.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// The world server's id in the `index` directory.
    pub server_id: u16,
    /// The WebSocket endpoint to connect to (e.g. `ws://localhost:8473/ws`).
    pub url: String,
    /// `true` when the gateway reused an existing session pin (reconnect
    /// affinity), `false` when it freshly allocated a server.
    pub reused: bool,
}

/// A command a host drives into the client. One variant per externally-triggered
/// action; the client processes them in order on its own task.
#[derive(Debug, Clone)]
pub enum Command {
    /// Acquire a world server from the gateway, connect to it, and log in as
    /// `name` (trust-on-first-use: the player is created if new). Drives the
    /// [`Event::LoginStarted`] → [`Event::ServerResolved`] →
    /// [`Event::LoggedIn`]/[`Event::LoginFailed`] sequence.
    Login { name: String },
    /// Add or move an [`anchor`](crate::zones) named `name`, recomputing which
    /// zones the client subscribes to. The pixijs host calls this once per
    /// viewport and again as the view pans (idempotent when nothing moved).
    ///
    /// `tile_x` / `tile_y` are global tile coordinates of the anchor centre;
    /// `radii` are the tier reach in tiles (only the `active` ring opens subs, so
    /// size it to the visible area + margin — see [`crate::zones`]). `soul` is the
    /// pawn card id for a soul anchor, or `0` for a viewport. Requires a live
    /// session; ignored (with a [`Event::Status`]) when not logged in.
    SetAnchor {
        name: String,
        tile_x: i32,
        tile_y: i32,
        surface: u8,
        radii: AnchorRadii,
        soul: u32,
    },
    /// Remove the anchor named `name`, closing any subscriptions only it held.
    RemoveAnchor { name: String },
    /// Drop the world-server connection and clear the session, without stopping
    /// the client (a later [`Command::Login`] can reconnect).
    Logout,
    /// Stop the client engine. The event task ends after this; the handle's
    /// further commands are no-ops.
    Shutdown,
}

/// An event the client emits to its host. Hosts react to these to render state
/// (pixijs), advance a script (a Rust driver), or log (the headless CLI).
#[derive(Debug, Clone)]
pub enum Event {
    /// A [`Command::Login`] was accepted and the gateway round-trip has begun.
    LoginStarted { name: String },
    /// The gateway handed back a world server; the client is about to connect.
    ServerResolved(ServerInfo),
    /// Login succeeded — the session is live and bound to `player_id`, whose data
    /// lives on `data_shard`. `server_url` is the world server now connected.
    LoggedIn {
        player_id: u32,
        data_shard: u16,
        server_url: String,
    },
    /// A login attempt failed at some stage (gateway unreachable, no server,
    /// connect error, or a `login_err` reply). Carries a human-readable reason.
    LoginFailed { reason: String },
    /// The world-server connection ended (clean close, transport drop, or
    /// [`Command::Logout`]). `reason` is `None` for an intentional logout.
    Disconnected { reason: Option<String> },
    /// A non-fatal status line worth surfacing (e.g. a server-pushed
    /// [`crate::protocol::ServerMsg::Error`]).
    Status(String),
    /// A subscribed zone's cold baseline arrived: its `ZONE_TILES` packed tile
    /// slots (`def_id:12 | reserved:4` each, row-major). The host expands them to
    /// renderable cells — running the DSL `on_create` per `def_id` for the white
    /// texture + tint — and paints the zone. Fired on every `cold_zones` row for
    /// the zone (the initial seed and any later rewrite).
    ZoneTiles { zone_id: u32, tiles: Vec<u16> },
    /// A zone's subscription closed (the anchor moved it out of range, or it was
    /// evicted). The host drops that zone's sprites.
    ZoneClosed { zone_id: u32 },
}

/// A receiver for the client's [`Event`]s, implemented by each host. The client
/// calls [`emit`](EventSink::emit) from its engine task, so an implementation
/// must be cheap and non-blocking — forward to a channel / callback rather than
/// doing work inline.
///
/// The `Send + Sync` bound holds on native hosts, where the engine runs on a
/// multi-threaded tokio runtime (`tokio::spawn` requires a `Send` future). The
/// wasm host is single-threaded (`spawn_local`) and its sink wraps a
/// `js_sys::Function`, which is neither `Send` nor `Sync` — so the bound is
/// dropped on `wasm32`.
#[cfg(not(target_arch = "wasm32"))]
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: Event);
}
#[cfg(target_arch = "wasm32")]
pub trait EventSink: 'static {
    fn emit(&self, event: Event);
}

/// Any `Fn(Event)` is an [`EventSink`] — the ergonomic default. A host passes a
/// closure (forwarding to a channel, printing, or calling into JS) without
/// declaring a type.
#[cfg(not(target_arch = "wasm32"))]
impl<F> EventSink for F
where
    F: Fn(Event) + Send + Sync + 'static,
{
    fn emit(&self, event: Event) {
        self(event)
    }
}
#[cfg(target_arch = "wasm32")]
impl<F> EventSink for F
where
    F: Fn(Event) + 'static,
{
    fn emit(&self, event: Event) {
        self(event)
    }
}
