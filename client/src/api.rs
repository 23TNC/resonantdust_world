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

/// One command type's gateway-call tally, surfaced via [`Event::CallStats`] for
/// the pixijs debug HUD's "calls" tab. `command` is the wire tag (`login`,
/// `sub_zone`, `unsub`, `release`); `requests` / `tx` count outbound frames of
/// that type and their serialized bytes; `ok` / `err` / `rx` count the correlated
/// replies and their bytes (only `login` yields `login_ok` / `login_err`, only
/// `sub_zone` yields `applied` — the fire-and-forget frames have no reply).
#[derive(Debug, Clone)]
pub struct CallStat {
    pub command: String,
    pub requests: u32,
    pub ok: u32,
    pub err: u32,
    pub tx: u64,
    pub rx: u64,
}

/// One relayed-row table's data tally, surfaced via [`Event::SubStats`] for the
/// pixijs debug HUD's "subs" tab. `table` is the wire tag (`cold_zone` /
/// `hot_tile` / `hot_thing` / `free_thing`); `rows` counts the `Row` frames of
/// that table streamed down a subscription and `rx` sums their serialized bytes —
/// the bulk of what a subscription pulls back (the calls tab covers the outbound
/// `sub_zone` / `unsub` control frames).
#[derive(Debug, Clone)]
pub struct SubStat {
    pub table: String,
    pub rows: u32,
    pub rx: u64,
}

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
    /// Release the thing affixed at `(zone_id, location)` out of the world into an
    /// object shard (Prison-Architect release). The server drives the cross-shard
    /// transfer; the result surfaces on existing subscriptions (the affixed thing
    /// vanishes, a [`Event::ZoneFreeThing`] appears). Requires a live session and
    /// the zone to be subscribed; ignored otherwise.
    Release { zone_id: u32, location: u8 },
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
    /// A subscribed zone's cold thing baseline: the packed things in
    /// `cold_zones.things` (`x:4 | y:4 | rotation:2 | object_id:12` each, in the
    /// thing def-id namespace). The host expands them to sprites — one per thing,
    /// positioned by the codec and tinted by the thing def's `:visual`. Fired
    /// alongside [`Event::ZoneTiles`] on every `cold_zones` row (seed or rewrite);
    /// an empty vec means the host clears the zone's things. These are the
    /// worldgen-scattered flora, distinct from the object-shard
    /// [`ZoneFreeThing`](Event::ZoneFreeThing) loose things.
    ZoneThings { zone_id: u32, things: Vec<u32> },
    /// A loose thing in a subscribed zone changed (object shard `free_things`).
    /// `removed` is `true` for a delete — the host drops the sprite keyed by
    /// `object_id`; otherwise it upserts one at the tile `location` shifted by the
    /// sub-tile `offset` (`resonantdust_codec::packed`: `x_off:4 | y_off:4`). `id`
    /// is the thing's what-kind def. Insert and update both arrive as `removed:
    /// false` — the host keys by `object_id`, so an upsert covers both.
    ZoneFreeThing {
        zone_id: u32,
        object_id: u64,
        removed: bool,
        location: u8,
        rotation: u8,
        id: u16,
        offset: u8,
    },
    /// A zone's subscription closed (the anchor moved it out of range, or it was
    /// evicted). The host drops that zone's sprites — tiles and loose things.
    ZoneClosed { zone_id: u32 },
    /// The running per-command gateway-call tally changed. Carries the full
    /// snapshot (one [`CallStat`] per command type seen so far), re-emitted after
    /// each outbound frame and each correlated reply. Diagnostic-only — the pixijs
    /// debug HUD's "calls" tab renders it; other hosts ignore it.
    CallStats(Vec<CallStat>),
    /// The subscription data tally changed. `open` is the live count of zone
    /// subscriptions currently on the wire; `total` is every `sub_zone` ever sent
    /// (cumulative); `tables` is the per-table row/byte breakdown. Re-emitted when
    /// a subscription opens/closes or a `Row` frame lands. Diagnostic-only — the
    /// pixijs debug HUD's "subs" tab renders it.
    SubStats {
        open: u32,
        total: u32,
        tables: Vec<SubStat>,
    },
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
