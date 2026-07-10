//! Per-client WebSocket session — the inbound half of the server.
//!
//! One [`client_session`] task per connected client. It owns:
//!   * a **players** upstream (for login), subscribed to the auth table so the
//!     post-`claim_or_login` row read hits a warm cache;
//!   * a lazily-grown map of **shard** upstreams, one per distinct
//!     [`ShardEndpoint`] a subscribed zone resolves to — regions on different
//!     SpacetimeDB databases each get their own connection;
//!   * the gate-owned session: WS → `player_id`, set at login.
//!
//! Outbound frames come from two places — the request/response handlers on this
//! async task, and the SDK row callbacks on the upstreams' threads — so they're
//! funneled through one unbounded channel to a single writer task that owns the
//! WS sink.

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
use crate::bindings::zone_shard::seed_cold_zone as _; // reducer trait → `reducers.seed_cold_zone`
use crate::bindings::zone_shard::begin_release as _; // reducer trait → `reducers.begin_release`
use crate::bindings::zone_shard::ack_release as _; // reducer trait → `reducers.ack_release`
use crate::bindings::object_shard::receive as _; // reducer trait → `reducers.receive`
use crate::bindings::object_shard::move_debug_mover as _; // reducer trait → `reducers.move_debug_mover`
use crate::bindings::object_shard::append_event as _; // reducer trait → `reducers.append_event`
use crate::connections::{await_ready, connect_object_shard, connect_players, connect_shard, Pool};
use crate::index::{resolve_object_or_default, resolve_zone_or_default, ShardEndpoint};
use crate::protocol::{
    ClientMsg, ColdZoneRow, FreeThingRow, HotCellRow, RowData, RowOp, ServerMsg, StateRow,
};

/// How long to wait for `claim_or_login` to commit before failing the login.
const REDUCER_CALL_TIMEOUT: Duration = Duration::from_secs(5);
/// Post-login player-row read budget: poll the players cache this many times,
/// `POLL_INTERVAL` apart, for the row the reducer just wrote to land.
const PLAYER_READ_POLLS: u32 = 20;
const POLL_INTERVAL: Duration = Duration::from_millis(100);

// Transfer `direction` / `state` discriminants, mirrored from the shard modules'
// `transfer.rs` (the gate compares the raw u8s on relayed rows; the modules own
// the canonical constants). A pending outbound zone transfer triggers the object
// `receive`; the object's received receipt triggers the zone `ack_release`.
const TRANSFER_DIR_OUT: u8 = 0;
const TRANSFER_DIR_IN: u8 = 1;
const TRANSFER_ST_PENDING: u8 = 0;
const TRANSFER_ST_RECEIVED: u8 = 1;

/// `GET /ws` — upgrade to a WebSocket and run a [`client_session`].
pub async fn handler(ws: WebSocketUpgrade, State(pool): State<Arc<Pool>>) -> Response {
    ws.on_upgrade(move |socket| client_session(socket, pool))
}

/// A live zone-shard upstream plus whether its row callbacks are installed. One
/// per distinct endpoint this client touches.
struct ShardConn {
    conn: Arc<bindings::zone_shard::DbConnection>,
    /// Row callbacks (cold + 2 hot tables) are wired once per connection — they
    /// relay every subscribed zone's rows, demuxed client-side by `zone_id`.
    wired: bool,
}

/// A live object-shard upstream (loose things). Mirror of [`ShardConn`] for the
/// second shard class; its `free_things` relay is likewise wired once.
struct ObjectConn {
    conn: Arc<bindings::object_shard::DbConnection>,
    wired: bool,
}

/// A live zone subscription. Terrain + affixed things come from the `zone_shard`
/// (`handle`); loose things come from the `object_shard` (`object_handle`, absent
/// if that shard was unreachable — loose things are best-effort, terrain is not).
/// `endpoint` is the zone shard this used (teardown recorded for symmetry).
struct ZoneSub {
    #[allow(dead_code)]
    endpoint: ShardEndpoint,
    handle: bindings::zone_shard::SubscriptionHandle,
    object_handle: Option<bindings::object_shard::SubscriptionHandle>,
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
    //     row callbacks on upstream threads. Can be a firehose (worldgen snapshots,
    //     a streaming mover) and the frames can be large.
    //   * `pong_tx` — a priority lane for clock-sync Pongs only. The writer drains
    //     it *ahead* of `out_tx`, so a Pong never departs behind a backlog of row
    //     frames. Without this, a row-flooded connection (whoever's driving the
    //     sim) sees its Pong queued out, inflating the client's RTT and biasing its
    //     offset — the server-side half of keeping pings off the busy path.
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

