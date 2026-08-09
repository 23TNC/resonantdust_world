//! The host-facing contract: the [`Command`]s driven *into* the client and the
//! [`Event`]s it emits *out*, plus the [`EventSink`] a host implements to receive
//! them.
//!
//! This is the seam that lets "stuff call into" the client — it's deliberately
//! pure data and host-agnostic (no tokio, no transport), so the same types serve
//! a native Rust driver today and the webgl wasm bridge later. A host:
//!   1. constructs a [`Command`] and sends it in (via [`crate::Client`]), and
//!   2. supplies an [`EventSink`] (a closure, a channel, a JS callback shim …)
//!      that the client calls for every [`Event`].
//!
//! Login is the only verb so far; the enums grow as world interaction lands.

use crate::clock::ClockSnapshot;
use crate::zones::AnchorRadii;

/// One command type's gateway-call tally, surfaced via [`Event::CallStats`] for
/// the webgl debug HUD's "calls" tab. `command` is the wire tag (`login`,
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
/// webgl debug HUD's "subs" tab. `table` is the wire tag (`state`); `rows` counts
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
    /// zones the client subscribes to. The webgl host calls this once per
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
        radii: AnchorRadii,
        soul: u32,
    },
    /// Remove the anchor named `name`, closing any subscriptions only it held.
    RemoveAnchor { name: String },
    /// Queue a raw action program (`docs/ACTIONS.md`) into the simulation — the general world door.
    /// The typed helpers ([`Place`](Command::Place) / [`BuildWall`](Command::BuildWall)) compile to
    /// this; movement rides it too since input-rework F3 (the host composes
    /// `EXECUTE_INTERACTION(move_to)` — the raw `MOVE_TO` door is closed at the edge).
    /// Requires a live session; ignored otherwise.
    Queue { actions: Vec<u32> },
    /// Seed the tic estimator's RATE with a persisted hint (movement-hardening F5): a host that
    /// remembered the last learned `tics_per_sec` skips the ~60 s cold-page warmup. A HINT only —
    /// clamped to the estimator's band, ignored once the stream has anchored.
    SeedTicRate { tics_per_sec: f64 },
    /// Order walls built on the PERIMETER of the `(start..end)` tile rect (build-walls D5) —
    /// compiles to a `BUILD_WALL` program; the worker expands + queues the per-tile SETs.
    /// `object` is the wall kind's definition reference. Requires a live session.
    BuildWall {
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
        object: u32,
    },
    /// Place `entity` at global tile `(tile_x, tile_y)` and promote it into `state` — compiles to a
    /// `PROMOTE_STATE` + `PLACE` program. Bootstraps an entity the client addresses by `entity` (the
    /// spawn path until `CREATE`'s minted-id claim lands). Requires a live session; ignored otherwise.
    Place {
        entity: u32,
        tile_x: i32,
        tile_y: i32,
    },
    /// Drop the world-server connection and clear the session, without stopping
    /// the client (a later [`Command::Login`] can reconnect).
    Logout,
    /// Stop the client engine. The event task ends after this; the handle's
    /// further commands are no-ops.
    Shutdown,
}

