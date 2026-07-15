//! Client ⇄ server wire protocol.
//!
//! JSON over a single WebSocket for now — easy to read in a browser devtools
//! frame inspector while the protocol is in flux. The old game framed an
//! equivalent `ClientMsg`/`GateMsg` pair as postcard binary in a shared
//! `resonantdust-protocol` crate; when this protocol settles it can move to
//! `shared/` and gain a binary codec, with the client and server agreeing by
//! construction. Until then this module is the single source of truth.
//!
//! Tagged enums use an internal `"t"` discriminator so a frame is a flat object
//! like `{"t":"login","cid":1,"client_time_ms":...,"name":"Alice"}`.

use serde::{Deserialize, Serialize};

/// A frame the client sends to the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Trust-on-first-use login: claim or create the player named `name`. The
    /// server relays to the `players` DB's `claim_or_login`, then reads back the
    /// resulting `player_id` + `data_shard` and binds this connection's session.
    /// `cid` correlates the [`ServerMsg::LoginOk`] / [`ServerMsg::LoginErr`] reply.
    Login {
        cid: u32,
        /// Client clock sample (ms since unix epoch). Forwarded to the reducer
        /// for wire-format parity; the auth DB stamps at server time.
        client_time_ms: u64,
        name: String,
    },
    /// Subscribe to a single zone's live data. The server resolves
    /// `zone_id → region → shard` via the index, connects to that shard if it hasn't
    /// already, and streams the zone's `state` rows (the tick pipeline's entities) back
    /// as [`ServerMsg::Row`]. `sid` is a client-chosen subscription id used to
    /// [`Unsub`](ClientMsg::Unsub) later.
    SubZone { sid: u32, zone_id: u32 },
    /// Drop a subscription previously opened with the same `sid`.
    Unsub { sid: u32 },
    /// Clock-sync probe: the client's wall clock at send. The server replies with
    /// [`ServerMsg::Pong`], echoing `client_send_ms` and adding its own clock, so
    /// the client can estimate the offset from the round-trip.
    Ping { client_send_ms: u64 },
    /// Materialize an entity through the tick pipeline: the server appends an
    /// `ACTION_SPAWN` event targeting `entity_key`, which resolves into a live `state` row
    /// (`kind` + placement). The one event-driven spawn path — an automated player mints
    /// its pawns this way rather than via a privileged direct write. Requires the zone's
    /// shard to be connected (subscribe first). No reply frame.
    Spawn {
        entity_key: u64,
        kind: u16,
        tile_x: i32,
        tile_y: i32,
    },
    /// Move an object toward global tile `(tile_x, tile_y)`. `pawn` is `None` to move the
    /// caller's **own** object (a self-move) or `Some(entity_key)` to move that specific
    /// entity (an automated player driving its wolves). The server appends an `ACTION_MOVE`
    /// event; no reply frame (the effect arrives as a `state` row on the zone
    /// subscription). Authority: today any logged-in session may move any pawn — no
    /// ownership check yet.
    Move {
        #[serde(default)]
        pawn: Option<u64>,
        tile_x: i32,
        tile_y: i32,
    },
    /// Interact with the cold thing at global tile `(tile_x, tile_y)` — the server
    /// `unpack`s it (cold→hot), so it becomes a live `state` entity. No reply frame (the
    /// effect arrives as a `state` insert + a `cold` update on the zone subscription).
    Interact { tile_x: i32, tile_y: i32 },
    /// Debug freeze: the server calls the shard's `set_paused`, so the master stops
    /// advancing the tic (the whole simulation halts). The new state relays back to every
    /// subscriber as [`ServerMsg::Paused`]. Requires a subscribed shard.
    SetPaused { paused: bool },
}

