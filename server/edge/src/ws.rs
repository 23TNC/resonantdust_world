//! Per-client WebSocket session — the inbound half of the server.
//!
//! One [`client_session`] task per connected client. It owns:
//!   * a **players** upstream (for login), subscribed to the auth table so the
//!     post-`claim_or_login` row read hits a warm cache;
//!   * a lazily-grown map of **shard** upstreams, one per distinct
//!     [`ShardEndpoint`] a subscribed zone resolves to;
//!   * the gate-owned session: WS → `player_id`, set at login.
//!
//! Pipeline-only: a zone subscription streams the shard's `state` rows (the tick
//! pipeline's client-visible entities); the client renders from those. Client intent
//! (`Move`) becomes an `append_event` on the shard. The old cold/hot terrain,
//! `free_things`, and the release/transfer saga were removed with the shard merge.
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
use spacetimedb_sdk::{DbContext, SubscriptionHandle as _, Table as _, TableWithPrimaryKey as _};
use tokio::sync::{mpsc, oneshot};

use crate::bindings;
use crate::bindings::players::claim_or_login as _; // reducer trait → `reducers.claim_or_login_then`
use crate::bindings::shard::append_event as _; // reducer trait → `reducers.append_event`
use crate::bindings::cold_things::seed_entity as _; // reducer trait → things `reducers.seed_entity`
use crate::bindings::cold_tiles::seed_entity as _; // reducer trait → cold_tiles `reducers.seed_entity`
use crate::bindings::cold_tiles::seed_cold_row as _; // reducer trait → cold_tiles `reducers.seed_cold_row`
use crate::connections::{
    await_ready, connect_players, connect_shard, connect_cold_things, connect_cold_tiles, Pool,
};
use crate::index::{resolve_zone_or_default, ShardEndpoint};
use crate::protocol::{
    ClientMsg, ColdObjectsRow, RowData, RowOp, ServerMsg, StateRow, ZoneThingsRow, ZoneTilesRow,
};

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

/// A live shard upstream (the tick pipeline). One per distinct endpoint this client
/// touches; its `state` relay is wired once and carries every subscribed zone's rows
/// (the client demuxes by `zone_id`).
struct ShardConn {
    conn: Arc<bindings::shard::DbConnection>,
    wired: bool,
}

/// The per-client cold-**tiles** upstream (the `cold_tiles` module — dense `Vec<u8>` grid).
/// Its `state` relay is wired once; one tiles DB today, so a client holds at most one.
struct TilesConn {
    conn: Arc<bindings::cold_tiles::DbConnection>,
    wired: bool,
}

/// The per-client cold-**things** upstream (the `cold_things` module — sparse `Vec<u64>`).
/// One cold-things DB today.
struct ColdThingsConn {
    conn: Arc<bindings::cold_things::DbConnection>,
    wired: bool,
}

/// A live zone subscription — the zone's object `state` rows from its shard, plus the
/// parallel tiles + things rows from their modules. Unsub tears down all three.
struct ZoneSub {
    #[allow(dead_code)]
    endpoint: ShardEndpoint,
    handle: bindings::shard::SubscriptionHandle,
    tiles_handle: bindings::cold_tiles::SubscriptionHandle,
    cold_things_handle: bindings::cold_things::SubscriptionHandle,
    /// The zone's cold-OBJECT rows (`cold_tiles::cold` — the object model), alongside the
    /// legacy tiles/things handles while the client migrates.
    cold_handle: bindings::cold_tiles::SubscriptionHandle,
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

