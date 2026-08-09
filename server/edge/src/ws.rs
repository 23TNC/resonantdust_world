//! Per-client WebSocket session — the inbound half of the server.
//!
//! One [`client_session`] task per connected client. It owns:
//!   * a **players** upstream (for login), subscribed to the auth table so the
//!     post-`claim_or_login` row read hits a warm cache;
//!   * the gate-owned session: WS → `player_id`, set at login.
//!
//! Scope: login + clock sync, plus the world surface — zone subscription and the cold
//! `tile`/`thing` relay ([`build_world`]/[`seed_zone`]) — rebuilt on the pipeline
//! (`docs/intent/spacetime-again/`). A zone maps directly to its shard DBs on this
//! server (single instance); the old `region → shard endpoint` router is retired
//! (coord-purge C), to be replaced macro-keyed when multi-shard is a real need.
//!
//! Outbound frames come from two places — the request/response handlers on this async
//! task, and the SDK row callbacks on the upstreams' threads — so they're funneled
//! through one unbounded channel to a single writer task that owns the WS sink.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use spacetimedb_sdk::{DbContext, Table as _, TableWithPrimaryKey as _};
use tokio::sync::{mpsc, oneshot};

use crate::bindings;
use crate::bindings::data_shard::entity_state_table::EntityStateTableAccess as _;
use crate::bindings::pawn::entity_state_table::EntityStateTableAccess as _;
use crate::bindings::pawn::inventory_table::InventoryTableAccess as _;
use crate::bindings::pawn::needs_table::NeedsTableAccess as _;
use crate::bindings::pawn::payload_table::PayloadTableAccess as _;
use crate::bindings::event_shard::event_table::EventTableAccess as _;
use crate::bindings::thing::seed as _; // reducer trait → thing.reducers().seed
use crate::bindings::thing::place_things as _; // reducer trait → thing.reducers().place_things
use crate::bindings::thing::entity_state_table::EntityStateTableAccess as _; // cold baseline
use crate::bindings::thing::overlay_table::OverlayTableAccess as _; // cold override
use crate::bindings::tile::seed as _; // reducer trait → tile.reducers().seed
use crate::bindings::tile::entity_state_table::EntityStateTableAccess as _; // cold baseline
use crate::bindings::tile::overlay_table::OverlayTableAccess as _; // cold override
use crate::bindings::event_shard::queue as _; // reducer trait → `reducers().queue_then`
use crate::bindings::players::claim_or_login as _; // reducer trait → `reducers.claim_or_login_then`
use crate::connections::{
    await_ready, connect_data_shard, connect_event_shard, connect_pawn, connect_players,
    connect_thing, connect_tile, Pool,
};
use crate::protocol::{ClientMsg, ServerMsg};
use crate::worldgen::Worldgen;

/// The per-client world upstreams: the two sim shards. `event_shard` takes the client's `queue`
/// intents and streams settled `event` rows; `data_shard` streams composed `state` rows. Each client
/// gets its own pair so their zone subscriptions don't collide (the set-semantics hazard).
struct World {
    event: Option<Arc<bindings::event_shard::DbConnection>>,
    data: Option<Arc<bindings::data_shard::DbConnection>>,
    pawn: Option<Arc<bindings::pawn::DbConnection>>,
    tile: Option<Arc<bindings::tile::DbConnection>>,
    thing: Option<Arc<bindings::thing::DbConnection>>,
}

/// How long to wait for `claim_or_login` to commit before failing the login.
const REDUCER_CALL_TIMEOUT: Duration = Duration::from_secs(5);
/// Post-login player-row read budget: poll the players cache this many times,
/// `POLL_INTERVAL` apart, for the row the reducer just wrote to land.
const PLAYER_READ_POLLS: u32 = 20;
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// `GET /ws` — upgrade to a WebSocket and run a [`client_session`].
pub async fn handler(ws: WebSocketUpgrade, State(pool): State<Arc<Pool>>) -> Response {
    ws.on_upgrade(move |socket| client_session(socket, pool))
}

/// Server wall clock in microseconds since the unix epoch — stamped onto login
/// replies so the client can seed its clock offset.
fn server_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

async fn client_session(socket: WebSocket, pool: Arc<Pool>) {
    let (mut sink, mut stream) = socket.split();

    // Outbound to the WS sink, drained by one writer task. Two lanes:
    //   * `out_tx` — the normal funnel: the stateful worker's replies and the SDK
    //     row callbacks on upstream threads. Can be a firehose and the frames large.
    //   * `pong_tx` — a priority lane for clock-sync Pongs only. The writer drains it
    //     *ahead* of `out_tx`, so a Pong never departs behind a backlog of row frames.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let (pong_tx, mut pong_rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                Some(txt) = pong_rx.recv() => {
                    if sink.send(Message::Text(txt.into())).await.is_err() {
                        break;
                    }
                }
                Some(txt) = out_rx.recv() => {
                    if sink.send(Message::Text(txt.into())).await.is_err() {
                        break;
                    }
                }
                else => break,
            }
        }
    });

    // Players upstream, built eagerly: login is the first thing most clients do, and the
    // row read needs the subscription warm. A failure leaves `players = None`; login then
    // reports the outage instead of hanging.
    let players = build_players(&pool, &out_tx).await;
    // World upstreams (the sim shards) — built eagerly and wired to relay row callbacks into this
    // connection's outbound funnel. A failure leaves the side `None`; queue/subscribe then report it.
    let world = build_world(&pool, &out_tx).await;

    // Stateful commands run on a dedicated worker task so the read loop never blocks on a
    // slow DB handler; the read loop answers pings inline and forwards everything else
    // over an ordered channel (per-connection command order is preserved).
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ClientMsg>();
    let worker = tokio::spawn(session_worker(pool.clone(), players, world, out_tx.clone(), cmd_rx));

    while let Some(frame) = stream.next().await {
        let msg = match frame {
            Ok(m) => m,
            Err(err) => {
                tracing::debug!(%err, "ws stream error; closing");
                break;
            }
        };
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            Message::Binary(_) | Message::Ping(_) | Message::Pong(_) => continue,
        };

        let cmsg: ClientMsg = match serde_json::from_str(text.as_str()) {
            Ok(m) => m,
            Err(err) => {
                send(&out_tx, ServerMsg::Error { error: format!("bad frame: {err}") });
                continue;
            }
        };

        match cmsg {
            // Clock-sync probe: stamped + answered right here on the read loop, on the
            // priority lane, so neither a slow worker handler nor a row backlog delays it.
            ClientMsg::Ping { client_send_ms } => {
                send(
                    &pong_tx,
                    ServerMsg::Pong { client_send_ms, server_ms: now_ms() },
                );
            }
            other => {
                if cmd_tx.send(other).is_err() {
                    break; // worker gone — nothing left to serve
                }
            }
        }
    }

    // Closing the command channel drains the worker, which owns session teardown.
    drop(cmd_tx);
    let _ = worker.await;
    writer.abort();
    tracing::debug!("client session ended");
}

