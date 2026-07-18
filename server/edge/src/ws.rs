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
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use spacetimedb_sdk::{DbContext, Table as _, TableWithPrimaryKey as _};
use tokio::sync::{mpsc, oneshot};

use crate::bindings;
use crate::bindings::data_shard::state_table::StateTableAccess as _;
use crate::bindings::event_shard::event_table::EventTableAccess as _;
use crate::bindings::thing::cold_thing_table::ColdThingTableAccess as _;
use crate::bindings::thing::seed as _; // reducer trait → thing.reducers().seed
use crate::bindings::thing::state_table::StateTableAccess as _; // cold overlay `state`
use crate::bindings::tile::cold_tile_table::ColdTileTableAccess as _;
use crate::bindings::tile::seed as _; // reducer trait → tile.reducers().seed
use crate::bindings::tile::state_table::StateTableAccess as _; // cold overlay `state`
use crate::bindings::event_shard::queue as _; // reducer trait → `reducers().queue_then`
use crate::bindings::players::claim_or_login as _; // reducer trait → `reducers.claim_or_login_then`
use crate::connections::{
    await_ready, connect_data_shard, connect_event_shard, connect_players, connect_thing,
    connect_tile, Pool,
};
use crate::protocol::{ClientMsg, ServerMsg};

/// The per-client world upstreams: the two sim shards. `event_shard` takes the client's `queue`
/// intents and streams settled `event` rows; `data_shard` streams composed `state` rows. Each client
/// gets its own pair so their zone subscriptions don't collide (the set-semantics hazard).
struct World {
    event: Option<Arc<bindings::event_shard::DbConnection>>,
    data: Option<Arc<bindings::data_shard::DbConnection>>,
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
            }
            ClientMsg::Queue { cid, actions } => {
                handle_queue(world.event.as_ref(), &out_tx, session, cid, actions);
            }
            ClientMsg::SubscribeZone { zone } => {
                handle_subscribe(&world, &out_tx, &mut zones, zone);
                seed_zone(&pool, &world, zone); // generate + seed the zone's cold layers on first sight
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

    if let Some(d) = &data {
        let o = out_tx.clone();
        d.db().state().on_insert(move |_ctx, row| send(&o, state_frame(row)));
        let o = out_tx.clone();
        d.db().state().on_update(move |_ctx, _old, row| send(&o, state_frame(row)));
        let o = out_tx.clone();
        d.db().state().on_delete(move |_ctx, row| {
            send(
                &o,
                ServerMsg::StateGone {
                    entity_reference: row.entity_reference,
                    zone: row.macro_position_reference,
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
        t.db().cold_tile().on_insert(move |_ctx, row| send(&o, cold_tile_frame(row)));
        let o = out_tx.clone();
        t.db().cold_tile().on_update(move |_ctx, _old, row| send(&o, cold_tile_frame(row)));
        // The cold overlay: a per-cell mutation in the tile shard's `state` table.
        let s = |o: &mpsc::UnboundedSender<String>, row: &bindings::tile::State, removed: bool| {
            send(o, cold_state_frame(row.macro_position_reference, row.entity_reference, row.position_reference, row.definition_reference, row.data, row.tic, removed))
        };
        let o = out_tx.clone();
        t.db().state().on_insert(move |_ctx, row| s(&o, row, false));
        let o = out_tx.clone();
        t.db().state().on_update(move |_ctx, _old, row| s(&o, row, false));
        let o = out_tx.clone();
        t.db().state().on_delete(move |_ctx, row| s(&o, row, true));
    }
    let (thing_url, thing_db) =
        pool.cold_endpoint(TYPE_BIOME_THING, 0).unwrap_or_else(|| (pool.cfg.uri.clone(), pool.cfg.thing_db()));
    let thing = match connect_thing(&thing_url, &thing_db) {
        Some((conn, ready)) => await_ready(ready).await.then_some(conn),
        None => None,
    };
    if let Some(t) = &thing {
        let o = out_tx.clone();
        t.db().cold_thing().on_insert(move |_ctx, row| send(&o, cold_thing_frame(row)));
        let o = out_tx.clone();
        t.db().cold_thing().on_update(move |_ctx, _old, row| send(&o, cold_thing_frame(row)));
        // The cold overlay: a per-cell mutation in the thing shard's `state` table.
        let s = |o: &mpsc::UnboundedSender<String>, row: &bindings::thing::State, removed: bool| {
            send(o, cold_state_frame(row.macro_position_reference, row.entity_reference, row.position_reference, row.definition_reference, row.data, row.tic, removed))
        };
        let o = out_tx.clone();
        t.db().state().on_insert(move |_ctx, row| s(&o, row, false));
        let o = out_tx.clone();
        t.db().state().on_update(move |_ctx, _old, row| s(&o, row, false));
        let o = out_tx.clone();
        t.db().state().on_delete(move |_ctx, row| s(&o, row, true));
    }

    World { event, data, tile, thing }
}

/// A `ColdTile` baseline frame — a cold `tile` shard's `cold_tile` row.
fn cold_tile_frame(row: &bindings::tile::ColdTile) -> ServerMsg {
    ServerMsg::ColdTile {
        zone: row.macro_position_reference,
        subtype_id: row.subtype_id,
        layer_id: row.layer_id,
        tic: row.tic,
        tiles: row.tiles.clone(),
    }
}

/// A `ColdThing` baseline frame — a cold `thing` shard's `cold_thing` row.
fn cold_thing_frame(row: &bindings::thing::ColdThing) -> ServerMsg {
    ServerMsg::ColdThing {
        zone: row.macro_position_reference,
        subtype_id: row.subtype_id,
        layer_id: row.layer_id,
        tic: row.tic,
        things: row.things.clone(),
    }
}

/// A `ColdState` overlay frame — a cold shard's `state` row, echoed to the client to composite over
/// the baseline cell at `position_reference`. `removed` marks the override clearing (a `state` delete).
fn cold_state_frame(
    zone: u16,
    entity_reference: u32,
    position_reference: u32,
    definition_reference: u32,
    data: u8,
    tic: u16,
    removed: bool,
) -> ServerMsg {
    ServerMsg::ColdState { zone, entity_reference, position_reference, definition_reference, data, tic, removed }
}

fn state_frame(row: &bindings::data_shard::State) -> ServerMsg {
    ServerMsg::State {
        entity_reference: row.entity_reference,
        zone: row.macro_position_reference,
        tic: row.tic,
        definition_reference: row.definition_reference,
        position_reference: row.position_reference,
        data: row.data,
    }
}

/// Validate + relay a client intent to `event_shard.queue`. The door enforces "logged in" and "the
/// program frames" here; ownership + rate limiting are future (no ownership model yet). The reducer's
/// own validation (parses, has an orchestrator) is reported back via `queue_then`.
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
        if let Err(e) = inst {
            send(out_tx, ServerMsg::QueueErr { cid, error: format!("malformed program: {e:?}") });
            return;
        }
    }
    let Some(conn) = event else {
        send(out_tx, ServerMsg::QueueErr { cid, error: "event shard unavailable".to_string() });
        return;
    };
    let out = out_tx.clone();
    let submit = conn.reducers().queue_then(actions, move |_ctx, res| match res {
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
        .subscribe([format!("SELECT * FROM state WHERE macro_position_reference = {zone}")]);
    let event = event
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "event subscription error"))
        .subscribe([format!("SELECT * FROM event WHERE macro_position_reference = {zone}")]);
    // Cold ground + scatter + overlay for the zone (best-effort — a missing cold shard doesn't fail
    // the zone). One subscription per shard covers the baseline (`cold_tile`/`cold_thing`) and its
    // hot-format overlay (`state`), both zone-keyed.
    //
    // **`on_applied` snapshot relay (delivery guarantee).** The baseline is seeded (`seed_zone`) right
    // *after* this subscribe, and a `cold_tile`/`cold_thing` row's `tic` never bumps again — so relying
    // on the `on_insert` callback alone races the seed: a row that lands in the SDK cache via this
    // subscription's initial snapshot (rather than a live insert) fires no `on_insert`, and with no
    // later change fires no `on_update` — the client would never receive it (black ground under
    // rendered scatter). So when the subscription **applies**, we explicitly push the zone's current
    // rows. `on_insert`/`on_update` (in `build_world`) still carry live changes + late seeds; a double
    // relay is harmless (the client re-expands the row idempotently).
    let o = out_tx.clone();
    let tile = world.tile.as_ref().map(|t| {
        t.subscription_builder()
            .on_error(|_ctx, err| tracing::warn!(%err, "cold_tile subscription error"))
            .on_applied(move |ctx| {
                for row in ctx.db.cold_tile().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, cold_tile_frame(&row));
                }
            })
            .subscribe([
                format!("SELECT * FROM cold_tile WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM state WHERE macro_position_reference = {zone}"),
            ])
    });
    let o = out_tx.clone();
    let thing = world.thing.as_ref().map(|t| {
        t.subscription_builder()
            .on_error(|_ctx, err| tracing::warn!(%err, "cold_thing subscription error"))
            .on_applied(move |ctx| {
                for row in ctx.db.cold_thing().iter().filter(|r| r.macro_position_reference == zone) {
                    send(&o, cold_thing_frame(&row));
                }
            })
            .subscribe([
                format!("SELECT * FROM cold_thing WHERE macro_position_reference = {zone}"),
                format!("SELECT * FROM state WHERE macro_position_reference = {zone}"),
            ])
    });
    zones.insert(zone, ZoneSub { _state: state, _event: event, _tile: tile, _thing: thing });
    tracing::debug!(zone, "subscribed zone");
}