    // Stateful commands run on a dedicated worker task so the read loop never blocks on a
    // slow DB handler; the read loop answers pings inline and forwards everything else
    // over an ordered channel (per-connection command order is preserved).
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ClientMsg>();
    let worker = tokio::spawn(session_worker(pool.clone(), players, out_tx.clone(), cmd_rx));

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

/// Drains stateful client commands in arrival order, owning all per-session DB state
/// (session id, shard upstreams, zone subs). Runs teardown once the channel closes.
async fn session_worker(
    pool: Arc<Pool>,
    players: Option<Arc<bindings::players::DbConnection>>,
    out_tx: mpsc::UnboundedSender<String>,
    mut cmd_rx: mpsc::UnboundedReceiver<ClientMsg>,
) {
    let mut session: Option<u32> = None;
    let mut shards: HashMap<ShardEndpoint, ShardConn> = HashMap::new();
    // The per-client tiles + cold-things upstreams (the `cold_tiles` / `cold_things` modules),
    // lazily connected on the first zone subscribe and shared across every zone watched.
    let mut tiles: Option<TilesConn> = None;
    let mut cold_things: Option<ColdThingsConn> = None;
    let mut subs: HashMap<u32, ZoneSub> = HashMap::new();

    while let Some(cmsg) = cmd_rx.recv().await {
        match cmsg {
            ClientMsg::Login { cid, client_time_ms, name } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
            }
            ClientMsg::SubZone { sid, zone_id } => {
                handle_sub_zone(
                    &pool, &out_tx, &mut shards, &mut tiles, &mut cold_things, &mut subs, sid,
                    zone_id,
                )
                .await;
            }
            ClientMsg::Unsub { sid } => {
                if let Some(zs) = subs.remove(&sid) {
                    let _ = zs.handle.unsubscribe();
                    let _ = zs.tiles_handle.unsubscribe();
                    let _ = zs.cold_things_handle.unsubscribe();
                    let _ = zs.cold_handle.unsubscribe();
                } else {
                    tracing::debug!(sid, "unsub for unknown sid");
                }
            }
            ClientMsg::Move { tile_x, tile_y } => {
                handle_move(&pool, &out_tx, &shards, session, tile_x, tile_y).await;
            }
            // Pings are answered on the read loop and never forwarded here.
            ClientMsg::Ping { .. } => {}
        }
    }

