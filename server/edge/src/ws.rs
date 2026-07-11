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
use crate::bindings::zone::seed_entity as _; // reducer trait → `reducers.seed_entity` (terrain seed)
use crate::connections::{await_ready, connect_players, connect_shard, connect_zone, Pool};
use crate::index::{resolve_zone_or_default, ShardEndpoint};
use crate::protocol::{ClientMsg, RowData, RowOp, ServerMsg, StateRow, ZoneTerrainRow};

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

/// A live zone-terrain upstream (the `zone` module). One per client; its `state` relay
/// is wired once and carries every subscribed zone's terrain row. There is a single
/// terrain DB today, so a client holds at most one.
struct TerrainConn {
    conn: Arc<bindings::zone::DbConnection>,
    wired: bool,
}

/// A live zone subscription — the zone's object `state` rows from its shard, plus the
/// parallel terrain `state` row from the `zone` module. Unsub tears down both.
struct ZoneSub {
    #[allow(dead_code)]
    endpoint: ShardEndpoint,
    handle: bindings::shard::SubscriptionHandle,
    terrain_handle: bindings::zone::SubscriptionHandle,
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
    // The single per-client terrain upstream (the `zone` module), lazily connected on the
    // first zone subscribe and shared across every zone this client watches.
    let mut terrain: Option<TerrainConn> = None;
    let mut subs: HashMap<u32, ZoneSub> = HashMap::new();

    while let Some(cmsg) = cmd_rx.recv().await {
        match cmsg {
            ClientMsg::Login { cid, client_time_ms, name } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
            }
            ClientMsg::SubZone { sid, zone_id } => {
                handle_sub_zone(&pool, &out_tx, &mut shards, &mut terrain, &mut subs, sid, zone_id)
                    .await;
            }
            ClientMsg::Unsub { sid } => {
                if let Some(zs) = subs.remove(&sid) {
                    let _ = zs.handle.unsubscribe();
                    let _ = zs.terrain_handle.unsubscribe();
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
        let _ = zs.terrain_handle.unsubscribe();
    }
    for (_endpoint, sc) in shards.drain() {
        let _ = sc.conn.disconnect();
    }
    if let Some(tc) = &terrain {
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
    terrain: &mut Option<TerrainConn>,
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

    // ── terrain: the zone's static tiles/things from the `zone` module (its own DB) ──
    // A second per-zone subscription, on a separate upstream, alongside the object one.
    let terrain_db = pool.cfg.default_terrain_db();
    if terrain.is_none() {
        let (conn, ready) = match connect_zone(&pool.cfg.uri, &terrain_db) {
            Some(c) => c,
            None => {
                send(out_tx, err_frame(&format!("terrain {terrain_db} unavailable")));
                let _ = handle.unsubscribe();
                return;
            }
        };
        if !await_ready(ready).await {
            send(out_tx, err_frame(&format!("terrain {terrain_db} connect timed out")));
            let _ = handle.unsubscribe();
            return;
        }
        *terrain = Some(TerrainConn { conn, wired: false });
    }
    let tconn = terrain.as_mut().expect("just inserted");
    if !tconn.wired {
        wire_terrain_relay(&tconn.conn, out_tx.clone());
        tconn.wired = true;
    }

    // Seed this zone's terrain from worldgen once the terrain subscription's cache is
    // populated (so a never-generated zone is distinguishable from a settled one). The
    // seeding insert relays back on the same subscription. Read the LIVE worldgen at
    // apply time so a content hot-reload between subscribe and apply is picked up.
    let seed_conn = tconn.conn.clone();
    let seed_pool = pool.clone();
    let terrain_error = out_tx.clone();
    let terrain_handle = tconn
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            let wg = seed_pool.current_worldgen();
            seed_terrain_if_empty(&seed_conn, wg.as_deref(), zone_id);
        })
        .on_error(move |_ctx, err| {
            send(&terrain_error, err_frame(&format!("terrain sub {sid}: {err}")))
        })
        .subscribe([format!("SELECT * FROM state WHERE zone_id = {zone_id}")]);

    subs.insert(sid, ZoneSub { endpoint, handle, terrain_handle });
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

/// Install insert/update/delete callbacks on the terrain (`zone`) `state` table; each
/// fired row relays as a [`RowData::ZoneTerrain`] frame (demux-by-zone, `sid: 0`, like
/// the object relay). Wired once per client's terrain upstream.
fn wire_terrain_relay(
    conn: &Arc<bindings::zone::DbConnection>,
    out: mpsc::UnboundedSender<String>,
) {
    use bindings::zone::state_table::StateTableAccess;

    let t = conn.db().state();
    let (si, su, sd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&si, RowOp::Insert, RowData::ZoneTerrain(terrain_row(r))));
    t.on_update(move |_c, _o, r| relay(&su, RowOp::Update, RowData::ZoneTerrain(terrain_row(r))));
    t.on_delete(move |_c, r| relay(&sd, RowOp::Delete, RowData::ZoneTerrain(terrain_row(r))));
}

/// Copy a generated terrain `State` row into the serde wire mirror.
fn terrain_row(r: &bindings::zone::state_type::State) -> ZoneTerrainRow {
    ZoneTerrainRow {
        zone_id: r.zone_id,
        tiles: r.tiles.clone(),
        things: r.things.clone(),
    }
}

/// Seed a zone's terrain from worldgen if it's never been generated. Called once the
/// terrain subscription's cache is populated: an existing terrain `state` row (matched by
/// the zone's positional key) means the zone is already stored — leave it, so a later
/// mutation is never clobbered. A missing one means a fresh zone, so worldgen classifies
/// its biomes into packed tiles + things and `seed_entity` stores it; the insert relays
/// back to every subscriber. No-op when worldgen is disabled (content failed to load).
fn seed_terrain_if_empty(
    conn: &Arc<bindings::zone::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    zone_id: u32,
) {
    use bindings::zone::state_table::StateTableAccess;
    use resonantdust_codec::refs::{pack_positional_entity, ENTITY_TYPE_ZONE_TERRAIN};

    let Some(wg) = worldgen else { return };
    let key = pack_positional_entity(ENTITY_TYPE_ZONE_TERRAIN, zone_id, 0, 0);
    // The terrain cache holds only this connection's subscribed zones' rows; match by key.
    if conn.db().state().iter().any(|s| s.entity_key == key) {
        return;
    }
    let (tiles, things) = wg.zone_terrain(zone_id);
    match conn.reducers.seed_entity(key, zone_id, tiles, things) {
        Ok(()) => tracing::info!(zone_id, "seeded fresh zone terrain"),
        Err(err) => tracing::warn!(zone_id, %err, "seed_entity (terrain) request failed"),
    }
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
