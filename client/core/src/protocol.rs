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
    /// A pawn's **payload sidecar** row (human-pawns P0) — the entity's growable opcode stream
    /// (`opcode:16 | count:16` + operands; `PART = 1`: slot, def). Joined to the entity's `State`
    /// rows by `entity_reference` (either may arrive first). `tic` = last payload CONTENT change.
    Payload { entity_reference: u32, zone: u16, tic: u16, payload: Vec<u32> },
    /// One `needs` sub-table row (stat-model F2) — a pawn's single need, fanned alone.
    /// `need` is the packed gameplay row `value:16 | kind:12 | variant:4`; `set_tic`
    /// anchors the lazy eval. Joined by `entity_reference`.
    Need { entity_reference: u32, zone: u16, need: u64, set_tic: u16 },
    /// One `inventory` sub-table row (inventory F2); `item = 0` = the slot emptied.
    Inventory { entity_reference: u32, zone: u16, slot: u8, item: u32, state: u32 },
    /// A pawn THIS session's player minted (npc-host I11 — the ownership re-attach lane).
    Owned { entity_reference: u32 },
    /// A settled, promoted event touching a subscribed zone.
    Event {
        event_reference: u32,
        zone: u16,
        tic: u16,
        actions: Vec<u32>,
    },
    /// A subscribed zone's cold **ground** — dense 256 `kind_reference`s (index = `tile_reference`) of
    /// one biome-row. `subtype_id` = biome, `layer_id` = layer; `type_id` = `TYPE_BIOME_TILE`.
    ColdTile { zone: u16, subtype_id: u16, layer_id: u8, tic: u16, tiles: Vec<u16> },
    /// A subscribed zone's cold **scatter** — sparse `kind_pos_reference`s (`kind:16 | tile:8 | data:8`)
    /// of one biome-row. `type_id` = `TYPE_BIOME_THING`.
    ColdThing { zone: u16, subtype_id: u16, layer_id: u8, tic: u16, things: Vec<u32> },
    /// A **cold overlay** row (from a cold shard's `state`) — a per-cell mutation the host composites
    /// over the baseline at `position_reference`. `removed` = the override cleared. `tic` orders it
    /// against the baseline row's `tic` (most recent wins).
    ColdState {
        zone: u16,
        entity_reference: u32,
        position_reference: u32,
        definition_reference: u32,
        data: u8,
        tic: u16,
        removed: bool,
    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_payload_frame_decodes_to_part_slots() {
        // The edge's `Payload` frame for a 2-PART sidecar row (human-pawns P0) — the wire JSON
        // → `ServerMsg::Payload` → `codec::payload::payload_parts` yields both slots' defs.
        let body = resonantdust_codec::payload::part_entry(0, 0x3200_1237);
        let head = resonantdust_codec::payload::part_entry(1, 0x3200_123B);
        let words: Vec<u32> = body.iter().chain(head.iter()).copied().collect();
        let json = format!(
            r#"{{"t":"payload","entity_reference":48,"zone":258,"tic":7,"payload":{words:?}}}"#
        );
        let msg: ServerMsg = serde_json::from_str(&json).expect("frame parses");
        let ServerMsg::Payload { entity_reference, zone, tic, payload } = msg else {
            panic!("not a Payload frame");
        };
        assert_eq!((entity_reference, zone, tic), (48, 258, 7));
        assert_eq!(
            resonantdust_codec::payload::payload_parts(&payload),
            vec![(0, 0x3200_1237), (1, 0x3200_123B)]
        );
    }
}