    // Teardown: drop zone subscriptions, then disconnect every upstream.
    for (_sid, zs) in subs.drain() {
        let _ = zs.handle.unsubscribe();
        let _ = zs.tiles_handle.unsubscribe();
        let _ = zs.cold_things_handle.unsubscribe();
        let _ = zs.cold_handle.unsubscribe();
    }
    for (_endpoint, sc) in shards.drain() {
        let _ = sc.conn.disconnect();
    }
    if let Some(tc) = &tiles {
        let _ = tc.conn.disconnect();
    }
    if let Some(tc) = &cold_things {
        let _ = tc.conn.disconnect();
    }
    if let Some(conn) = &players {
        let _ = conn.disconnect();
    }
    tracing::debug!(player = ?session, "client session worker ended");
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
/// `player_id` + `data_shard` and bind the session.
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
        Some((player_id, data_shard)) => {
            *session = Some(player_id);
            tracing::debug!(player_id, %name, "session established");
            send(
                out_tx,
                ServerMsg::LoginOk { cid, player_id, data_shard, server_micros: server_micros() },
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

/// Poll the players cache for the latest row named `name`, returning its
/// `(player_id, data_shard)`. The reducer's row insert arrives asynchronously after
/// commit, so we retry over a short window.
async fn read_player_by_name(
    conn: &Arc<bindings::players::DbConnection>,
    name: &str,
) -> Option<(u32, u16)> {
    use bindings::players::players_table::PlayersTableAccess;
    for _ in 0..PLAYER_READ_POLLS {
        let latest = conn
            .db()
            .players()
            .iter()
            .filter(|p| p.name == name)
            .max_by_key(|p| p.valid_at >> 16);
        if let Some(p) = latest {
            return Some((p.player_id, p.data_shard));
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    None
}

/// Resolve a zone to its shard, connect if needed, wire its `state` relay once, and
/// open the zone's `state` subscription. Terrain/things now flow as pipeline `state`
/// rows; the cold-zone tier lands in a later phase.
async fn handle_sub_zone(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &mut HashMap<ShardEndpoint, ShardConn>,
    tiles: &mut Option<TilesConn>,
    cold_things: &mut Option<ColdThingsConn>,
    subs: &mut HashMap<u32, ZoneSub>,
    sid: u32,
    zone_id: u32,
) {
    if subs.contains_key(&sid) {
        send(out_tx, err_frame(&format!("sid {sid} already in use")));
        return;
    }

    let endpoint = match resolve_zone_or_default(&pool.index, &pool.cfg, zone_id) {
        Ok(e) => e,
        Err(err) => {
            send(out_tx, err_frame(&format!("route zone {zone_id}: {err}")));
            return;
        }
    };

    if !shards.contains_key(&endpoint) {
        let (conn, ready) = match connect_shard(&endpoint.url, &endpoint.db_name) {
            Some(c) => c,
            None => {
                send(out_tx, err_frame(&format!("shard {} unavailable", endpoint.db_name)));
                return;
            }
        };
        if !await_ready(ready).await {
            send(out_tx, err_frame(&format!("shard {} connect timed out", endpoint.db_name)));
            return;
        }
        shards.insert(endpoint.clone(), ShardConn { conn, wired: false });
    }
    let shard = shards.get_mut(&endpoint).expect("just inserted");

    if !shard.wired {
        wire_state_relay(&shard.conn, out_tx.clone());
        shard.wired = true;
    }

    let applied_tx = out_tx.clone();
    let error_tx = out_tx.clone();
    let handle = shard
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| send(&applied_tx, ServerMsg::Applied { sid }))
        .on_error(move |_ctx, err| send(&error_tx, err_frame(&format!("sub {sid}: {err}"))))
        .subscribe([format!("SELECT * FROM state WHERE zone_id = {zone_id}")]);

    // ── tiles: the zone's dense Vec<u8> grid from the `cold_tiles` module (its own DB) ──
    // Seeded + relayed like the object sub; read the LIVE worldgen at apply time so a
    // content hot-reload between subscribe and apply is picked up.
    let tiles_db = pool.cfg.default_cold_tiles_db();
    if tiles.is_none() {
        let (conn, ready) = match connect_cold_tiles(&pool.cfg.uri, &tiles_db) {
            Some(c) => c,
            None => {
                send(out_tx, err_frame(&format!("tiles {tiles_db} unavailable")));
                let _ = handle.unsubscribe();
                return;
            }
        };
        if !await_ready(ready).await {
            send(out_tx, err_frame(&format!("tiles {tiles_db} connect timed out")));
            let _ = handle.unsubscribe();
            return;
        }
        *tiles = Some(TilesConn { conn, wired: false });
    }
    let tconn = tiles.as_mut().expect("just inserted");
    if !tconn.wired {
        wire_tiles_relay(&tconn.conn, out_tx.clone());
        // The object-model `cold` table lives in the SAME cold_tiles module/DB, so wire
        // its relay on this one connection too.
        wire_cold_relay(&tconn.conn, out_tx.clone());
        tconn.wired = true;
    }
    let seed_conn = tconn.conn.clone();
    let seed_pool = pool.clone();
    let tiles_error = out_tx.clone();
    let tiles_handle = tconn
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            let wg = seed_pool.current_worldgen();
            seed_tiles_if_empty(&seed_conn, wg.as_deref(), seed_pool.cfg.server_id, zone_id);
        })
        .on_error(move |_ctx, err| send(&tiles_error, err_frame(&format!("tiles sub {sid}: {err}"))))
        .subscribe([format!("SELECT * FROM state WHERE zone_id = {zone_id}")]);

    // ── cold objects: the zone's ColdRows from the cold_tiles module's `cold` table
    // (object model — biome-tile ground + biome-thing scatter, biome in the subtype).
    // Same connection as tiles; seeded from `zone_cold_objects` on apply. Additive
    // alongside the legacy tiles/things subs while the client migrates.
    let cold_seed_conn = tconn.conn.clone();
    let cold_seed_pool = pool.clone();
    let cold_error = out_tx.clone();
    let cold_handle = tconn
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            let wg = cold_seed_pool.current_worldgen();
            seed_cold_if_empty(&cold_seed_conn, wg.as_deref(), zone_id);
        })
        .on_error(move |_ctx, err| send(&cold_error, err_frame(&format!("cold sub {sid}: {err}"))))
        .subscribe([format!("SELECT * FROM cold WHERE zone_id = {zone_id}")]);