    // Players upstream, built eagerly: login is the first thing most clients do,
    // and the row read needs the subscription warm. A failure here leaves
    // `players = None`; login then reports the outage instead of hanging.
    let players = build_players(&pool, &out_tx).await;

    // Stateful commands run on a dedicated worker task so the read loop never
    // blocks on a slow DB handler (login / sub_zone / worldgen can take seconds).
    // That head-of-line blocking used to delay — and, because the ping reply is
    // stamped when the loop *reaches* it, systematically bias — the clock-sync
    // Pong. Now the read loop only reads: it answers pings inline and forwards
    // everything else to the worker over an ordered channel (per-connection
    // command order is preserved).
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<ClientMsg>();
    let worker = tokio::spawn(session_worker(
        pool.clone(),
        players,
        out_tx.clone(),
        cmd_rx,
    ));

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
            // Ping/Pong are handled by axum; binary isn't part of the protocol.
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
            // Clock-sync probe: stamped and answered right here on the read loop
            // (so a slow worker handler can't delay it) and sent on the priority
            // lane (so a backlog of row frames can't delay it either). No DB work —
            // this task stamps its own wall clock (same box / NTP-synced to the
            // SpacetimeDB host that stamps `valid_at`).
            ClientMsg::Ping { client_send_ms } => {
                send(
                    &pong_tx,
                    ServerMsg::Pong {
                        client_send_ms,
                        server_ms: now_ms(),
                    },
                );
            }
            // Everything stateful is handled in arrival order on the worker.
            other => {
                if cmd_tx.send(other).is_err() {
                    break; // worker gone — nothing left to serve
                }
            }
        }
    }

    // Closing the command channel drains the worker, which owns session teardown.
    // The writer is aborted only after the worker returns (its senders are
    // scattered across the SDK callbacks the worker tears down).
    drop(cmd_tx);
    let _ = worker.await;
    writer.abort();
    tracing::debug!("client session ended");
}