/// One live zone subscription: the `state` (data shard) and `event` (event shard) handles. Dropping
/// them unsubscribes, so removing a zone from the map is the unsubscribe.
struct ZoneSub {
    _state: bindings::data_shard::SubscriptionHandle,
    _pawn: Option<bindings::pawn::SubscriptionHandle>,
    _event: bindings::event_shard::SubscriptionHandle,
    _tile: Option<bindings::tile::SubscriptionHandle>,
    _thing: Option<bindings::thing::SubscriptionHandle>,
}

/// Drains stateful client commands in arrival order, owning all per-session state (the session id,
/// and the set of subscribed zones). Runs teardown once the channel closes.
async fn session_worker(
    pool: Arc<Pool>,
    players: Option<Arc<bindings::players::DbConnection>>,
    world: World,
    out_tx: mpsc::UnboundedSender<String>,
    mut cmd_rx: mpsc::UnboundedReceiver<ClientMsg>,
) {
    let mut session: Option<u32> = None;
    let mut zones: HashMap<u16, ZoneSub> = HashMap::new();

    while let Some(cmsg) = cmd_rx.recv().await {
        match cmsg {
            ClientMsg::Login { cid, client_time_ms, name } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
                // player-pawns P2: the mint funnel runs DETACHED after LoginOk (I6 — a
                // healing player_pawn shard can never block or fail a login; the next
                // login simply retries).
                if let (Some(player_id), Some(p)) = (session, players.as_ref()) {
                    tokio::spawn(ensure_player_pawn(pool.clone(), p.clone(), player_id));
                    // The OWNER fan (P3, F6): the session's own player-pawn rows, relayed
                    // on the EXISTING `Need` frame — a 0x40… ref is self-describing.
                    tokio::spawn(fan_player_pawn(pool.clone(), p.clone(), player_id, out_tx.clone()));
                }
            }
            ClientMsg::Queue { cid, actions } => {
                handle_queue(world.event.as_ref(), &out_tx, session, cid, actions);
            }
            ClientMsg::SubscribeZone { zone } => {
                // Seeding moved INTO the tile subscription's on_applied (lumberjack):
                // only a zone whose applied snapshot has NO baseline rows generates.
                handle_subscribe(&pool, &world, &out_tx, &mut zones, zone);
            }
            ClientMsg::UnsubscribeZone { zone } => {
                if zones.remove(&zone).is_some() {
                    tracing::debug!(zone, "unsubscribed zone"); // drop → unsubscribe
                }
            }
            // Pings are answered on the read loop and never forwarded here.
            ClientMsg::Ping { .. } => {}
        }
    }

    zones.clear(); // drop the handles → unsubscribe before the connections close
    if let Some(conn) = &players {
        let _ = conn.disconnect();
    }
    if let Some(conn) = &world.event {
        let _ = conn.disconnect();
    }
    if let Some(conn) = &world.data {
        let _ = conn.disconnect();
    }
    if let Some(conn) = &world.tile {
        let _ = conn.disconnect();
    }
    if let Some(conn) = &world.thing {
        let _ = conn.disconnect();
    }
    tracing::debug!(player = ?session, "client session worker ended");
}

/// Per-connection entity→zone tracker (movement-hardening P2, closing first-pawns I4). A hot
/// row migrating between zone subscriptions fires a DELETE in the old zone (and an insert in
/// the new — order not guaranteed); relaying that delete as `StateGone` made every zone
/// crossing a phantom despawn (the browser dropped + re-added the pawn; the npc once
/// deadlocked). The tracker remembers where each entity was last relayed: a delete for an
/// entity now live in a DIFFERENT zone is swallowed outright, and a delete matching the
/// entity's known zone HOLDS for [`GONE_HOLD_MS`] and relays only if no reappearance lands —
/// a real removal still relays, one beat late.
/// `entity → (zone, seq)` — `seq` bumps on every relayed insert/update, so ANY reappearance
/// (even same-zone delete+reinsert churn) is distinguishable from silence during the hold.
type ZoneMap = Arc<Mutex<HashMap<u32, (u16, u64)>>>;

/// The reappearance window — ~2 tics; subscription migration settles well inside it.
const GONE_HOLD_MS: u64 = 400;

/// Note a relayed insert/update in the tracker.
fn note_state(zones: &ZoneMap, entity_reference: u32, zone: u16) {
    let mut map = zones.lock().unwrap_or_else(|e| e.into_inner());
    let e = map.entry(entity_reference).or_insert((zone, 0));
    e.0 = zone;
    e.1 = e.1.wrapping_add(1);
}

/// Relay-or-swallow a hot-shard delete through the tracker (shared by data + pawn relays).
/// `rt` is the server runtime — the SDK fires callbacks on its own thread, where a bare
/// `tokio::spawn` would panic.
fn relay_gone(
    zones: &ZoneMap,
    out: &mpsc::UnboundedSender<String>,
    rt: &tokio::runtime::Handle,
    entity_reference: u32,
    zone: u16,
) {
    let seen = zones.lock().unwrap_or_else(|e| e.into_inner()).get(&entity_reference).copied();
    if let Some((z, _)) = seen {
        if z != zone {
            return; // already relayed from another zone — a migration artifact, not a despawn
        }
    }
    let zones = zones.clone();
    let out = out.clone();
    rt.spawn(async move {
        tokio::time::sleep(Duration::from_millis(GONE_HOLD_MS)).await;
        {
            let mut map = zones.lock().unwrap_or_else(|e| e.into_inner());
            if map.get(&entity_reference).copied() != seen {
                return; // reappeared during the hold (any zone, or same-zone churn) — swallow
            }
            map.remove(&entity_reference);
        }
        send(&out, ServerMsg::StateGone { entity_reference, zone });
    });
}

