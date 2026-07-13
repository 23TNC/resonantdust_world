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

use crate::clock::ClockSnapshot;
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
/// pixijs debug HUD's "subs" tab. `table` is the wire tag (`state`); `rows` counts
/// the `Row` frames of that table streamed down a subscription and `rx` sums their
/// serialized bytes — the bulk of what a subscription pulls back (the calls tab
/// covers the outbound `sub_zone` / `unsub` control frames).
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
    /// Move the player's own object toward global tile `(tile_x, tile_y)`. The
    /// pixijs host sends this on a click; the server appends an `ACTION_MOVE` event
    /// to the shard's tick pipeline, which surfaces as a `state` row on the zone
    /// subscription. Requires a live session; ignored otherwise.
    Move { tile_x: i32, tile_y: i32 },
    /// Interact with the cold thing at global tile `(tile_x, tile_y)` — the pixijs host
    /// sends this on a right-click; the server `unpack`s the cold object (cold→hot) into a
    /// live `state` entity. Requires a live session; ignored otherwise.
    Interact { tile_x: i32, tile_y: i32 },
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
    /// A resolved entity from the shard's tick pipeline (`state`) changed in a
    /// subscribed zone. The raw u64 `entity_key` exceeds JS's 2^53 integer range, so it's
    /// decoded host-side into the entity's type (`obj_type`, an `entity_type`) and its
    /// 32-bit `object_id` (the minted `entity_id`, JS-safe), which the host keys the circle
    /// by; shown only when `obj_type == ENTITY_TYPE_DEMO`/`PLAYER`. `removed` on delete.
    StateObject {
        zone_id: u32,
        obj_type: u8,
        object_id: u64,
        /// The entity's `kind` — its content thing/pawn id, which the host resolves to a
        /// sprite (a wolf carries the wolf thing id). Distinct from `obj_type` (the
        /// entity class from the key: demo / player / pawn).
        kind: u16,
        tic: u32,
        location: u8,
        /// Facing (0=south, 1=east, 2=north, 3=west) and sub-tile `offset`
        /// (`x_off:4 | y_off:4`), so a moving pawn renders with the right facing sprite.
        rotation: u8,
        offset: u8,
        removed: bool,
    },
    /// A subscribed zone's cold-object row arrived (a module's generic `cold` table —
    /// the object model): the shared `object_type_reference` (type / subtype = biome /
    /// layer) plus its members as `object_kind_reference`s. A `biome-tile` row is the
    /// dense ground for one biome, a `biome-thing` row the sparse scatter. The host
    /// decodes each `object_kind_reference` (`shared/codec` `object`) to position + kind
    /// + variant and expands it via the DSL. Supersedes [`Event::ZoneTiles`] /
    /// [`Event::ZoneThings`]. Fired per cold row for the zone (seed + any later rewrite).
    ColdObjects { zone_id: u32, type_reference: u32, kinds: Vec<u32> },
    /// A zone's subscription closed (the anchor moved it out of range, or it was
    /// evicted). The host drops that zone's entities.
    ZoneClosed { zone_id: u32 },
    /// The running per-command gateway-call tally changed. Carries the full
    /// snapshot (one [`CallStat`] per command type seen so far), re-emitted after
    /// each outbound frame and each correlated reply. Diagnostic-only — the pixijs
    /// debug HUD's "calls" tab renders it; other hosts ignore it.
    CallStats(Vec<CallStat>),
    /// The clock estimate advanced — a fresh [`ClockSnapshot`] from a login seed
    /// or a ping/pong round-trip. The host stores the implied offset
    /// (`server_now_ms − now`) and renders the world at `now + offset − delay`,
    /// so all clients agree on the instant they display. Also drives the debug
    /// HUD's "sync" tab.
    ClockSync(ClockSnapshot),
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
