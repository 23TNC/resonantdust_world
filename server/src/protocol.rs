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
    /// `zone_id → region → shard` via the index, connects to that shard's
    /// SpacetimeDB if it hasn't already, and streams the zone's `cold_zones` +
    /// `hot_*` rows back as [`ServerMsg::Row`]. `sid` is a client-chosen
    /// subscription id used to [`Unsub`](ClientMsg::Unsub) later.
    SubZone { sid: u32, zone_id: u32 },
    /// Drop a subscription previously opened with the same `sid`.
    Unsub { sid: u32 },
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
    /// A zone subscription's initial rows have all been delivered.
    Applied { sid: u32 },
    /// One upstream row insert/update/delete. `sid` is `0` — the client routes
    /// by the row's `zone_id` (and table), since one shard connection multiplexes
    /// every zone subscribed on it. The shape is in [`RowData`].
    Row { sid: u32, op: RowOp, row: RowData },
    /// A protocol- or routing-level error not tied to a single `cid`.
    Error { error: String },
}

/// The kind of upstream row change a [`ServerMsg::Row`] carries.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RowOp {
    Insert,
    Update,
    Delete,
}

/// A relayed shard row, tagged by its source table. These mirror the generated
/// SpacetimeDB row types field-for-field; the generated types derive *sats*
/// `Serialize` (not serde), so we copy into these plain serde structs rather
/// than pull in the sats→serde bridge. Keep them in sync with the `shard`
/// module's table definitions.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "table", rename_all = "snake_case")]
pub enum RowData {
    /// The settled baseline for a zone (`shard::cold_zones`).
    ColdZone(ColdZoneRow),
    /// A changed terrain cell overlaying cold (`shard::hot_tiles`).
    HotTile(HotCellRow),
    /// A changed thing cell (`shard::hot_things`).
    HotThing(HotCellRow),
}

/// Mirror of `shard::cold_zone_type::ColdZone`.
#[derive(Debug, Clone, Serialize)]
pub struct ColdZoneRow {
    pub valid_at: u64,
    pub zone_id: u32,
    pub tiles: Vec<u16>,
    pub things: Vec<u32>,
}

/// Mirror of the two identical hot layer rows
/// (`shard::{hot_tile,hot_thing}_type`).
#[derive(Debug, Clone, Serialize)]
pub struct HotCellRow {
    pub valid_at: u64,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
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