/// Build the per-client world upstreams and register the row callbacks that relay `state`/`event`
/// rows into this connection's outbound funnel. The callbacks fire for whatever the zone
/// subscriptions (added later) put in the cache — the row carries its own `macro_position_reference`,
/// so one set of callbacks serves every subscribed zone.
async fn build_world(pool: &Arc<Pool>, out_tx: &mpsc::UnboundedSender<String>) -> World {
    let event = match connect_event_shard(&pool.cfg.uri, &pool.cfg.event_shard_db()) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if event.is_none() {
        send(out_tx, err_frame("event shard upstream unavailable"));
    }
    let data = match connect_data_shard(&pool.cfg.uri, &pool.cfg.data_shard_db()) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if data.is_none() {
        send(out_tx, err_frame("data shard upstream unavailable"));
    }

    // One tracker per client connection, shared by BOTH hot shards (an entity lives on one
    // shard, so the keyspace never collides) — deletes route through `relay_gone` (P2/I4).
    let zone_map: ZoneMap = Arc::new(Mutex::new(HashMap::new()));
    let rt = tokio::runtime::Handle::current();

    if let Some(d) = &data {
        let o = out_tx.clone();
        let zm = zone_map.clone();
        d.db().entity_state().on_insert(move |_ctx, row| {
            note_state(&zm, row.entity_reference, row.macro_position_reference);
            send(&o, state_frame(row))
        });
        let o = out_tx.clone();
        let zm = zone_map.clone();
        d.db().entity_state().on_update(move |_ctx, _old, row| {
            note_state(&zm, row.entity_reference, row.macro_position_reference);
            send(&o, state_frame(row))
        });
        let o = out_tx.clone();
        let zm = zone_map.clone();
        let h = rt.clone();
        d.db().entity_state().on_delete(move |_ctx, row| {
            relay_gone(&zm, &o, &h, row.entity_reference, row.macro_position_reference)
        });
    }
    // The pawn shard — the hot movers (first-pawns). Same relay shape as `data_shard`.
    let pawn = match connect_pawn(&pool.cfg.uri, &pool.cfg.pawn_db()) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if pawn.is_none() {
        send(out_tx, err_frame("pawn shard upstream unavailable"));
    }
    if let Some(p) = &pawn {
        let o = out_tx.clone();
        let zm = zone_map.clone();
        p.db().entity_state().on_insert(move |_ctx, row| {
            note_state(&zm, row.entity_reference, row.macro_position_reference);
            send(&o, pawn_state_frame(row))
        });
        let o = out_tx.clone();
        let zm = zone_map.clone();
        p.db().entity_state().on_update(move |_ctx, _old, row| {
            note_state(&zm, row.entity_reference, row.macro_position_reference);
            send(&o, pawn_state_frame(row))
        });
        let o = out_tx.clone();
        let zm = zone_map.clone();
        let h = rt.clone();
        p.db().entity_state().on_delete(move |_ctx, row| {
            relay_gone(&zm, &o, &h, row.entity_reference, row.macro_position_reference)
        });
        // The payload sidecar (human-pawns P0): live content changes + the slaved zone re-key
        // both land as updates. No delete relay — a pawn's departure already fans `StateGone`,
        // which drops the client-side join.
        let o = out_tx.clone();
        p.db().payload().on_insert(move |_ctx, row| send(&o, payload_frame(row)));
        let o = out_tx.clone();
        p.db().payload().on_update(move |_ctx, _old, row| send(&o, payload_frame(row)));
        // The needs sub-table (stat-model F2): a sip updates ONE row here and nothing else.
        let o = out_tx.clone();
        p.db().needs().on_insert(move |_ctx, row| send(&o, need_frame(row)));
        let o = out_tx.clone();
        p.db().needs().on_update(move |_ctx, _old, row| send(&o, need_frame(row)));
        // The inventory sub-table (inventory F2): pick_up inserts, drop deletes — a
        // deleted row relays as `item = 0` (slot emptied) because the pawn LIVES on;
        // there is no StateGone to drop the join (unlike needs, which only die with
        // their pawn).
        let o = out_tx.clone();
        p.db().inventory().on_insert(move |_ctx, row| send(&o, inventory_frame(row)));
        let o = out_tx.clone();
        p.db().inventory().on_update(move |_ctx, _old, row| send(&o, inventory_frame(row)));
        let o = out_tx.clone();
        p.db().inventory().on_delete(move |_ctx, row| {
            send(
                &o,
                ServerMsg::Inventory {
                    entity_reference: row.entity_reference,
                    zone: row.macro_position_reference,
                    slot: row.slot,
                    item: 0,
                    state: 0,
                },
            )
        });
    }

    if let Some(e) = &event {
        let o = out_tx.clone();
        e.db().event().on_insert(move |_ctx, row| {
            send(
                &o,
                ServerMsg::Event {
                    event_reference: row.event_reference,
                    zone: row.macro_position_reference,
                    tic: row.event_tic,
                    actions: row.actions.clone(),
                },
            )
        });
    }

    // Cold shards: a zone's ground + scatter. A cold row is a whole zone-layer, sent on insert/update.
    // The endpoint comes from the index router (`cold_shards`), resolved for region 0 — the bootstrap
    // while every visible zone is in region 0; falls back to the configured default if unrouted.
    // Multi-region → multi-shard routing (a per-region connection, resolved at subscribe time) is the
    // follow-up when a second cold shard is deployed.
    use resonantdust_codec::object::{TYPE_BIOME_THING, TYPE_BIOME_TILE};
    let (tile_url, tile_db) =
        pool.cold_endpoint(TYPE_BIOME_TILE, 0).unwrap_or_else(|| (pool.cfg.uri.clone(), pool.cfg.tile_db()));
    let tile = match connect_tile(&tile_url, &tile_db) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if let Some(t) = &tile {
        let o = out_tx.clone();
        t.db().entity_state().on_insert(move |_ctx, row| send(&o, cold_tile_frame(row)));
        let o = out_tx.clone();
        t.db().entity_state().on_update(move |_ctx, _old, row| send(&o, cold_tile_frame(row)));
        // The cold overlay: a whole-row override in the tile shard's `overlay` table, re-framed to the
        // per-cell `ColdState` wire (F10 — client-transparent). A row insert/update relays all its
        // cells; a row delete (a `fold`) clears them (the value now lives in the baseline).
        let o = out_tx.clone();
        t.db().overlay().on_insert(move |_ctx, row| relay_tile_overlay(&o, row, false));
        let o = out_tx.clone();
        t.db().overlay().on_update(move |_ctx, _old, row| relay_tile_overlay(&o, row, false));
        let o = out_tx.clone();
        t.db().overlay().on_delete(move |_ctx, row| relay_tile_overlay(&o, row, true));
    }
    let (thing_url, thing_db) =
        pool.cold_endpoint(TYPE_BIOME_THING, 0).unwrap_or_else(|| (pool.cfg.uri.clone(), pool.cfg.thing_db()));
    let thing = match connect_thing(&thing_url, &thing_db) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if let Some(t) = &thing {
        let o = out_tx.clone();
        t.db().entity_state().on_insert(move |_ctx, row| send(&o, cold_thing_frame(row)));
        let o = out_tx.clone();
        t.db().entity_state().on_update(move |_ctx, _old, row| send(&o, cold_thing_frame(row)));
        let o = out_tx.clone();
        t.db().overlay().on_insert(move |_ctx, row| relay_thing_overlay(&o, row, false));
        let o = out_tx.clone();
        t.db().overlay().on_update(move |_ctx, _old, row| relay_thing_overlay(&o, row, false));
        let o = out_tx.clone();
        t.db().overlay().on_delete(move |_ctx, row| relay_thing_overlay(&o, row, true));
    }

    World { event, data, pawn, tile, thing }
}

/// A `ColdTile` baseline frame — a cold `tile` shard's dense `entity_state` row, re-packed to the wire
/// `tiles: Vec<u16>` (the dense `items[i].kind_reference`, `tile_reference` = the index).
fn cold_tile_frame(row: &bindings::tile::EntityState) -> ServerMsg {
    ServerMsg::ColdTile {
        zone: row.macro_position_reference,
        subtype_id: row.subtype_id,
        layer_id: row.layer_id,
        tic: row.tic,
        tiles: row.items.iter().map(|it| it.kind_reference).collect(),
    }
}

/// A `ColdThing` baseline frame — a cold `thing` shard's sparse `entity_state` row, re-packed to the
/// wire `things: Vec<u32>` (`kind_pos_reference` = `kind:16 | tile:8 | data:8` per occupied cell).
fn cold_thing_frame(row: &bindings::thing::EntityState) -> ServerMsg {
    ServerMsg::ColdThing {
        zone: row.macro_position_reference,
        subtype_id: row.subtype_id,
        layer_id: row.layer_id,
        tic: row.tic,
        things: row
            .items
            .iter()
            .map(|it| {
                resonantdust_codec::object::pack_kind_pos_reference(it.kind_reference, it.tile_reference, it.data)
            })
            .collect(),
    }
}