    // ── cold things: the zone's sparse Vec<u32> list from the `cold_things` module ──
    let cold_things_db = pool.cfg.default_cold_things_db();
    if cold_things.is_none() {
        let (conn, ready) = match connect_cold_things(&pool.cfg.uri, &cold_things_db) {
            Some(c) => c,
            None => {
                send(out_tx, err_frame(&format!("cold-things {cold_things_db} unavailable")));
                let _ = handle.unsubscribe();
                let _ = tiles_handle.unsubscribe();
                return;
            }
        };
        if !await_ready(ready).await {
            send(out_tx, err_frame(&format!("cold-things {cold_things_db} connect timed out")));
            let _ = handle.unsubscribe();
            let _ = tiles_handle.unsubscribe();
            return;
        }
        *cold_things = Some(ColdThingsConn { conn, wired: false });
    }
    let thconn = cold_things.as_mut().expect("just inserted");
    if !thconn.wired {
        wire_cold_things_relay(&thconn.conn, out_tx.clone());
        thconn.wired = true;
    }
    let seed_conn = thconn.conn.clone();
    let seed_pool = pool.clone();
    let cold_things_error = out_tx.clone();
    let cold_things_handle = thconn
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            let wg = seed_pool.current_worldgen();
            seed_cold_things_if_empty(&seed_conn, wg.as_deref(), seed_pool.cfg.server_id, zone_id);
        })
        .on_error(move |_ctx, err| {
            send(&cold_things_error, err_frame(&format!("things sub {sid}: {err}")))
        })
        .subscribe([format!("SELECT * FROM state WHERE zone_id = {zone_id}")]);

    subs.insert(sid, ZoneSub { endpoint, handle, tiles_handle, cold_things_handle, cold_handle });
}

/// Turn a client move intent into an `ACTION_MOVE` event on the shard's tick pipeline,
/// targeting **this player's own object** (`ENTITY_TYPE_PLAYER` + `player_id`). The object
/// materializes on the first move (work-gen carries a default base, the move sets
/// position), so no login-time seed is needed. Thin intent check: must be logged in.
async fn handle_move(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    player_id: Option<u32>,
    tile_x: i32,
    tile_y: i32,
) {
    use resonantdust_codec::packed::cell;
    use resonantdust_codec::refs::{pack_minted_entity, ENTITY_TYPE_PLAYER, SERVER_REF_NONE};

    let Some(player_id) = player_id else {
        send(out_tx, err_frame("move: not logged in"));
        return;
    };
    // Keep the player object inside zone (0,0) so it stays on the client's subscribed
    // grid — wrap the clicked world tile into the 16×16 zone. (A real move would honor
    // the tile's actual zone; cross-zone movement is a later phase.)
    let dest_zone = 0u32;
    let location = cell(tile_x.rem_euclid(16) as u8, tile_y.rem_euclid(16) as u8);
    let endpoint = match resolve_zone_or_default(&pool.index, &pool.cfg, dest_zone) {
        Ok(e) => e,
        Err(err) => {
            send(out_tx, err_frame(&format!("move: route zone {dest_zone}: {err}")));
            return;
        }
    };
    let Some(shard) = shards.get(&endpoint) else {
        send(out_tx, err_frame("move: shard not connected (subscribe the zone first)"));
        return;
    };
    // The player's own entity; both actor and target (a self-move). Not shard-minted:
    // SERVER_REF_NONE as the minting server + the (globally-unique) player_id as the id.
    let key = pack_minted_entity(ENTITY_TYPE_PLAYER, player_id, SERVER_REF_NONE);
    let data = resonantdust_tick::pack_move(dest_zone, location, 0, 0);
    if let Err(err) = shard.conn.reducers.append_event(
        pool.cfg.server_id, // source_server_reference
        0,                  // actor_server_reference: same shard (self-move)
        0,                  // requesting_server_reference: externally injected
        0,                  // trigger_server_reference: no trigger
        0,                  // trigger_event_reference
        key,
        key,
        resonantdust_tick::ACTION_MOVE,
        data[0],
        data[1],
    ) {
        send(out_tx, err_frame(&format!("move: request failed: {err}")));
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Install insert/update/delete callbacks on a shard's `state` table. Every fired row
/// is forwarded as a [`ServerMsg::Row`] (with the `sid: 0` demux-by-zone sentinel).
fn wire_state_relay(conn: &Arc<bindings::shard::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::shard::state_table::StateTableAccess;

    let t = conn.db().state();
    let (si, su, sd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&si, RowOp::Insert, RowData::State(state_row(r))));
    t.on_update(move |_c, _o, r| relay(&su, RowOp::Update, RowData::State(state_row(r))));
    t.on_delete(move |_c, r| relay(&sd, RowOp::Delete, RowData::State(state_row(r))));
}

/// Copy a generated `State` row into the serde wire mirror.
fn state_row(r: &bindings::shard::state_type::State) -> StateRow {
    StateRow {
        entity_key: r.entity_key,
        tic: r.tic,
        kind: r.kind,
        zone_id: r.zone_id,
        location: r.location,
        rotation: r.rotation,
        offset: r.offset,
        data_0: r.data_0,
        data_1: r.data_1,
    }
}

/// Install insert/update/delete callbacks on the tiles (`cold_tiles`) `state` table; each fired
/// row relays as a [`RowData::ZoneTiles`] frame (demux-by-zone, `sid: 0`).
fn wire_tiles_relay(conn: &Arc<bindings::cold_tiles::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::cold_tiles::state_table::StateTableAccess;

    let t = conn.db().state();
    let (si, su, sd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&si, RowOp::Insert, RowData::ZoneTiles(tiles_row(r))));
    t.on_update(move |_c, _o, r| relay(&su, RowOp::Update, RowData::ZoneTiles(tiles_row(r))));
    t.on_delete(move |_c, r| relay(&sd, RowOp::Delete, RowData::ZoneTiles(tiles_row(r))));
}

