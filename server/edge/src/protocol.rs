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
//!
//! Scope: login + clock sync. The world frames — zone subscription, row streaming, and
//! client intent (spawn/move/interact/pause) — were removed with the shard and pipeline.
//! They are redesigned, not merely deleted: see `docs/intent/spacetime-again/`. The client
//! still sends some of them; an unknown `"t"` deserializes to an error frame, which is the
//! intended signal until the rebuild reinstates them.

use serde::{Deserialize, Serialize};

/// A frame the client sends to the server.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Trust-on-first-use login: claim or create the player named `name`. The
    /// server relays to the `players` DB's `claim_or_login`, then reads back the
    /// resulting `player_id` + `player_shard_reference` and binds this connection's session.
    /// `cid` correlates the [`ServerMsg::LoginOk`] / [`ServerMsg::LoginErr`] reply.
    Login {
        cid: u32,
        /// Client clock sample (ms since unix epoch). Forwarded to the reducer
        /// for wire-format parity; the auth DB stamps at server time.
        client_time_ms: u64,
        name: String,
    },
    /// Clock-sync probe: the client's wall clock at send. The server replies with
    /// [`ServerMsg::Pong`], echoing `client_send_ms` and adding its own clock, so
    /// the client can estimate the offset from the round-trip.
    Ping { client_send_ms: u64 },
    /// Client intent: an action program to queue into the simulation. The edge validates it (logged
    /// in · parses · — ownership/rate are future) and relays to `event_shard.queue`, which mints the
    /// event and stamps `event_tic = master + 3`. `cid` correlates the `QueueOk`/`QueueErr` reply.
    /// The program is the [`ACTIONS.md`](../../../docs/ACTIONS.md) `Vec<u32>` — the only door in.
    Queue { cid: u32, actions: Vec<u32> },
    /// Subscribe to a zone: stream its `state` (composed entities) and `event` (settled, promoted)
    /// rows. `zone` is a `macro_position_reference`. Idempotent — re-subscribing is a no-op.
    SubscribeZone { zone: u16 },
    /// Stop streaming a zone. Unknown/duplicate zone is a no-op.
    UnsubscribeZone { zone: u16 },
}

/// A frame the server sends to the client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    /// Login succeeded: the connection is now bound to `player_id`, whose cards
    /// live on `player_shard_reference`. `server_micros` is the server's wall clock at reply
    /// time (µs since unix epoch) — the client seeds its clock offset from it.
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
    /// Reply to [`ClientMsg::Ping`]: `client_send_ms` echoed back (the round-trip
    /// correlator) and `server_ms`, the server wall clock (ms) at reply time.
    Pong { client_send_ms: u64, server_ms: u64 },
    /// The intent was accepted into the queue. Correlates `Queue.cid`. (The minted `event_reference`
    /// isn't returned by the reducer; the client sees the event when it settles into a subscribed
    /// zone's `event` stream.)
    QueueOk { cid: u32 },
    /// The intent was rejected (not logged in, malformed program, upstream error). Correlates `cid`.
    QueueErr { cid: u32, error: String },
    /// A composed entity in a subscribed zone — sent on insert and update. The client interpolates by
    /// `tic`. `zone` is the row's `macro_position_reference`, echoed so a client tracking several
    /// zones can bucket it.
    State {
        entity_reference: u32,
        zone: u16,
        tic: u16,
        definition_reference: u32,
        position_reference: u32,
        data: u8,
    },
    /// A composed entity left a subscribed zone (its `state` row was deleted, e.g. despawned).
    StateGone { entity_reference: u32, zone: u16 },
    /// A settled, promoted event touching a subscribed zone (one per zone the event reached).
    Event { event_reference: u32, zone: u16, tic: u16, actions: Vec<u32> },
    /// A subscribed zone's **cold ground** — the dense 256 `kind_reference`s (index = `tile_reference`).
    /// Sent on the `cold_tile` row's insert/update; the client paints it as the terrain floor.
    ColdTile { zone: u16, layer_reference: u8, tiles: Vec<u16> },
    /// A subscribed zone's **cold scatter** — sparse `kind_pos_reference`s (`kind:16 | tile:8 |
    /// data:8`). Sent on `cold_thing` insert/update; the client paints things over the ground.
    ColdThing { zone: u16, layer_reference: u8, things: Vec<u32> },
    /// A protocol- or routing-level error not tied to a single `cid`.
    Error { error: String },
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