/// Re-frame a tile `overlay` row's cells to per-cell `ColdState` (F10). Each cell gets a stable
/// pseudo `entity_reference` (`cold_entity_reference(server, position)`) so a re-issue updates rather
/// than duplicates; `removed` (a row delete / `kind == 0`) tells the client to drop the override and
/// read the baseline.
fn relay_tile_overlay(o: &mpsc::UnboundedSender<String>, row: &bindings::tile::Overlay, removed: bool) {
    use resonantdust_codec::object::{
        cold_entity_reference, pack_definition_reference, pack_layer_reference, pack_position_reference,
        pack_type_reference, TYPE_BIOME_TILE,
    };
    let server = resonantdust_codec::refs::pack_server_reference(TYPE_BIOME_TILE, 0);
    for it in &row.items {
        let layer_reference = pack_layer_reference(TYPE_BIOME_TILE, row.layer_id);
        let micro = ((it.tile_reference as u16) << 8) | layer_reference as u16;
        let position = pack_position_reference(row.macro_position_reference, micro);
        let definition =
            pack_definition_reference(pack_type_reference(TYPE_BIOME_TILE, row.subtype_id), it.kind_reference);
        send(
            o,
            ServerMsg::ColdState {
                zone: row.macro_position_reference,
                entity_reference: cold_entity_reference(server, position),
                position_reference: position,
                definition_reference: definition,
                data: 0,
                tic: row.tic,
                removed: removed || it.kind_reference == 0,
            },
        );
    }
}

/// Re-frame a thing `overlay` row's cells to per-cell `ColdState` (F10) — as [`relay_tile_overlay`] but
/// carrying the thing's `data` byte.
fn relay_thing_overlay(o: &mpsc::UnboundedSender<String>, row: &bindings::thing::Overlay, removed: bool) {
    tracing::info!(zone = row.macro_position_reference, cells = row.items.len(), removed,
        "relaying thing overlay row");
    use resonantdust_codec::object::{
        cold_entity_reference, pack_definition_reference, pack_layer_reference, pack_position_reference,
        pack_type_reference, TYPE_BIOME_THING,
    };
    let server = resonantdust_codec::refs::pack_server_reference(TYPE_BIOME_THING, 0);
    for it in &row.items {
        let layer_reference = pack_layer_reference(TYPE_BIOME_THING, row.layer_id);
        let micro = ((it.tile_reference as u16) << 8) | layer_reference as u16;
        let position = pack_position_reference(row.macro_position_reference, micro);
        let definition =
            pack_definition_reference(pack_type_reference(TYPE_BIOME_THING, row.subtype_id), it.kind_reference);
        send(
            o,
            ServerMsg::ColdState {
                zone: row.macro_position_reference,
                entity_reference: cold_entity_reference(server, position),
                position_reference: position,
                definition_reference: definition,
                data: it.data,
                tic: row.tic,
                // A kind-0 THING cell is a TOMBSTONE (a felled tree — lumberjack F5): it
                // must reach the client as a live override that SUPPRESSES the baseline
                // and draws nothing. `removed: true` is only for the override row itself
                // leaving the subscription (restore the baseline). Tiles differ: ground
                // can't be "nothing", so the tile relay's kind-0 = clear-override stands.
                removed,
            },
        );
    }
}

fn state_frame(row: &bindings::data_shard::EntityState) -> ServerMsg {
    ServerMsg::State {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        tic: row.tic,
        definition_reference: row.definition_reference,
        position_reference: resonantdust_codec::object::pack_position_reference(row.macro_position_reference, row.micro_position_reference),
        data: row.data,
    }
}

/// The pawn shard's `entity_state` → the same `State` wire frame (identical macro shape,
/// distinct bindings type).
fn pawn_state_frame(row: &bindings::pawn::EntityState) -> ServerMsg {
    ServerMsg::State {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        tic: row.tic,
        definition_reference: row.definition_reference,
        position_reference: resonantdust_codec::object::pack_position_reference(row.macro_position_reference, row.micro_position_reference),
        data: row.data,
    }
}

/// The pawn shard's `payload` sidecar row → the `Payload` wire frame (human-pawns P0) —
/// the entity's opcode stream, joined client-side to its `State` rows by `entity_reference`.
fn payload_frame(row: &bindings::pawn::Payload) -> ServerMsg {
    ServerMsg::Payload {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        tic: row.tic,
        payload: row.payload.clone(),
    }
}

/// One `inventory` sub-table row → the `Inventory` wire frame (inventory F2).
fn inventory_frame(row: &bindings::pawn::Inventory) -> ServerMsg {
    ServerMsg::Inventory {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        slot: row.slot,
        item: row.item,
        state: row.state,
    }
}

/// One `needs` sub-table row → the `Need` wire frame (stat-model F2).
fn need_frame(row: &bindings::pawn::Needs) -> ServerMsg {
    ServerMsg::Need {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        need: row.need,
        set_tic: row.set_tic,
    }
}

/// The verbs a CLIENT may queue (movement-hardening F2 — the first, deliberately tiny,
/// authorization seam: a verb-set check, NOT an ownership model). Everything else is
/// server-only: `MOVE_TO` is the chain SEED the worker's `move_to` interaction arm queues
/// (input-rework F3 — clients move via `EXECUTE_INTERACTION`), `MOVE_STEP` carries a
/// trip-serial that clients could stomp to steer pawns past validation, and `INIT_ZONE` is
/// worldgen (the edge's own path calls the reducer directly, not through this door).
const CLIENT_VERBS: &[u32] = &[
    resonantdust_codec::action::PROMOTE,
    resonantdust_codec::action::PROMOTE_EVENT,
    // CREATE left this list (spawn-authority F4): clients REQUEST spawns; the
    // worker's SPAWN_REQUEST arm is CREATE's only composer now.
    resonantdust_codec::action::SPAWN_REQUEST,
    resonantdust_codec::action::PLACE,
    resonantdust_codec::action::SET,
    resonantdust_codec::action::BUILD_WALL, // build-walls D5: the client-issued build order
    // needs-moodlets P2: open like MOVE_TO while no ownership model exists — the npc mints
    // thirst and the drills force-set it through this door. Tighten with ownership.
    resonantdust_codec::action::SET_NEED,
    resonantdust_codec::action::GRANT_CONDITION,
    // interactions P3: the client REQUESTS an interaction; the worker resolves + validates
    // it against the corpus and queues the real writes (F4). Same open-door posture as
    // SET_NEED until ownership lands (interactions I4).
    resonantdust_codec::action::EXECUTE_INTERACTION,
    // intent-queue-ui F3: a strip-circle click cancels by entry_id; resolves against
    // worker memory, writes nothing. QUEUE_STATE stays WORKER-ONLY (not listed).
    resonantdust_codec::action::CANCEL_INTENT,
    // inventory F3: INV_ADD / INV_REMOVE stay WORKER-ONLY (not listed) — items move
    // only through the worker's store/drop composers, which pair every mutation with
    // the free-count SET_NEED in one program.
];