/// Copy a generated tiles `State` row into the serde wire mirror.
fn tiles_row(r: &bindings::cold_tiles::state_type::State) -> ZoneTilesRow {
    ZoneTilesRow { zone_id: r.zone_id, tiles: r.tiles.clone() }
}

/// Install insert/update/delete callbacks on the things (`things`) `state` table; each
/// fired row relays as a [`RowData::ZoneThings`] frame (demux-by-zone, `sid: 0`).
fn wire_cold_things_relay(
    conn: &Arc<bindings::cold_things::DbConnection>,
    out: mpsc::UnboundedSender<String>,
) {
    use bindings::cold_things::state_table::StateTableAccess;

    let t = conn.db().state();
    let (si, su, sd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&si, RowOp::Insert, RowData::ZoneThings(cold_things_row(r))));
    t.on_update(move |_c, _o, r| relay(&su, RowOp::Update, RowData::ZoneThings(cold_things_row(r))));
    t.on_delete(move |_c, r| relay(&sd, RowOp::Delete, RowData::ZoneThings(cold_things_row(r))));
}

/// Copy a generated things `State` row into the serde wire mirror.
fn cold_things_row(r: &bindings::cold_things::state_type::State) -> ZoneThingsRow {
    ZoneThingsRow { zone_id: r.zone_id, things: r.things.clone() }
}

/// Seed a zone's tiles from worldgen if never generated — the whole-zone tile-grid entity
/// keyed by its `zone_reference` (`server_id` = this edge, the minting server). See
/// [`seed_cold_things_if_empty`] for the shared rationale (existing row = settled, leave it;
/// missing = fresh, generate + store).
fn seed_tiles_if_empty(
    conn: &Arc<bindings::cold_tiles::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    server_id: u16,
    zone_id: u32,
) {
    use bindings::cold_tiles::state_table::StateTableAccess;
    use resonantdust_codec::refs::pack_zone_reference;

    let Some(wg) = worldgen else { return };
    let key = pack_zone_reference(server_id, zone_id);
    if conn.db().state().iter().any(|s| s.entity_key == key) {
        return;
    }
    let (tiles, _things) = wg.zone_terrain(zone_id);
    match conn.reducers.seed_entity(key, zone_id, tiles) {
        Ok(()) => tracing::info!(zone_id, "seeded fresh zone tiles"),
        Err(err) => tracing::warn!(zone_id, %err, "seed_entity (tiles) request failed"),
    }
}