/// A frame the server sends to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Login succeeded: the connection is now bound to `player_id`, whose cards
    /// live on `data_shard`. `server_micros` is the server's wall clock at reply
    /// time (µs since unix epoch) — the client seeds its clock offset from it.
    LoginOk {
        cid: u32,
        player_id: u32,
        data_shard: u16,
        server_micros: u64,
    },
    /// Login failed (reserved name, validation error, upstream timeout, …).
    LoginErr {
        cid: u32,
        error: String,
        server_micros: u64,
    },
    /// Reply to [`ClientMsg::Ping`]: `client_send_ms` echoed back (the round-trip
    /// correlator) and `server_ms`, the server wall clock (ms) at reply time.
    Pong { client_send_ms: u64, server_ms: u64 },
    /// A zone subscription's initial rows have all been delivered.
    Applied { sid: u32 },
    /// One upstream row insert/update/delete. `sid` is `0` — the client routes
    /// by the row's `zone_id` (and table), since one shard connection multiplexes
    /// every zone subscribed on it. The shape is in [`RowData`].
    Row { sid: u32, op: RowOp, row: RowData },
    /// A protocol- or routing-level error not tied to a single `cid`.
    Error { error: String },
    /// The simulation's freeze state on a subscribed shard changed (debug `/pause`). Sent to
    /// every subscriber when the shard's `tic_meta.paused` flips (and once on subscribe), so
    /// tic-driven actors (npc) can stop/resume issuing commands.
    Paused { paused: bool },
}

/// The kind of upstream row change a [`ServerMsg::Row`] carries.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOp {
    Insert,
    Update,
    Delete,
}

/// A relayed shard row, tagged by its source table. Mirrors the generated SpacetimeDB
/// row types field-for-field; the generated types derive *sats* `Serialize` (not serde),
/// so we copy into these plain serde structs. Keep in sync with the `shard` module.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "table", rename_all = "snake_case")]
pub enum RowData {
    /// A resolved entity from the shard's tick pipeline (`shard::state`) — the
    /// client-visible present per entity in a subscribed zone.
    State(StateRow),
    /// A zone's cold-object row from a module's `cold` table (object model) — the shared
    /// `object_type_reference` (type/subtype=biome/layer) plus a `Vec<object_kind_reference>`.
    /// `biome-tile` rows are the ground, `biome-thing` rows the scatter. Supersedes
    /// `ZoneTiles`/`ZoneThings` (`docs/object-model.md`).
    ColdObjects(ColdObjectsRow),
}

/// Mirror of a module's `cold_type::Cold` — one cold-object row: the shared
/// `object_type_reference` for a `(zone, type, subtype=biome, layer)`, plus its members
/// as `object_kind_reference`s (`kind/subkind/variant/x/y/data`). Keep in sync with the
/// generic `cold` table on `decl_tick_pipeline!`.
#[derive(Debug, Clone, Serialize)]
pub struct ColdObjectsRow {
    /// The world-global zone the edge reconstructs from the shard's realm + the row's
    /// `macro_position` (the row itself never repeats the realm — the shard implies it).
    pub zone_id: u32,
    /// `type_id:4 | subtype_id:12`.
    pub type_reference: u16,
    /// The row's tile-slot — part of its identity (`macro_position | type_reference | layer_id`).
    pub layer_id: u8,
    /// `kind_pos_reference` per object: `kind_reference:16 | tile_reference:8 | data:8`.
    pub kinds: Vec<u32>,
}

/// Mirror of `shard::state_type::State` — one resolved entity. `entity_key`
/// carries the tagged object/zone id; `tic` is how current this entity is (for later
/// staleness display). The client identifies demo objects by the type in the id.
#[derive(Debug, Clone, Serialize)]
pub struct StateRow {
    pub entity_key: u64,
    pub tic: u32,
    pub kind: u16,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub offset: u8,
    pub data_0: u64,
    pub data_1: u64,
}

impl ServerMsg {
    /// Serialize to a JSON text frame. Infallible in practice (these types are
    /// all plain data); on the impossible error we fall back to a generic error
    /// frame so a single bad row can't take down the connection.
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|e| {
            format!("{{\"t\":\"error\",\"error\":\"serialize failed: {e}\"}}")
        })
    }
}
