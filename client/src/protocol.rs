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
    /// Release the thing affixed at `(zone_id, location)` into the object shard.
    /// The server drives the transfer saga; no direct reply.
    Release { zone_id: u32, location: u8 },
    /// Clock-sync probe. `client_send_ms` is the client wall clock at send; the
    /// server echoes it in [`ServerMsg::Pong`] alongside its own clock, letting
    /// the client pin the offset from the round-trip. Sent periodically while
    /// connected.
    Ping { client_send_ms: u64 },
    /// Move the (debug) controllable thing toward global tile `(tile_x, tile_y)`.
    /// The server pathfinds from its current tile and commits the path as
    /// future-stamped move rows; no direct reply (the effect arrives on the
    /// existing free-thing subscription).
    Move { tile_x: i32, tile_y: i32 },
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
    /// The settled baseline for a zone (`zone_shard::cold_zones`).
    ColdZone(ColdZoneRow),
    /// A changed terrain cell overlaying cold (`zone_shard::hot_tiles`).
    HotTile(HotCellRow),
    /// A changed thing cell (`zone_shard::hot_things`).
    HotThing(HotCellRow),
    /// A loose thing from the object shard (`object_shard::free_things`) — a
    /// stable `object_id`, a tile `location`, and a sub-tile `offset`.
    FreeThing(FreeThingRow),
    /// A resolved entity from the object shard's tick pipeline (`object_shard::state`).
    State(StateRow),
}

/// Mirror of `shard::cold_zone_type::ColdZone`.
#[derive(Debug, Clone, Deserialize)]
pub struct ColdZoneRow {
    pub valid_at: u64,
    pub zone_id: u32,
    pub tiles: Vec<u16>,
    pub things: Vec<u32>,
}

/// Mirror of the two identical hot layer rows.
#[derive(Debug, Clone, Deserialize)]
pub struct HotCellRow {
    pub valid_at: u64,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
}

/// Mirror of `object_shard::free_thing_type::FreeThing` (server `FreeThingRow`).
#[derive(Debug, Clone, Deserialize)]
pub struct FreeThingRow {
    pub valid_at: u64,
    pub object_id: u64,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
    pub offset: u8,
}

/// Mirror of `object_shard::state_type::State` (server `StateRow`). No `valid_at` —
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