/// Validate + relay a client intent to `event_shard.queue`. The door enforces "logged in",
/// "the program frames", and "client verbs only" here; ownership + rate limiting are future
/// (no ownership model yet). The reducer's own validation (parses, has an orchestrator) is
/// reported back via `queue_then`.
fn handle_queue(
    event: Option<&Arc<bindings::event_shard::DbConnection>>,
    out_tx: &mpsc::UnboundedSender<String>,
    session: Option<u32>,
    cid: u32,
    actions: Vec<u32>,
) {
    if session.is_none() {
        send(out_tx, ServerMsg::QueueErr { cid, error: "not logged in".to_string() });
        return;
    }
    for inst in resonantdust_codec::action::program(&actions) {
        match inst {
            Err(e) => {
                send(out_tx, ServerMsg::QueueErr { cid, error: format!("malformed program: {e:?}") });
                return;
            }
            Ok(i) if !CLIENT_VERBS.contains(&i.action) => {
                send(out_tx, ServerMsg::QueueErr { cid, error: format!("server-only verb: {}", i.action) });
                return;
            }
            Ok(_) => {}
        }
    }
    let Some(conn) = event else {
        send(out_tx, ServerMsg::QueueErr { cid, error: "event shard unavailable".to_string() });
        return;
    };
    let out = out_tx.clone();
    // npc-host I11: stamp the ISSUING player — the spawn-attribution link.
    let issuer = session.unwrap_or(0);
    let submit = conn.reducers().queue_then(actions, issuer, move |_ctx, res| match res {
        Ok(Ok(())) => send(&out, ServerMsg::QueueOk { cid }),
        Ok(Err(error)) => send(&out, ServerMsg::QueueErr { cid, error }),
        Err(e) => send(&out, ServerMsg::QueueErr { cid, error: format!("internal: {e}") }),
    });
    if let Err(e) = submit {
        send(out_tx, ServerMsg::QueueErr { cid, error: format!("submit failed: {e}") });
    }
}

/// Subscribe the client to a zone's `state` + `event` streams. Idempotent per zone. The initial
/// matching rows are delivered through the row callbacks as the subscription applies.
fn handle_subscribe(
    pool: &Arc<Pool>,
    world: &World,
    out_tx: &mpsc::UnboundedSender<String>,
    zones: &mut HashMap<u16, ZoneSub>,
    zone: u16,
) {
    if zones.contains_key(&zone) {
        return; // already streaming this zone
    }
    let (Some(data), Some(event)) = (&world.data, &world.event) else {
        send(out_tx, err_frame("world upstream unavailable"));
        return;
    };
    let state = data
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "state subscription error"))
        .subscribe([format!("SELECT * FROM entity_state WHERE macro_position_reference = {zone}")]);
    // The pawn shard's movers for the zone. A RESTING pawn's row arrives in this subscription's
    // initial snapshot and never updates again, so `on_insert` alone would miss it (the same
    // delivery guarantee as the cold baselines below) — replay the zone's rows on apply.
    let o = out_tx.clone();
    let pawn = world.pawn.as_ref().map(|p| {
        p.subscription_builder()
            .on_error(|_ctx, err| tracing::warn!(%err, "pawn subscription error"))
            .on_applied(move |ctx| {
                for row in ctx.db.entity_state().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, pawn_state_frame(&row));
                }
                // …and the payload sidecar — a resting pawn's payload arrives in this
                // subscription's snapshot and never updates again, the same delivery
                // guarantee as its state row above.
                for row in ctx.db.payload().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, payload_frame(&row));
                }
                // …and the needs sub-table rows (stat-model F2), same delivery guarantee.
                for row in ctx.db.needs().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, need_frame(&row));
                }
                // …and the inventory rows (inventory F2), same delivery guarantee.
                for row in ctx.db.inventory().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, inventory_frame(&row));
                }
            })
            .subscribe([
                format!("SELECT * FROM entity_state WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM payload WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM needs WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM inventory WHERE macro_position_reference = {zone}"),
            ])
    });
    // Settled events (the movement INTENT channel). Same delivery guarantee as the cold
    // baselines + the pawn sub: a row landing via this subscription's snapshot fires no
    // `on_insert`, and viewport-driven re-subscribes churn the per-zone handles — so replay the
    // zone's rows on apply (first-pawns I3). Clients dedup (`lastIntentTic`), so a replay of
    // history is harmless.
    let o = out_tx.clone();
    let event = event
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "event subscription error"))
        .on_applied(move |ctx| {
            for row in ctx.db.event().iter().filter(|r| r.macro_position_reference == zone) {
                send(
                    &o,
                    ServerMsg::Event {
                        event_reference: row.event_reference,
                        zone: row.macro_position_reference,
                        tic: row.event_tic,
                        actions: row.actions.clone(),
                    },
                );
            }
        })
        .subscribe([format!("SELECT * FROM event WHERE macro_position_reference = {zone}")]);
    // Cold ground + scatter + overlay for the zone (best-effort — a missing cold shard doesn't fail
    // the zone). One subscription per shard covers the baseline (dense/sparse `entity_state`) and its
    // sparse override (`overlay`), both zone-keyed.
    //
    // **`on_applied` snapshot relay (delivery guarantee).** The baseline is seeded (`seed_zone`) right
    // *after* this subscribe, and an `entity_state` row's `tic` never bumps again — so relying on the
    // `on_insert` callback alone races the seed: a row that lands in the SDK cache via this
    // subscription's initial snapshot (rather than a live insert) fires no `on_insert`, and with no
    // later change fires no `on_update` — the client would never receive it (black ground under
    // rendered scatter). So when the subscription **applies**, we explicitly push the zone's current
    // baseline rows. `on_insert`/`on_update` (in `build_world`) still carry live changes + late seeds;
    // a double relay is harmless (the client re-expands the row idempotently).
    let o = out_tx.clone();
    let seed_pool = pool.clone();
    let seed_tile = world.tile.clone();
    let seed_thing = world.thing.clone();
    let tile = world.tile.as_ref().map(|t| {
        t.subscription_builder()
            .on_error(|_ctx, err| tracing::warn!(%err, "tile cold subscription error"))
            .on_applied(move |ctx| {
                for row in ctx.db.entity_state().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, cold_tile_frame(&row));
                }
                // …and the OVERLAY. The subscription asks for both tables, but replaying only the
                // baseline means any override that PRE-DATES this subscription is silently never sent —
                // it is subscribed, so no later callback fires for it either. See torch-thing I4.
                for row in ctx.db.overlay().iter().filter(|r| r.macro_position_reference == zone) {
                    relay_tile_overlay(&o, &row, false);
                }
                // SEED ONLY A VIRGIN ZONE (lumberjack): the once-guard used to be edge
                // process memory, so every edge restart re-seeded touched zones — and a
                // re-seed's fresh row tic SHADOWS older overlay tombstones (felled trees
                // resurrected on the next reload). The applied snapshot is the truth: any
                // baseline row for the zone means it was seeded in some life; seed only
                // when there is none.
                if ctx.db.entity_state().iter().all(|r| r.macro_position_reference != zone) {
                    seed_zone(&seed_pool, seed_tile.as_ref(), seed_thing.as_ref(), zone);
                }
            })
            .subscribe([
                format!("SELECT * FROM entity_state WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM overlay WHERE macro_position_reference = {zone}"),
            ])
    });
    let o = out_tx.clone();
    let thing = world.thing.as_ref().map(|t| {
        t.subscription_builder()
            .on_error(|_ctx, err| tracing::warn!(%err, "thing cold subscription error"))
            .on_applied(move |ctx| {
                let (mut nb, mut no) = (0, 0);
                for row in ctx.db.entity_state().iter().filter(|r| r.macro_position_reference == zone) {
                    nb += 1;
                    send(&o, cold_thing_frame(&row));
                }
                for row in ctx.db.overlay().iter().filter(|r| r.macro_position_reference == zone) {
                    no += 1;
                    relay_thing_overlay(&o, &row, false);
                }
                tracing::info!(zone, baselines = nb, overlays = no, "thing zone replay applied");
            })
            .subscribe([
                format!("SELECT * FROM entity_state WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM overlay WHERE macro_position_reference = {zone}"),
            ])
    });
    zones.insert(zone, ZoneSub { _state: state, _pawn: pawn, _event: event, _tile: tile, _thing: thing });
    tracing::debug!(zone, "subscribed zone");
}