/// An event the client emits to its host. Hosts react to these to render state
/// (webgl), advance a script (a Rust driver), or log (the headless CLI).
#[derive(Debug, Clone)]
pub enum Event {
    /// A [`Command::Login`] was accepted and the gateway round-trip has begun.
    LoginStarted { name: String },
    /// The gateway handed back a world server; the client is about to connect.
    ServerResolved(ServerInfo),
    /// Login succeeded — the session is live and bound to `player_id`, whose data
    /// lives on `player_shard_reference`. `server_url` is the world server now connected.
    LoggedIn {
        player_id: u32,
        player_shard_reference: u16,
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
    /// A composed entity in a subscribed zone changed (`state` insert/update) or left it
    /// (`removed` — a `StateGone`). `entity_reference` (`server_reference:8 | object_reference:24`)
    /// is JS-safe (fits 2^53) and keys the mover; its top nibble is the object type. The host draws
    /// `definition_reference` as the sprite at global tile `(tile_x, tile_y)` facing `facing`
    /// (0=south, 1=east, 2=north, 3=west), interpolating position by `tic`. `macro_position` is the
    /// entity's zone (`region:8 | zone:8`) — the wire address, carried through unchanged.
    StateObject {
        macro_position: u16,
        entity_reference: u32,
        definition_reference: u32,
        tile_x: i32,
        tile_y: i32,
        /// Subtile sixteenths (chord-movement F1) — a walking pawn's rows land mid-tile;
        /// `(0, 0)` for whole-tile positions (and everything pre-chord).
        sub_x: u8,
        sub_y: u8,
        facing: u8,
        tic: u16,
        removed: bool,
    },
    /// A pawn's decoded **part slots** (human-pawns P0) — from the payload sidecar's `PART`
    /// entries: `(slot, definition_reference)` pairs, the FULL def each part slot draws. Joined
    /// to the entity's `StateObject`s by the host (either may arrive first); unknown payload
    /// opcodes were skipped by their count. An entity with no payload never emits this.
    ///
    /// `payload` is the RAW opcode stream alongside the decoded parts (needs-moodlets P4):
    /// `NEED`/`CONDITION` entries are evaluated lazily by their consumers — the npc Brain through
    /// `resonantdust_content::needs_eval`, the webgl panel through the wasm `pawnConditions` — so the
    /// core decodes parts (its own join) and passes everything else through untouched.
    PawnParts { macro_position: u16, entity_reference: u32, tic: u16, parts: Vec<(u8, u32)>, payload: Vec<u32> },
    /// One `needs` sub-table row (stat-model F2): the packed gameplay row
    /// (`value:16 | kind:12 | variant:4`) + its `set_tic`. Joined by `entity_reference`.
    PawnNeed { macro_position: u16, entity_reference: u32, need: u32, set_tic: u16 },
    /// A pawn this session's player MINTED (npc-host I11) — the ownership set a module
    /// brain commands; replayed at login, live thereafter.
    OwnedPawn { entity_reference: u32 },
    /// One `inventory` sub-table row (inventory F2): a held item in `slot` — `item` is the
    /// thing's `definition_reference`, `0` = the slot emptied (drop/removal); `state` is
    /// RESERVED (the item-as-entity successor). Joined by `entity_reference`.
    PawnInventory { macro_position: u16, entity_reference: u32, slot: u8, item: u32, state: u32 },
    /// A subscribed zone's cold **ground** — the dense 256 `kind_reference`s of one biome-row,
    /// indexed by `tile_reference` (0..256). The host paints them as the terrain floor.
    /// `macro_position` (`region:8 | zone:8`) is the wire zone address (the host expands prims via
    /// `macro_world_origin`); `subtype_id` is the biome, `layer_id` the layer; `type_id` is
    /// `TYPE_BIOME_TILE`.
    ColdTiles { macro_position: u16, subtype_id: u16, layer_id: u8, tic: u16, tiles: Vec<u16> },
    /// A subscribed zone's cold **scatter** — sparse `kind_pos_reference`s (`kind:16 | tile:8 |
    /// data:8`), one per occupied cell of one biome-row. `type_id` is `TYPE_BIOME_THING`.
    ColdThings { macro_position: u16, subtype_id: u16, layer_id: u8, tic: u16, things: Vec<u32> },
    /// A cold **overlay** row — a per-cell mutation the host composites over the baseline. The cell is
    /// `position_reference`'s `tile_reference`; the sprite comes from `definition_reference`
    /// (`type_reference | kind_reference` → tile vs thing namespace + kind); `data` is rotation/count.
    /// `removed` clears the override (the baseline shows through). Keyed by `entity_reference`. `tic`
    /// orders the override against the baseline row's `tic` (most recent wins — resolves the arrival
    /// race between a cold-row update and its overlay).
    ColdState {
        macro_position: u16,
        entity_reference: u32,
        position_reference: u32,
        definition_reference: u32,
        data: u8,
        tic: u16,
        removed: bool,
    },
    /// A zone's subscription closed (the anchor moved it out of range, or it was
    /// evicted). The host drops that zone's entities. `macro_position` = the wire zone address.
    ZoneClosed { macro_position: u16 },
    /// The simulation's freeze state changed (debug `/pause`). `true` = frozen (the tic
    /// stopped, movement halts); `false` = running. Emitted to every subscriber when the
    /// shard's flag flips (and once on subscribe). Tic-driven hosts (npc) gate on this to
    /// stop/resume issuing commands; webgl surfaces it as a chat system line.
    Paused { paused: bool },
    /// The running per-command gateway-call tally changed. Carries the full
    /// snapshot (one [`CallStat`] per command type seen so far), re-emitted after
    /// each outbound frame and each correlated reply. Diagnostic-only — the webgl
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
    /// webgl debug HUD's "subs" tab renders it.
    SubStats {
        open: u32,
        total: u32,
        tables: Vec<SubStat>,
    },
    /// The wall↔tic estimate RE-ANCHORED (`ticclock` — a fresher wire tic arrived, or the old
    /// anchor aged out). The host computes fractional deltas locally:
    /// `serial(tic − t) + (now − wall_ms) · tics_per_sec / 1000`. `tics_per_sec` is the
    /// LEARNED rate (pawn-movement F6 — starts at `TIC_HZ`, refined from the stream; the true
    /// rate measurably drifts from the authored one). Sparse by construction; the first place
    /// clients care about tic at all (first-pawns P3).
    TicAnchor { tic: u16, wall_ms: f64, tics_per_sec: f64 },
    /// A promoted move INTENT reached a subscribed zone (`ACTIONS.md` §Movement):
    /// `entity_reference` is heading to global tile `(tile_x, tile_y)`, its first hop composed
    /// at `event_tic`. Clients SPECULATE position from this — per-hop state never fans out.
    MoveIntent {
        macro_position: u16,
        entity_reference: u32,
        tile_x: i32,
        tile_y: i32,
        event_tic: u16,
    },
    /// A pawn's INTENT-QUEUE snapshot reached a subscribed zone (intent-queue-ui F1 —
    /// the details panel's strip; `ACTIONS.md` § palette `QUEUE_STATE`). `entries` is
    /// the flat stride-4 payload verbatim: `[entry_id, interaction_ref, phase,
    /// started:16|fire:16] × n`, entry 0 = the ACTIVE event. Display truth only.
    QueueState {
        macro_position: u16,
        entity_reference: u32,
        event_tic: u16,
        entries: Vec<u32>,
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