/// Drains stateful client commands in arrival order, owning all per-session DB
/// state (session id, shard/object upstreams, zone subs, the transfer bridge
/// flag). Split out from the read loop so clock-sync pings are never queued behind
/// a slow handler here. Runs teardown once the command channel closes.
async fn session_worker(
    pool: Arc<Pool>,
    players: Option<Arc<bindings::players::DbConnection>>,
    out_tx: mpsc::UnboundedSender<String>,
    mut cmd_rx: mpsc::UnboundedReceiver<ClientMsg>,
) {
    let mut session: Option<u32> = None;
    let mut shards: HashMap<ShardEndpoint, ShardConn> = HashMap::new();
    let mut object_shards: HashMap<ShardEndpoint, ObjectConn> = HashMap::new();
    let mut subs: HashMap<u32, ZoneSub> = HashMap::new();
    // Whether the release saga's cross-shard `transfers` bridge is installed
    // (once per session, on the first `Release`; single zone/object pair in dev).
    let mut transfer_bridge_wired = false;

    while let Some(cmsg) = cmd_rx.recv().await {
        match cmsg {
            ClientMsg::Login {
                cid,
                client_time_ms,
                name,
            } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
            }
            ClientMsg::SubZone { sid, zone_id } => {
                handle_sub_zone(
                    &pool,
                    &out_tx,
                    &mut shards,
                    &mut object_shards,
                    &mut subs,
                    sid,
                    zone_id,
                )
                .await;
            }
            ClientMsg::Unsub { sid } => {
                if let Some(zs) = subs.remove(&sid) {
                    let _ = zs.handle.unsubscribe();
                    if let Some(h) = zs.object_handle {
                        let _ = h.unsubscribe();
                    }
                } else {
                    tracing::debug!(sid, "unsub for unknown sid");
                }
            }
            ClientMsg::Release { zone_id, location } => {
                handle_release(
                    &pool,
                    &out_tx,
                    &shards,
                    &object_shards,
                    &mut transfer_bridge_wired,
                    zone_id,
                    location,
                )
                .await;
            }
            ClientMsg::Move { tile_x, tile_y } => {
                handle_move(&pool, &out_tx, &object_shards, session, tile_x, tile_y).await;
            }
            // Pings are answered on the read loop and never forwarded here.
            ClientMsg::Ping { .. } => {}
        }
    }

    // Teardown: drop zone subscriptions, then disconnect every upstream.
    for (_sid, zs) in subs.drain() {
        let _ = zs.handle.unsubscribe();
        if let Some(h) = zs.object_handle {
            let _ = h.unsubscribe();
        }
    }
    for (_endpoint, sc) in shards.drain() {
        let _ = sc.conn.disconnect();
    }
    for (_endpoint, oc) in object_shards.drain() {
        let _ = oc.conn.disconnect();
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
    // Subscribe to the whole (low-write) auth table; login reads the latest row
    // for a name straight from this cache. Narrowing to a per-name query would
    // need SQL-escaping the user-supplied name — deferred; the table is small.
    let handle = conn
        .subscription_builder()
        .on_error(|_ctx, err| tracing::warn!(%err, "players subscription error"))
        .subscribe(["SELECT * FROM players"]);
    // The handle must outlive this fn or the subscription tears down immediately.
    // Leak it for the connection's lifetime — it's dropped when the process exits
    // and the connection with it.
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

    // Submit the reducer; a oneshot carries its committed result back here.
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
            send(
                out_tx,
                ServerMsg::LoginErr {
                    cid,
                    error,
                    server_micros: server_micros(),
                },
            );
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
                ServerMsg::LoginOk {
                    cid,
                    player_id,
                    data_shard,
                    server_micros: server_micros(),
                },
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
/// `(player_id, data_shard)`. The reducer's row insert arrives asynchronously
/// after commit, so we retry over a short window.
async fn read_player_by_name(
    conn: &Arc<bindings::players::DbConnection>,
    name: &str,
) -> Option<(u32, u16)> {
    use bindings::players::players_table::PlayersTableAccess;
    for _ in 0..PLAYER_READ_POLLS {
        // Latest version row for the name = max time component of `valid_at`.
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

/// Resolve a zone to its shard, connect to that shard if needed, wire its row
/// relay, and open the four-table subscription for the zone.
async fn handle_sub_zone(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &mut HashMap<ShardEndpoint, ShardConn>,
    object_shards: &mut HashMap<ShardEndpoint, ObjectConn>,
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

    // Get-or-connect the shard upstream for this endpoint.
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

    // Wire the row relay once per connection — it carries every zone subscribed
    // on this shard; the client demuxes by `zone_id`.
    if !shard.wired {
        wire_shard_relay(&shard.conn, out_tx.clone());
        shard.wired = true;
    }

    // One subscription handle covers the zone's three tables. `zone_id` is a u32,
    // safe to interpolate.
    let q = |table: &str| format!("SELECT * FROM {table} WHERE zone_id = {zone_id}");
    let applied_tx = out_tx.clone();
    let error_tx = out_tx.clone();
    // Captured into `on_applied` to seed the zone if it's never been settled: once
    // the cold cache is populated we can tell whether a row exists, and worldgen
    // generates the terrain for a fresh one.
    let seed_conn = shard.conn.clone();
    // Capture the pool (not a worldgen snapshot) so seeding reads the LIVE
    // worldgen when the subscription applies — a content hot-reload between
    // subscribe and apply then generates this zone from the new content.
    let seed_pool = pool.clone();
    let handle = shard
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            send(&applied_tx, ServerMsg::Applied { sid });
            let wg = seed_pool.current_worldgen();
            seed_zone_if_empty(&seed_conn, wg.as_deref(), zone_id);
        })
        .on_error(move |_ctx, err| send(&error_tx, err_frame(&format!("sub {sid}: {err}"))))
        .subscribe([
            q("cold_zones"),
            q("hot_tiles"),
            q("hot_things"),
        ]);

    // Also open the zone's loose-thing feed from the object shard. Best-effort:
    // if the object shard is unreachable the zone still renders its terrain, just
    // without free things (`object_handle` = None).
    let object_handle = open_object_sub(pool, out_tx, object_shards, sid, zone_id).await;

    subs.insert(sid, ZoneSub { endpoint, handle, object_handle });
}

/// Get-or-connect the object shard holding `zone_id`'s region, wire its
/// `free_things` relay once, and open the zone's loose-thing subscription.
/// Returns `None` (having logged) when the object shard is unreachable — loose
/// things are best-effort, so the caller keeps the zone subscription regardless.
async fn open_object_sub(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    object_shards: &mut HashMap<ShardEndpoint, ObjectConn>,
    sid: u32,
    zone_id: u32,
) -> Option<bindings::object_shard::SubscriptionHandle> {
    let endpoint = resolve_object_or_default(zone_id, &pool.cfg);

    if !object_shards.contains_key(&endpoint) {
        let (conn, ready) = match connect_object_shard(&endpoint.url, &endpoint.db_name) {
            Some(c) => c,
            None => {
                tracing::warn!(db = %endpoint.db_name, "object shard unavailable; zone served without loose things");
                return None;
            }
        };
        if !await_ready(ready).await {
            tracing::warn!(db = %endpoint.db_name, "object shard connect timed out; zone served without loose things");
            return None;
        }
        object_shards.insert(endpoint.clone(), ObjectConn { conn, wired: false });
    }
    let oc = object_shards.get_mut(&endpoint).expect("just inserted");

    // Wire the object-shard relays once per connection — they carry every zone
    // subscribed on this object shard; the client demuxes by `zone_id`. `free_things`
    // is the legacy loose-thing stream; `state` is the tick pipeline's resolved
    // entities (the moving demo objects).
    if !oc.wired {
        wire_object_relay(&oc.conn, out_tx.clone());
        wire_state_relay(&oc.conn, out_tx.clone());
        oc.wired = true;
    }

    let error_tx = out_tx.clone();
    let handle = oc
        .conn
        .subscription_builder()
        .on_error(move |_ctx, err| send(&error_tx, err_frame(&format!("object sub {sid}: {err}"))))
        .subscribe([
            format!("SELECT * FROM free_things WHERE zone_id = {zone_id}"),
            format!("SELECT * FROM state WHERE zone_id = {zone_id}"),
        ]);
    Some(handle)
}

/// Drive a Prison-Architect release for the thing affixed at `(zone_id,
/// location)`. Requires the zone to already be subscribed, so both the zone-shard
/// and object-shard upstreams are live. Installs the transfer bridge once, then
/// kicks the saga with `begin_release`; the bridge's callbacks carry it the rest
/// of the way (`receive` → `ack_release`).
async fn handle_release(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    object_shards: &HashMap<ShardEndpoint, ObjectConn>,
    bridge_wired: &mut bool,
    zone_id: u32,
    location: u8,
) {
    let zone_endpoint = match resolve_zone_or_default(&pool.index, &pool.cfg, zone_id) {
        Ok(e) => e,
        Err(err) => {
            send(out_tx, err_frame(&format!("release zone {zone_id}: {err}")));
            return;
        }
    };
    let Some(shard) = shards.get(&zone_endpoint) else {
        send(out_tx, err_frame(&format!("release: zone {zone_id:#010x} not subscribed")));
        return;
    };
    let object_endpoint = resolve_object_or_default(zone_id, &pool.cfg);
    let Some(object) = object_shards.get(&object_endpoint) else {
        send(out_tx, err_frame("release: object shard not connected (subscribe the zone first)"));
        return;
    };

    if !*bridge_wired {
        wire_transfer_bridge(&shard.conn, &object.conn);
        *bridge_wired = true;
    }

    if let Err(err) = shard.conn.reducers.begin_release(now_ms(), zone_id, location) {
        send(out_tx, err_frame(&format!("release: begin_release request failed: {err}")));
    }
}

/// Turn a client move intent into an `ACTION_MOVE` event on the object shard's tick
/// pipeline, targeting **this player's own object** (`OBJ_TYPE_PLAYER | player_id`).
/// The object materializes on the first move (work-gen carries a default base, the move
/// sets position), so no login-time seed is needed. Thin intent check: must be logged
/// in. In dev a single object shard serves every region; we resolve it from the
/// destination tile's zone (the client is subscribed near the origin, so it's live).
async fn handle_move(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    object_shards: &HashMap<ShardEndpoint, ObjectConn>,
    player_id: Option<u32>,
    tile_x: i32,
    tile_y: i32,
) {
    use resonantdust_codec::packed::{cell, pack_object_id, pack_object_key, OBJ_TYPE_PLAYER};

    let Some(player_id) = player_id else {
        send(out_tx, err_frame("move: not logged in"));
        return;
    };
    // Demo: keep the player object inside zone (0,0) so it stays on the client's
    // subscribed grid — wrap the clicked world tile into the 16×16 zone. (A real move
    // would honor the tile's actual zone; that's the cross-shard Phase-C concern.)
    let dest_zone = 0u32;
    let location = cell(tile_x.rem_euclid(16) as u8, tile_y.rem_euclid(16) as u8);
    let endpoint = resolve_object_or_default(dest_zone, &pool.cfg);
    let Some(object) = object_shards.get(&endpoint) else {
        send(out_tx, err_frame("move: object shard not connected (subscribe near the target first)"));
        return;
    };
    // The player's own entity; both actor and target (a self-move).
    let key = pack_object_key(pack_object_id(OBJ_TYPE_PLAYER, player_id));
    let data = resonantdust_tick::pack_move(dest_zone, location, 0, 0);
    if let Err(err) = object.conn.reducers.append_event(
        pool.cfg.server_id,
        0, // actor shard: same shard (self-move)
        key,
        key,
        resonantdust_tick::ACTION_MOVE,
        data[0],
        data[1],
    ) {
        send(out_tx, err_frame(&format!("move: request failed: {err}")));
    }
}

/// Install the release saga's cross-shard bridge on a (zone shard, object shard)
/// pair: subscribe each side's `transfers` table and wire the two hops that carry
/// a release across the DB boundary —
///   1. a pending outbound zone transfer → `object_shard::receive`, and
///   2. the object shard's received receipt → `zone_shard::ack_release`.
///
/// Both target reducers are idempotent on `transfer_id`, so a replayed callback
/// (e.g. rows redelivered on subscribe) is a no-op. The `transfers` subscription
/// handles are leaked for the connections' lifetime, like the players sub.
fn wire_transfer_bridge(
    zone_conn: &Arc<bindings::zone_shard::DbConnection>,
    object_conn: &Arc<bindings::object_shard::DbConnection>,
) {
    use bindings::object_shard::transfers_table::TransfersTableAccess as _;
    use bindings::zone_shard::transfers_table::TransfersTableAccess as _;

    // Hop 1: a pending outbound zone transfer → object `receive`.
    {
        let obj = object_conn.clone();
        zone_conn.db().transfers().on_insert(move |_c, row| {
            if row.direction == TRANSFER_DIR_OUT && row.state == TRANSFER_ST_PENDING {
                if let Err(err) = obj.reducers.receive(
                    now_ms(),
                    row.transfer_id,
                    row.zone_id,
                    row.location,
                    row.rotation,
                    row.id,
                ) {
                    tracing::warn!(transfer_id = row.transfer_id, %err, "transfer: receive relay failed");
                }
            }
        });
        let h = zone_conn
            .subscription_builder()
            .on_error(|_c, err| tracing::warn!(%err, "zone transfers subscription error"))
            .subscribe(["SELECT * FROM transfers"]);
        std::mem::forget(h);
    }

    // Hop 2: the object shard's received receipt → zone `ack_release`.
    {
        let zone = zone_conn.clone();
        object_conn.db().transfers().on_insert(move |_c, row| {
            if row.direction == TRANSFER_DIR_IN && row.state == TRANSFER_ST_RECEIVED {
                if let Err(err) = zone.reducers.ack_release(now_ms(), row.transfer_id) {
                    tracing::warn!(transfer_id = row.transfer_id, %err, "transfer: ack_release relay failed");
                }
            }
        });
        let h = object_conn
            .subscription_builder()
            .on_error(|_c, err| tracing::warn!(%err, "object transfers subscription error"))
            .subscribe(["SELECT * FROM transfers"]);
        std::mem::forget(h);
    }
}

/// Seed a fresh zone's terrain if the shard has no cold row for it yet. Called
/// once the zone subscription applies (so the cold cache reflects what's stored):
/// an existing row means the zone is already settled — leave it alone, so we
/// never clobber edits. A missing row means a never-generated zone, so worldgen
/// classifies its biomes into tiles + things and `seed_cold_zone` stores it; the resulting
/// insert relays back to every subscriber. No-op when worldgen is disabled
/// (content failed to load).
fn seed_zone_if_empty(
    conn: &Arc<bindings::zone_shard::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    zone_id: u32,
) {
    use bindings::zone_shard::cold_zones_table::ColdZonesTableAccess;

    let Some(wg) = worldgen else { return };
    // The cache holds only this connection's subscribed rows (possibly several
    // zones on one shard), so match by `zone_id`.
    if conn.db().cold_zones().iter().any(|z| z.zone_id == zone_id) {
        return;
    }
    let (tiles, things) = wg.zone_terrain(zone_id);
    match conn.reducers.seed_cold_zone(now_ms(), zone_id, tiles, things) {
        Ok(()) => tracing::info!(zone_id, "seeded fresh zone"),
        Err(err) => tracing::warn!(zone_id, %err, "seed_cold_zone request failed"),
    }
}

/// Server wall-clock time in milliseconds — the write time worldgen stamps a
/// seeded zone with (the shard trusts the caller's `now_ms`, mirroring the old
/// gateway). The `valid_at` sequence is allocated shard-side.
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Install insert/update/delete callbacks for the three zone tables on a shard
/// connection. Every fired row is converted to a [`RowData`] and pushed to the
/// outbound channel as a [`ServerMsg::Row`] (with the `sid: 0` demux-by-zone
/// sentinel).
fn wire_shard_relay(conn: &Arc<bindings::zone_shard::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::zone_shard::cold_zones_table::ColdZonesTableAccess;
    use bindings::zone_shard::hot_things_table::HotThingsTableAccess;
    use bindings::zone_shard::hot_tiles_table::HotTilesTableAccess;

    let db = conn.db();

    // cold_zones
    {
        let cold = db.cold_zones();
        let (oi, ou, od) = (out.clone(), out.clone(), out.clone());
        cold.on_insert(move |_c, r| relay(&oi, RowOp::Insert, RowData::ColdZone(cold_row(r))));
        cold.on_update(move |_c, _o, r| relay(&ou, RowOp::Update, RowData::ColdZone(cold_row(r))));
        cold.on_delete(move |_c, r| relay(&od, RowOp::Delete, RowData::ColdZone(cold_row(r))));
    }
    // hot_tiles
    {
        let t = db.hot_tiles();
        let (oi, ou, od) = (out.clone(), out.clone(), out.clone());
        t.on_insert(move |_c, r| relay(&oi, RowOp::Insert, RowData::HotTile(hot_row(r))));
        t.on_update(move |_c, _o, r| relay(&ou, RowOp::Update, RowData::HotTile(hot_row(r))));
        t.on_delete(move |_c, r| relay(&od, RowOp::Delete, RowData::HotTile(hot_row(r))));
    }
    // hot_things
    {
        let t = db.hot_things();
        let (oi, ou, od) = (out.clone(), out.clone(), out);
        t.on_insert(move |_c, r| relay(&oi, RowOp::Insert, RowData::HotThing(hot_row(r))));
        t.on_update(move |_c, _o, r| relay(&ou, RowOp::Update, RowData::HotThing(hot_row(r))));
        t.on_delete(move |_c, r| relay(&od, RowOp::Delete, RowData::HotThing(hot_row(r))));
    }
}

/// Install insert/update/delete callbacks for the object shard's `free_things`
/// table. Each fired row becomes a [`RowData::FreeThing`] pushed to the outbound
/// channel (the `sid: 0` demux-by-zone sentinel, like the zone-shard relay).
fn wire_object_relay(
    conn: &Arc<bindings::object_shard::DbConnection>,
    out: mpsc::UnboundedSender<String>,
) {
    use bindings::object_shard::free_things_table::FreeThingsTableAccess;

    let t = conn.db().free_things();
    let (oi, ou, od) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&oi, RowOp::Insert, RowData::FreeThing(free_thing_row(r))));
    t.on_update(move |_c, _o, r| relay(&ou, RowOp::Update, RowData::FreeThing(free_thing_row(r))));
    t.on_delete(move |_c, r| relay(&od, RowOp::Delete, RowData::FreeThing(free_thing_row(r))));
}