/// Generate + seed a zone's cold layers (ground + scatter) into the tile/thing shards, **once per
/// zone per edge process** (`claim_zone_seed`). Worldgen is deterministic and `seed` idempotent, so a
/// race or a second edge just re-writes the same content. A missing corpus or cold upstream is a
/// no-op — the zone simply has no terrain yet. The client's cold subscription then delivers the
/// seeded rows, which relay as `ColdTile`/`ColdThing`.
fn seed_zone(
    pool: &Arc<Pool>,
    tile: Option<&Arc<bindings::tile::DbConnection>>,
    thing: Option<&Arc<bindings::thing::DbConnection>>,
    zone: u16,
) {
    if !pool.claim_zone_seed(zone) {
        return; // already seeded this process (dedupe across this process's clients)
    }
    let Some(worldgen) = pool.current_worldgen() else { return };
    let (Some(tile), Some(thing)) = (tile, thing) else { return };
    // `zone` is the `macro_position_reference` worldgen keys on directly. One row per biome present;
    // `type_id` is the module, so `seed` takes `(macro, subtype_id, layer_id, payload)`. Worldgen
    // writes the primary layer (`layer_id = 0`).
    let layers = worldgen.zone_cold(zone);
    for (subtype_id, tiles) in layers.tiles {
        if let Err(err) = tile.reducers().seed(zone, subtype_id, 0, tiles) {
            tracing::warn!(%err, zone, subtype_id, "tile seed failed");
        }
    }
    let mut things = layers.things;
    append_init_objects(&worldgen, zone, &mut things);
    for (subtype_id, things) in things {
        if let Err(err) = thing.reducers().seed(zone, subtype_id, 0, things) {
            tracing::warn!(%err, zone, subtype_id, "thing seed failed");
        }
    }
    tracing::debug!(zone, "seeded zone cold layers");
}

/// World **initial conditions** — deliberate object placements, applied when their own zone is seeded.
///
/// Distinct from worldgen: these are not scattered by biome or probability, they are specific objects at
/// specific cells. That is the whole point — a torch at a known cell can be asserted about ("this tile is lit
/// by exactly one source"), where a `0.006` scatter can only be counted and eyeballed.
///
/// **Provisional home.** The user's call was "wherever we seed the initial conditions, we will move them
/// later", so this is deliberately a flat table plus a thin application: the fixture is one const and the
/// only logic is "resolve the kind, call the reducer". Moving it to a real initial-conditions pass should be
/// a cut-and-paste. See `docs/work/2026-07-26-torch-thing/` B-2.
fn append_init_objects(worldgen: &Worldgen, zone: u16, things: &mut Vec<(u16, Vec<u32>)>) {
    use resonantdust_codec::object::{pack_kind_pos_reference, pack_kind_reference};
    let mut seeded: Vec<u32> = Vec::new();
    for (init_zone, kind_name, cells) in INIT_OBJECTS {
        if zone != *init_zone {
            continue;
        }
        // The corpus is the ONLY authority for name→kind id. Resolved here, at the point of use, from the
        // live (hot-reloadable) bundle — never cached and never duplicated as a server-side constant,
        // because kind ids are append-ordered and a stale copy would silently place the wrong kind.
        let Some(kind_id) = worldgen.bundle().thing_object_id(kind_name) else {
            tracing::warn!(zone, kind_name, "init object kind not in corpus — skipping");
            continue;
        };
        // APPENDED to worldgen's payload rather than written through the overlay ([I4]). The overlay
        // relays as `ColdState` (the per-ENTITY frame) while the scatter relays as `ColdThing` (the
        // batched frame `onColdThings` consumes) — and only the latter builds a cold-thing prim and
        // attaches the kind's light. Appending keeps init objects on the proven path, and appending
        // (rather than replacing) is what stops them clobbering the zone's trees, which was F1's actual
        // concern about `seed`.
        //
        // `subtype_id = 0` is the reserved biome-agnostic subtype: an init object is not biome-derived,
        // so it gets its own row instead of claiming a biome's.
        // `kind_reference` is NOT the raw object id — it is `pack_kind_reference(kind_id, variant)`,
        // i.e. `kind_id << 4 | variant`, exactly as worldgen builds it. Passing the bare object id
        // decodes as kind_id 0 (an invalid kind) and the client silently drops the thing ([I5]).
        tracing::info!(zone, kind_name, count = cells.len(), "seeding init objects");
        seeded.extend(
            cells
                .iter()
                .map(|&t| pack_kind_pos_reference(pack_kind_reference(kind_id, 0), t, 0)),
        );
    }
    // ONE push for ALL init kinds. `seed` is REPLACE-by-(zone, subtype, layer), so pushing once per
    // INIT_OBJECTS row made the last row silently clobber the earlier ones — two warm torches vanished
    // the moment a blue one was added under the same subtype 0. The kind rides per CELL inside
    // `pack_kind_pos_reference`, so many kinds share one subtype-0 entry with no ambiguity; the subtype
    // is the STORAGE key, not the kind. Anything appending here must extend this vec, never push again.
    if !seeded.is_empty() {
        things.push((0, seeded));
    }
}