/// Seed a zone's cold things from worldgen if it's never been generated. Called once the
/// things subscription's cache is populated: an existing row (matched by the zone's
/// `zone_reference`) means the zone is already stored — leave it, so a later mutation is
/// never clobbered. A missing one means a fresh zone, so worldgen scatters its things and
/// `seed_entity` stores them; the insert relays back. The tiles seed is the mirror. No-op
/// when worldgen is disabled (content failed to load).
fn seed_cold_things_if_empty(
    conn: &Arc<bindings::cold_things::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    server_id: u16,
    zone_id: u32,
) {
    use bindings::cold_things::state_table::StateTableAccess;
    use resonantdust_codec::refs::pack_zone_reference;

    let Some(wg) = worldgen else { return };
    let key = pack_zone_reference(server_id, zone_id);
    if conn.db().state().iter().any(|s| s.entity_key == key) {
        return;
    }
    let (_tiles, things) = wg.zone_terrain(zone_id);
    match conn.reducers.seed_entity(key, zone_id, things) {
        Ok(()) => tracing::info!(zone_id, "seeded fresh zone cold things"),
        Err(err) => tracing::warn!(zone_id, %err, "seed_entity (cold things) request failed"),
    }
}

/// Install insert/update/delete callbacks on the `cold_tiles` module's `cold` table (the
/// object model); each fired row relays as a [`RowData::ColdObjects`] frame
/// (demux-by-zone, `sid: 0`). The object-model counterpart to [`wire_tiles_relay`] +
/// [`wire_cold_things_relay`].
fn wire_cold_relay(conn: &Arc<bindings::cold_tiles::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::cold_tiles::cold_table::ColdTableAccess;

    let t = conn.db().cold();
    let (ci, cu, cd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&ci, RowOp::Insert, RowData::ColdObjects(cold_row(r))));
    t.on_update(move |_c, _o, r| relay(&cu, RowOp::Update, RowData::ColdObjects(cold_row(r))));
    t.on_delete(move |_c, r| relay(&cd, RowOp::Delete, RowData::ColdObjects(cold_row(r))));
}

/// Copy a `cold` row into the serde wire mirror.
fn cold_row(r: &bindings::cold_tiles::cold_type::Cold) -> ColdObjectsRow {
    ColdObjectsRow { zone_id: r.zone_id, type_reference: r.type_reference, kinds: r.kinds.clone() }
}

/// Seed a zone's cold OBJECTS (biome-tile ground + biome-thing scatter) from worldgen if
/// the zone has no `cold` rows yet — the object-model path (`docs/object-model.md`).
/// Each `ColdRow` of [`Worldgen::zone_cold_objects`] becomes one `cold` row via
/// `seed_cold_row` (insert-if-absent per `(zone, type_reference)`, so a worldgen re-run
/// never clobbers a mutated row). No-op when worldgen is disabled.
fn seed_cold_if_empty(
    conn: &Arc<bindings::cold_tiles::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    zone_id: u32,
) {
    use bindings::cold_tiles::cold_table::ColdTableAccess;

    let Some(wg) = worldgen else { return };
    if conn.db().cold().iter().any(|c| c.zone_id == zone_id) {
        return;
    }
    let rows = wg.zone_cold_objects(zone_id);
    let n = rows.len();
    for row in rows {
        if let Err(err) = conn.reducers.seed_cold_row(zone_id, row.type_reference, row.kinds) {
            tracing::warn!(zone_id, %err, "seed_cold_row request failed");
        }
    }
    tracing::info!(zone_id, rows = n, "seeded fresh zone cold objects");
}

fn relay(out: &mpsc::UnboundedSender<String>, op: RowOp, row: RowData) {
    send(out, ServerMsg::Row { sid: 0, op, row });
}

/// Serialize a [`ServerMsg`] and enqueue it. A closed channel (client gone) is a no-op.
fn send(out: &mpsc::UnboundedSender<String>, msg: ServerMsg) {
    let _ = out.send(msg.to_text());
}

/// Shorthand for a protocol-level error frame.
fn err_frame(msg: &str) -> ServerMsg {
    ServerMsg::Error { error: msg.to_string() }
}