/// Copy a generated `FreeThing` row into the serde wire mirror.
fn free_thing_row(r: &bindings::object_shard::free_thing_type::FreeThing) -> FreeThingRow {
    FreeThingRow {
        valid_at: r.valid_at,
        object_id: r.object_id,
        zone_id: r.zone_id,
        location: r.location,
        rotation: r.rotation,
        id: r.id,
        offset: r.offset,
    }
}

/// Relay the object shard's `state` table (the tick pipeline's resolved entities) —
/// insert/update/delete forwarded as [`RowData::State`]. Wired once per object-shard
/// connection, like [`wire_object_relay`].
fn wire_state_relay(
    conn: &Arc<bindings::object_shard::DbConnection>,
    out: mpsc::UnboundedSender<String>,
) {
    use bindings::object_shard::state_table::StateTableAccess;

    let t = conn.db().state();
    let (si, su, sd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&si, RowOp::Insert, RowData::State(state_row(r))));
    t.on_update(move |_c, _o, r| relay(&su, RowOp::Update, RowData::State(state_row(r))));
    t.on_delete(move |_c, r| relay(&sd, RowOp::Delete, RowData::State(state_row(r))));
}

/// Copy a generated `State` row into the serde wire mirror.
fn state_row(r: &bindings::object_shard::state_type::State) -> StateRow {
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

/// Copy a generated `ColdZone` row into the serde wire mirror.
fn cold_row(r: &bindings::zone_shard::cold_zone_type::ColdZone) -> ColdZoneRow {
    ColdZoneRow {
        valid_at: r.valid_at,
        zone_id: r.zone_id,
        tiles: r.tiles.clone(),
        things: r.things.clone(),
    }
}

/// Copy either of the two identical hot rows into the serde wire mirror. The
/// generated types are distinct structs with identical fields, so this is a
/// macro over field access rather than a generic.
macro_rules! hot_row_impl {
    ($ty:path) => {
        impl HotRowSource for $ty {
            fn to_wire(&self) -> HotCellRow {
                HotCellRow {
                    valid_at: self.valid_at,
                    zone_id: self.zone_id,
                    location: self.location,
                    rotation: self.rotation,
                    id: self.id,
                }
            }
        }
    };
}
trait HotRowSource {
    fn to_wire(&self) -> HotCellRow;
}
hot_row_impl!(bindings::zone_shard::hot_tile_type::HotTile);
hot_row_impl!(bindings::zone_shard::hot_thing_type::HotThing);

fn hot_row<T: HotRowSource>(r: &T) -> HotCellRow {
    r.to_wire()
}

/// Push a row frame to the outbound channel (sid 0 — demux by zone client-side).
fn relay(out: &mpsc::UnboundedSender<String>, op: RowOp, row: RowData) {
    send(out, ServerMsg::Row { sid: 0, op, row });
}

/// Serialize a [`ServerMsg`] and enqueue it. A closed channel (client gone) is a
/// no-op — the session is already tearing down.
fn send(out: &mpsc::UnboundedSender<String>, msg: ServerMsg) {
    let _ = out.send(msg.to_text());
}

/// Shorthand for a protocol-level error frame.
fn err_frame(msg: &str) -> ServerMsg {
    ServerMsg::Error {
        error: msg.to_string(),
    }
}
