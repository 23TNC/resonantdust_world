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
    /// Release the thing affixed at `(zone_id, location)` out into the object
    /// shard (the Prison-Architect release). The server drives the transfer saga:
    /// `begin_release` on the zone shard → `receive` on the object shard → back to
    /// `ack_release`. Requires the zone to be subscribed (both shard upstreams
    /// live). No reply frame — the effect arrives as the affixed thing vanishing
    /// and a `free_thing` appearing on the client's existing subscriptions.
    Release { zone_id: u32, location: u8 },
    /// Clock-sync probe: the client's wall clock at send. The server replies with
    /// [`ServerMsg::Pong`], echoing `client_send_ms` and adding its own clock, so
    /// the client can estimate the offset from the round-trip.
    Ping { client_send_ms: u64 },
    /// Move the (debug) controllable thing toward global tile `(tile_x, tile_y)`.
    /// The server relays to the object shard's `move_debug_mover`, which pathfinds
    /// and commits the path; no reply frame (the effect arrives on the free-thing
    /// subscription).
    Move { tile_x: i32, tile_y: i32 },
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
    /// The settled baseline for a zone (`zone_shard::cold_zones`).
    ColdZone(ColdZoneRow),
    /// A changed terrain cell overlaying cold (`zone_shard::hot_tiles`).
    HotTile(HotCellRow),
    /// A changed thing cell (`zone_shard::hot_things`).
    HotThing(HotCellRow),
    /// A loose thing from the object shard (`object_shard::free_things`). Carries
    /// a stable `object_id` and a sub-tile `offset`; the client overlays these on
    /// top of the zone's cold+hot things.
    FreeThing(FreeThingRow),
    /// A resolved entity from the object shard's tick pipeline (`object_shard::state`).
    /// The client-visible present per entity; drives the moving demo circles.
    State(StateRow),
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
/// (`zone_shard::{hot_tile,hot_thing}_type`).
#[derive(Debug, Clone, Serialize)]
pub struct HotCellRow {
    pub valid_at: u64,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
}

/// Mirror of `object_shard::free_thing_type::FreeThing`. Like a hot thing plus a
/// stable `object_id` instance handle and a `u8` sub-tile `offset`
/// (`resonantdust_codec::packed`: `x_off:4 | y_off:4`).
#[derive(Debug, Clone, Serialize)]
pub struct FreeThingRow {
    pub valid_at: u64,
    pub object_id: u64,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub id: u16,
    pub offset: u8,
}

/// Mirror of `object_shard::state_type::State` — one resolved entity. `entity_key`
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