/// `(macro_position, kind name, in-zone tile_references)`.
///
/// **ONE light, deliberately** (user, 2026-08-02: "remove the current lights and use one light so we
/// can debug"). A single source is the only configuration where a lit pixel has exactly one possible
/// cause: any falloff, normal, or shadow artefact seen on screen belongs to *this* torch, with no
/// additive sum to disentangle it from. Debugging the lighting chain against overlapping pools means
/// guessing which contributor is wrong.
///
/// The torch sits at in-zone cell (4, 2) of zone (6, 3) — `macro_position 0x0063`, cell byte `x:4|y:4`
/// = `0x42` — which is global tile (100, 50), the default camera focus, so it is on screen the moment
/// the debug URL loads.
///
/// This previously seeded THREE (two `torch` at (4,2)/(12,4) plus one `torch_blue` at (8,10)) to
/// exercise additive accumulation and multi-caster shadowing at reach 16, where all three pools
/// overlap heavily. To restore that harder case, append the cells back — `torch_blue` must stay a
/// separate entry because `&thing.light.*` is authored per KIND and the prim light leaf carries no
/// per-instance colour. Note `seed` is REPLACE-by-(zone, subtype, layer), so anything added here must
/// extend the single `seeded` vec in `append_init_objects`, never push a second row.
const INIT_OBJECTS: &[(u16, &str, &[u8])] = &[(0x0063, "torch", &[0x42])];

/// Build the per-client players upstream and subscribe to the auth table so the
/// post-login row read is served from cache. `None` (with an error frame already
/// sent) if the upstream can't be reached.
async fn build_players(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
) -> Option<Arc<bindings::players::DbConnection>> {
    let (conn, ready) = match connect_players(&pool.cfg.uri, &pool.cfg.players_db()) {
        Some(c) => c,
        None => {
            send(out_tx, err_frame("players upstream unavailable"));
            return None;
        }
    };
    if !await_ready(ready).await {
        send(out_tx, err_frame("players upstream connect timed out"));
        return None;
    }
    let handle = conn
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "players subscription error"))
        // player_pawns rides the same subscription (player-pawns P2): the login funnel
        // reads the linkage to decide whether a mint is needed.
        .subscribe(["SELECT * FROM players", "SELECT * FROM player_pawns"]);
    // The handle must outlive this fn or the subscription tears down immediately.
    std::mem::forget(handle);
    Some(conn)
}