/// Generate + seed a zone's cold layers (ground + scatter) into the tile/thing shards, **once per
/// zone per edge process** (`claim_zone_seed`). Worldgen is deterministic and `seed` idempotent, so a
/// race or a second edge just re-writes the same content. A missing corpus or cold upstream is a
/// no-op — the zone simply has no terrain yet. The client's cold subscription then delivers the
/// seeded rows, which relay as `ColdTile`/`ColdThing`.
fn seed_zone(pool: &Arc<Pool>, world: &World, zone: u16) {
    if !pool.claim_zone_seed(zone) {
        return; // already seeded this process
    }
    let Some(worldgen) = pool.current_worldgen() else { return };
    let (Some(tile), Some(thing)) = (&world.tile, &world.thing) else { return };
    // `zone` is the `macro_position_reference` worldgen keys on directly. One row per biome present;
    // `type_id` is the module, so `seed` takes `(macro, subtype_id, layer_id, payload)`. Worldgen
    // writes the primary layer (`layer_id = 0`).
    let layers = worldgen.zone_cold(zone);
    for (subtype_id, tiles) in layers.tiles {
        if let Err(err) = tile.reducers().seed(zone, subtype_id, 0, tiles) {
            tracing::warn!(%err, zone, subtype_id, "tile seed failed");
        }
    }
    for (subtype_id, things) in layers.things {
        if let Err(err) = thing.reducers().seed(zone, subtype_id, 0, things) {
            tracing::warn!(%err, zone, subtype_id, "thing seed failed");
        }
    }
    tracing::debug!(zone, "seeded zone cold layers");
}

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
        .subscribe(["SELECT * FROM players"]);
    // The handle must outlive this fn or the subscription tears down immediately.
    std::mem::forget(handle);
    Some(conn)
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
    let _ = out.send(msg.to_text());
}

/// Shorthand for a protocol-level error frame.
fn err_frame(msg: &str) -> ServerMsg {
    ServerMsg::Error { error: msg.to_string() }
}
