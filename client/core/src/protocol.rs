//! Client ⇄ edge wire protocol — the client-side mirror of `server/edge/src/protocol.rs`.
//!
//! JSON over a single WebSocket, tagged with an internal `"t"` discriminator so a frame is a flat
//! object like `{"t":"login","cid":1,"client_time_ms":…,"name":"Alice"}`. The **edge** owns the
//! canonical definition; here the derives are flipped — we **serialize** [`ClientMsg`] (we send it)
//! and **deserialize** [`ServerMsg`] (we receive it). Keep the two files in lockstep until the
//! protocol settles into a shared crate both sides link.
//!
//! This is the rebuild's surface (`docs/intent/spacetime-again/`): the old bitemporal `StateRow` /
//! `Row` / `Applied` framing is gone. The world is now the two sim shards behind the edge — a client
//! `queue`s an action program and subscribes to a zone's composed `state` + settled `event` rows.

use serde::{Deserialize, Serialize};

/// A frame the client sends to the edge.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Trust-on-first-use login: claim or create the player named `name`. `cid` correlates the
    /// [`ServerMsg::LoginOk`] / [`ServerMsg::LoginErr`] reply.
    Login {
        cid: u32,
        /// Client clock sample (ms since unix epoch); the auth DB stamps authoritative time itself.
        client_time_ms: u64,
        name: String,
    },
    /// Clock-sync probe. `client_send_ms` is the client wall clock at send; the edge echoes it in
    /// [`ServerMsg::Pong`] with its own clock, so the client pins the offset from the round-trip.
    Ping { client_send_ms: u64 },
    /// A world intent: an action program (`docs/ACTIONS.md`) to queue into the simulation. `cid`
    /// correlates the [`ServerMsg::QueueOk`] / [`ServerMsg::QueueErr`] reply.
    Queue { cid: u32, actions: Vec<u32> },
    /// Subscribe to a zone's `state` + `event` streams. `zone` is a `macro_position_reference`
    /// (`region:8 | zone:8`) — the middle two bytes of the geographic `zone_id`.
    SubscribeZone { zone: u16 },
    /// Stop streaming a zone.
    UnsubscribeZone { zone: u16 },
}

/// A frame the edge sends to the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Login succeeded: the connection is bound to `player_id`, whose data lives on
    /// `player_shard_reference` (a `realm_server_reference` — its high byte is the realm).
    /// `server_micros` seeds the client clock offset.
    LoginOk {
        cid: u32,
        player_id: u32,
        player_shard_reference: u16,
        server_micros: u64,
    },
    /// Login failed (reserved name, validation error, upstream timeout, …).
    LoginErr {
        cid: u32,
        error: String,
        server_micros: u64,
    },
    /// Reply to [`ClientMsg::Ping`]. `client_send_ms` echoed back (the round-trip correlator);
    /// `server_ms` is the edge wall clock (ms) at reply.
    Pong { client_send_ms: u64, server_ms: u64 },
    /// A queued intent was accepted. Correlates [`ClientMsg::Queue`]'s `cid`.
    QueueOk { cid: u32 },
    /// A queued intent was rejected (not logged in, malformed program, upstream error).
    QueueErr { cid: u32, error: String },
    /// A composed entity in a subscribed zone — sent on insert and update. `zone` is the row's
    /// `macro_position_reference`. Interpolate by `tic`.
    State(StateRow),
    /// A composed entity left a subscribed zone (its `state` row was deleted).
    StateGone { entity_reference: u32, zone: u16 },
    /// A settled, promoted event touching a subscribed zone.
    Event {
        event_reference: u32,
        zone: u16,
        tic: u16,
        actions: Vec<u32>,
    },
    /// A subscribed zone's cold **ground** — dense 256 `kind_reference`s (index = `tile_reference`).
    ColdTile { zone: u16, layer_reference: u8, tiles: Vec<u16> },
    /// A subscribed zone's cold **scatter** — sparse `kind_pos_reference`s (`kind:16 | tile:8 | data:8`).
    ColdThing { zone: u16, layer_reference: u8, things: Vec<u32> },
    /// A protocol- or routing-level error not tied to a single `cid`.
    Error { error: String },
}

/// Mirror of the edge's `ServerMsg::State` payload — one composed `data_shard.state` row. The three
/// orthogonal references of the reference model plus its `tic` and `data` byte. `entity_reference`
/// is a `u32` (`server_reference:8 | object_reference:24`) — JS-safe, unlike the old u64 key.
#[derive(Debug, Clone, Deserialize)]
pub struct StateRow {
    pub entity_reference: u32,
    /// `macro_position_reference` — the zone this entity is in.
    pub zone: u16,
    pub tic: u16,
    pub definition_reference: u32,
    pub position_reference: u32,
    /// `rotation:2 | count:6` — facing in the top two bits.
    pub data: u8,
}