/// The OWNER fan (player-pawns P3, F6): relay the session's OWN player-pawn `needs` rows on
/// the existing `Need` frame (the ref's type nibble makes it self-describing — no new
/// protocol). Waits for the linkage (the mint may be in flight), subscribes the one
/// entity's rows, and lives until the session's outbound channel closes.
async fn fan_player_pawn(
    pool: Arc<Pool>,
    players: Arc<bindings::players::DbConnection>,
    player_id: u32,
    out_tx: mpsc::UnboundedSender<String>,
) {
    use bindings::players::player_pawns_table::PlayerPawnsTableAccess;
    use bindings::player_pawn::needs_table::NeedsTableAccess;

    let mut ppref = None;
    for _ in 0..PLAYER_READ_POLLS {
        if let Some(r) =
            players.db().player_pawns().iter().find(|r| r.player_id == player_id && r.active)
        {
            ppref = Some(r.player_pawn_reference);
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let Some(ppref) = ppref else { return }; // no linkage yet — the next login fans
    let Some((conn, ready)) =
        crate::connections::connect_player_pawn(&pool.cfg.uri, &pool.cfg.player_pawn_db())
    else {
        return;
    };
    if !await_ready(ready).await {
        return;
    }
    let o = out_tx.clone();
    conn.db().needs().on_insert(move |_ctx, row| send(&o, player_pawn_need_frame(row)));
    let o = out_tx.clone();
    conn.db().needs().on_update(move |_ctx, _old, row| send(&o, player_pawn_need_frame(row)));
    let handle = conn
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "player_pawn needs fan error"))
        .subscribe([format!("SELECT * FROM needs WHERE entity_reference = {ppref}")]);
    // Live until the session's outbound closes; dropping conn + handle tears down.
    while !out_tx.is_closed() {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    drop(handle);
}

/// The player's NAME from the auth cache (the F10 login-name resolution's input).
async fn read_player_name(
    players: &Arc<bindings::players::DbConnection>,
    player_id: u32,
) -> Option<String> {
    use bindings::players::players_table::PlayersTableAccess;
    for _ in 0..PLAYER_READ_POLLS {
        if let Some(p) = players.db().players().iter().find(|p| p.player_id == player_id) {
            return Some(p.name.clone());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    None
}

fn player_pawn_need_frame(row: &bindings::player_pawn::Needs) -> ServerMsg {
    ServerMsg::Need {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        need: row.need,
        set_tic: row.set_tic,
    }
}

/// Ensure the player owns a player-pawn (player-pawns P2, F2/I6) — the login funnel's second
/// half, spawned DETACHED after `LoginOk` so a healing shard can never block or fail a login:
/// if the linkage shows no row for this player, mint one on the `player_pawn` shard
/// (idempotent — the spawn ledger dedups on `(player_id, 0)`), read the minted reference off
/// `spawn_log`, and record the linkage (idempotent again; the first link becomes ACTIVE
/// inside the reducer — the one enforcement funnel, I4). Every failure path just logs:
/// the next login retries the whole funnel.
async fn ensure_player_pawn(
    pool: Arc<Pool>,
    players: Arc<bindings::players::DbConnection>,
    player_id: u32,
) {
    use bindings::players::link_player_pawn as _;
    use bindings::players::player_pawns_table::PlayerPawnsTableAccess;
    use bindings::player_pawn::spawn as _;
    use bindings::player_pawn::spawn_log_table::SpawnLogTableAccess;

    if players.db().player_pawns().iter().any(|r| r.player_id == player_id) {
        return; // already owns one — the common every-login case
    }
    let (conn, ready) = match crate::connections::connect_player_pawn(
        &pool.cfg.uri,
        &pool.cfg.player_pawn_db(),
    ) {
        Some(c) => c,
        None => {
            tracing::warn!(player_id, "player_pawn upstream unavailable — mint deferred to next login");
            return;
        }
    };
    if !await_ready(ready).await {
        tracing::warn!(player_id, "player_pawn connect timed out — mint deferred to next login");
        return;
    }
    let handle = conn
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "player_pawn spawn_log subscription error"))
        .subscribe([format!("SELECT * FROM spawn_log WHERE event_reference = {player_id}")]);
    // The definition + derived rows (P3, F5 + npc-host F10, THE LOGIN-NAME LAW): a brain
    // def whose name equals the player's name wins (the host logs each module-player in AS
    // its brain); everything else takes the default `player` thing def. Needs mint FULL at
    // max (the pawn mint law; the owner's bookkeeping writes the true counts). Player
    // traits are CONSTANT binds — derived by readers, never minted (F4). A corpus without
    // the def degrades to def 0 (row-carrier still works; rows arrive when the corpus does).
    let name = read_player_name(&players, player_id).await;
    let (def, need_rows) = pool
        .current_worldgen()
        .and_then(|wg| {
            let b = wg.bundle();
            let full_rows = |nrefs: Vec<u32>| -> Vec<u32> {
                nrefs
                    .into_iter()
                    .filter_map(|nref| {
                        let np = b.need_params_by_ref(nref)?;
                        let q = resonantdust_codec::value::quantize(
                            np.max as f32,
                            np.min as f32,
                            np.max as f32,
                        );
                        Some(resonantdust_codec::object::pack_gameplay_row(nref, q))
                    })
                    .collect()
            };
            if let Some(brain_def) = name.as_deref().and_then(|n| b.brain_definition_reference(n)) {
                let brain_id = b.brain_object_id(name.as_deref().unwrap())?;
                return Some((brain_def, full_rows(b.brain_needs(brain_id))));
            }
            let def = b.definition_reference(false, "player")?;
            let kind = resonantdust_codec::object::def_kind_id(def);
            Some((def, full_rows(b.thing_needs(kind))))
        })
        .unwrap_or((0, vec![]));
    if def == 0 {
        tracing::warn!(player_id, "no `player` def in the corpus — minting def-0 (rows follow the corpus)");
    }
    // The mint: the ledger key is (player_id, 0) — the login funnel's dedup, not a world
    // event. Position 0 and promote=true so the live row exists (the worker's ghost guard
    // requires it).
    if let Err(err) = conn.reducers.spawn(0, 0, player_id, 0, def, 0, vec![], need_rows, 0, true) {
        tracing::warn!(player_id, %err, "player_pawn spawn submit failed — mint deferred");
        return; // dropping `handle` + `conn` tears the subscription down
    }
    // Read the minted reference off the ledger (the reducer returns no value): poll the
    // subscribed cache the same way the login read polls `players`.
    let mut minted = None;
    for _ in 0..PLAYER_READ_POLLS {
        if let Some(r) = conn.db().spawn_log().iter().find(|r| r.event_reference == player_id) {
            minted = Some(r.entity_reference);
            break;
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    let Some(player_pawn_reference) = minted else {
        tracing::warn!(player_id, "player-pawn mint not visible — link deferred to next login");
        return;
    };
    // Await the reducer OUTCOME (the `_then` form, claim_or_login's shape) on a connection
    // THIS TASK OWNS: the session's players conn is explicitly `disconnect()`ed at session
    // teardown, so a login-and-quit races the detached submit to death — seen live (the
    // linkage never landed while the direct CLI call worked). A dedicated conn's lifecycle
    // ends at this fn's end, after the outcome arrives.
    let (own_players, ready) = match crate::connections::connect_players(&pool.cfg.uri, &pool.cfg.players_db()) {
        Some(c) => c,
        None => {
            tracing::warn!(player_id, "players upstream unavailable for the link — deferred");
            return;
        }
    };
    if !await_ready(ready).await {
        tracing::warn!(player_id, "players connect timed out for the link — deferred");
        return;
    }
    let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
    let done_tx = Mutex::new(Some(done_tx));
    let submit = own_players.reducers.link_player_pawn_then(player_id, player_pawn_reference, move |_ctx, res| {
        let outcome = res.unwrap_or_else(|e| Err(format!("internal: {e}")));
        if let Some(tx) = done_tx.lock().unwrap().take() {
            let _ = tx.send(outcome);
        }
    });
    if let Err(err) = submit {
        tracing::warn!(player_id, %err, "link_player_pawn submit failed — link deferred");
        return;
    }
    match tokio::time::timeout(REDUCER_CALL_TIMEOUT, done_rx).await {
        Ok(Ok(Ok(()))) => tracing::info!(
            player_id,
            player_pawn = format!("{player_pawn_reference:#010x}"),
            "player-pawn minted + linked (player-pawns P2)"
        ),
        Ok(Ok(Err(err))) => tracing::warn!(player_id, %err, "link_player_pawn refused — link deferred"),
        _ => tracing::warn!(player_id, "link_player_pawn outcome not seen — link deferred to next login"),
    }
    drop(handle); // drop = unsubscribe; the connection follows at fn end
}

/// Relay `claim_or_login` to the players DB, then read back the resulting
/// `player_id` + `player_shard_reference` and bind the session.
async fn handle_login(
    players: Option<&Arc<bindings::players::DbConnection>>,
    out_tx: &mpsc::UnboundedSender<String>,
    session: &mut Option<u32>,
    cid: u32,
    client_time_ms: u64,
    name: String,
) {
    let Some(conn) = players else {
        send(
            out_tx,
            ServerMsg::LoginErr {
                cid,
                error: "players upstream unavailable".to_string(),
                server_micros: server_micros(),
            },
        );
        return;
    };

    let (done_tx, done_rx) = oneshot::channel::<Result<(), String>>();
    let submit = conn.reducers.claim_or_login_then(
        client_time_ms,
        name.clone(),
        move |_ctx, res| {
            let outcome = res.unwrap_or_else(|e| Err(format!("internal: {e}")));
            let _ = done_tx.send(outcome);
        },
    );
    if let Err(err) = submit {
        send(
            out_tx,
            ServerMsg::LoginErr {
                cid,
                error: format!("login submit failed: {err}"),
                server_micros: server_micros(),
            },
        );
        return;
    }

    let committed = tokio::time::timeout(REDUCER_CALL_TIMEOUT, done_rx).await;
    match committed {
        Ok(Ok(Ok(()))) => {} // reducer succeeded; read the row below
        Ok(Ok(Err(error))) => {
            send(out_tx, ServerMsg::LoginErr { cid, error, server_micros: server_micros() });
            return;
        }
        _ => {
            send(
                out_tx,
                ServerMsg::LoginErr {
                    cid,
                    error: "login timed out".to_string(),
                    server_micros: server_micros(),
                },
            );
            return;
        }
    }

    match read_player_by_name(conn, &name).await {
        Some((player_id, player_shard_reference)) => {
            *session = Some(player_id);
            tracing::debug!(player_id, %name, "session established");
            send(
                out_tx,
                ServerMsg::LoginOk { cid, player_id, player_shard_reference, server_micros: server_micros() },
            );
        }
        None => {
            tracing::warn!(%name, "login ok but player row not seen");
            send(
                out_tx,
                ServerMsg::LoginErr {
                    cid,
                    error: "login committed but player row not visible".to_string(),
                    server_micros: server_micros(),
                },
            );
        }
    }
}

/// Poll the players cache for the row named `name`, returning its
/// `(player_id, player_shard_reference)`. The reducer's row insert arrives asynchronously after
/// commit, so we retry over a short window.
///
/// One row per name — `Player.name` is unique, and the table is no longer versioned.
/// This used to scan every row and take the max by `valid_at`'s time half, picking the
/// newest of a player's version rows; there are no version rows to pick between now.
async fn read_player_by_name(
    conn: &Arc<bindings::players::DbConnection>,
    name: &str,
) -> Option<(u32, u16)> {
    use bindings::players::players_table::PlayersTableAccess;
    for _ in 0..PLAYER_READ_POLLS {
        if let Some(p) = conn.db().players().iter().find(|p| p.name == name) {
            return Some((p.player_id, p.player_shard_reference));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    None
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Serialize a [`ServerMsg`] and enqueue it. A closed channel (client gone) is a no-op.
fn send(out: &mpsc::UnboundedSender<String>, msg: ServerMsg) {
    let text = msg.to_text();
    // TEMP (logs-drop drill): trace cold-state delivery end to end.
    if text.contains("coldState") || text.contains("cold_state") || text.contains("ColdState") {
        tracing::info!(frame = %text.chars().take(140).collect::<String>(), ok = !out.is_closed(),
            "sending ColdState frame");
    }
    let _ = out.send(text);
}

/// Shorthand for a protocol-level error frame.
fn err_frame(msg: &str) -> ServerMsg {
    ServerMsg::Error { error: msg.to_string() }
}
