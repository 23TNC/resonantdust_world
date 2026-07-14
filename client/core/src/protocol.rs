//! Client ⇄ world-server wire protocol — the client-side mirror of
//! `server/src/protocol.rs`.
//!
//! JSON over a single WebSocket, tagged with an internal `"t"` discriminator so a
//! frame is a flat object like `{"t":"login","cid":1,"client_time_ms":…,"name":"Alice"}`.
//! The server owns the canonical definition; here the derives are flipped — we
//! **serialize** [`ClientMsg`] (we send it) and **deserialize** [`ServerMsg`] (we
//! receive it). Keep the two files in lockstep until the protocol settles and
//! moves into a shared crate both sides link (see the note in the server module).
//!
//! The full surface is mirrored, not just login, so that once the server starts
//! pushing `Applied` / `Row` frames an existing client deserializes them instead
//! of erroring on an unknown tag.

use serde::{Deserialize, Serialize};

/// A frame the client sends to the server.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Trust-on-first-use login: claim or create the player named `name`. `cid`
    /// correlates the [`ServerMsg::LoginOk`] / [`ServerMsg::LoginErr`] reply.
    Login {
        cid: u32,
        /// Client clock sample (ms since unix epoch). Forwarded to the reducer for
        /// wire-format parity; the auth DB stamps at server time.
        client_time_ms: u64,
        name: String,
    },
    /// Subscribe to a single zone's live data. `sid` is a client-chosen
    /// subscription id used to [`Unsub`](ClientMsg::Unsub) later.
    SubZone { sid: u32, zone_id: u32 },
    /// Drop a subscription previously opened with the same `sid`.
    Unsub { sid: u32 },
    /// Clock-sync probe. `client_send_ms` is the client wall clock at send; the
    /// server echoes it in [`ServerMsg::Pong`] alongside its own clock, letting
    /// the client pin the offset from the round-trip. Sent periodically while
    /// connected.
    Ping { client_send_ms: u64 },
    /// Materialize an entity: the server appends an `ACTION_SPAWN` event targeting
    /// `entity_key`, which the tick pipeline resolves into a live `state` row carrying
    /// `kind` + placement. The event-driven spawn path (an automated player mints its
    /// pawns this way); no direct reply.
    Spawn {
        entity_key: u64,
        kind: u16,
        tile_x: i32,
        tile_y: i32,
    },
    /// Move an object toward global tile `(tile_x, tile_y)`. `pawn` is `None` for the
    /// player's own object (a self-move) or `Some(entity_key)` for a specific entity. The
    /// server appends an `ACTION_MOVE` event; no direct reply (the effect arrives as a
    /// `state` row on the zone subscription). `pawn` is omitted from the frame when `None`,
    /// so an existing self-move client stays byte-compatible.
    Move {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pawn: Option<u64>,
        tile_x: i32,
        tile_y: i32,
    },
    /// Interact with the cold thing at global tile `(tile_x, tile_y)` — the server
    /// `unpack`s it (cold→hot) into a live `state` entity.
    Interact { tile_x: i32, tile_y: i32 },
    /// Debug: freeze / unfreeze the simulation on the client's current shard. The server
    /// calls `set_paused`, so the master stops advancing the tic; the new state is relayed
    /// back as [`ServerMsg::Paused`] to every subscriber (so tic-driven actors pause too).
    SetPaused { paused: bool },
}

/// A frame the server sends to the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Login succeeded: the connection is now bound to `player_id`, whose cards
    /// live on `data_shard`. `server_micros` is the server's wall clock at reply
    /// time (µs since unix epoch) — seed the client clock offset from it.
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
    /// Reply to [`ClientMsg::Ping`]. `client_send_ms` is echoed back verbatim (the
    /// round-trip correlator); `server_ms` is the server wall clock (ms) at reply.
    Pong { client_send_ms: u64, server_ms: u64 },
    /// A zone subscription's initial rows have all been delivered.
    Applied { sid: u32 },
    /// One upstream row insert/update/delete. `sid` is `0` — route by the row's
    /// `zone_id` (and table), since one shard connection multiplexes every zone.
    Row { sid: u32, op: RowOp, row: RowData },
    /// A protocol- or routing-level error not tied to a single `cid`.
    Error { error: String },
    /// The simulation's freeze state on a subscribed shard changed (debug `/pause`). Sent to
    /// every subscriber when the shard's `tic_meta.paused` flips (and once on subscribe), so
    /// tic-driven actors (npc) can stop/resume issuing commands.
    Paused { paused: bool },
}

/// The kind of upstream row change a [`ServerMsg::Row`] carries.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOp {
    Insert,
    Update,
    Delete,
}

/// A relayed shard row, tagged by its source table. Mirrors the server's wire
/// structs field-for-field.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "table", rename_all = "snake_case")]
pub enum RowData {
    /// A resolved entity from the shard's tick pipeline (`shard::state`).
    State(StateRow),
    /// A zone's cold-object row from a module's generic `cold` table (object model): the
    /// shared `object_type_reference` (type/subtype=biome/layer) + a
    /// `Vec<object_kind_reference>`. `biome-tile` rows = ground, `biome-thing` = scatter.
    ColdObjects(ColdObjectsRow),
}

/// Mirror of a module's `cold_type::Cold` — one cold-object row: the shared
/// `object_type_reference` for a `(zone, type, subtype=biome, layer)`, plus its members
/// as `object_kind_reference`s.
#[derive(Debug, Clone, Deserialize)]
pub struct ColdObjectsRow {
    pub zone_id: u32,
    pub type_reference: u32,
    pub kinds: Vec<u32>,
}

/// Mirror of `shard::state_type::State` (server `StateRow`). No `valid_at` —
/// the tick pipeline is tic-based, not bitemporal.
#[derive(Debug, Clone, Deserialize)]
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
