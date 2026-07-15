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
use crate::bindings::shard::append as _; // reducer trait → `reducers.append` (DSL word stream + targets)
use crate::bindings::shard::set_paused as _; // reducer trait → `reducers.set_paused` (debug freeze)
use crate::bindings::shard::seed_cold_row as _; // reducer trait → `reducers.seed_cold_row` (terrain)
use crate::connections::{await_ready, connect_players, connect_shard, Pool};
use crate::index::{resolve_zone_or_default, ShardEndpoint};
use crate::protocol::{ClientMsg, ColdObjectsRow, RowData, RowOp, ServerMsg, StateRow};

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
    /// Keeps the shard-global `tic_meta` subscription alive (wired once with the relays), so
    /// the [`wire_paused_relay`] callbacks fire on a `set_paused` change. Dropping it unsubs.
    tic_sub: Option<bindings::shard::SubscriptionHandle>,
}

/// A live zone subscription — the zone's object `state` rows + its cold-object rows, both
/// from the shard. Unsub tears down both.
struct ZoneSub {
    #[allow(dead_code)]
    endpoint: ShardEndpoint,
    handle: bindings::shard::SubscriptionHandle,
    /// The zone's cold-OBJECT rows (`shard::cold` — the object model; cold objects live on
    /// the object module beside the hot `state`, so `unpack`/`pack` stay in-module).
    cold_handle: bindings::shard::SubscriptionHandle,
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
    let mut subs: HashMap<u32, ZoneSub> = HashMap::new();

    while let Some(cmsg) = cmd_rx.recv().await {
        match cmsg {
            ClientMsg::Login { cid, client_time_ms, name } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
            }
            ClientMsg::SubZone { sid, zone_id } => {
                handle_sub_zone(&pool, &out_tx, &mut shards, &mut subs, sid, zone_id).await;
            }
            ClientMsg::Unsub { sid } => {
                if let Some(zs) = subs.remove(&sid) {
                    let _ = zs.handle.unsubscribe();
                    let _ = zs.cold_handle.unsubscribe();
                } else {
                    tracing::debug!(sid, "unsub for unknown sid");
                }
            }
            ClientMsg::Spawn { entity_key, kind, tile_x, tile_y } => {
                handle_spawn(&pool, &out_tx, &shards, session, entity_key, kind, tile_x, tile_y)
                    .await;
            }
            ClientMsg::Move { pawn, tile_x, tile_y } => {
                handle_move(&pool, &out_tx, &shards, session, pawn, tile_x, tile_y).await;
            }
            ClientMsg::Interact { tile_x, tile_y } => {
                handle_interact(&pool, &out_tx, &shards, session, tile_x, tile_y).await;
            }
            ClientMsg::SetPaused { paused } => {
                handle_set_paused(&out_tx, &shards, session, paused);
            }
            // Pings are answered on the read loop and never forwarded here.
            ClientMsg::Ping { .. } => {}
        }
    }

    // Teardown: drop zone subscriptions, then disconnect every upstream.
    for (_sid, zs) in subs.drain() {
        let _ = zs.handle.unsubscribe();
        let _ = zs.cold_handle.unsubscribe();
    }
    for (_endpoint, sc) in shards.drain() {
        let _ = sc.conn.disconnect();
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
    subs: &mut HashMap<u32, ZoneSub>,
    sid: u32,
    zone_id: u32,
) {
    use resonantdust_codec::packed::zone_realm;

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
        shards.insert(endpoint.clone(), ShardConn { conn, wired: false, tic_sub: None });
    }
    let shard = shards.get_mut(&endpoint).expect("just inserted");

    if !shard.wired {
        wire_state_relay(&shard.conn, out_tx.clone());
        // Cold objects (the object model) live on THIS module beside the hot `state`, so
        // wire their relay on the same connection — one shard serves hot movers + cold
        // terrain, and `unpack`/`pack` stay in-module.
        wire_cold_relay(&shard.conn, out_tx.clone(), zone_realm(zone_id));
        // The shard's freeze flag (`tic_meta.paused`) — relay changes to this client so a
        // `/pause` from any session reaches every subscriber (including tic-driven npcs).
        // Subscribed shard-globally (once), separate from the per-zone subs.
        wire_paused_relay(&shard.conn, out_tx.clone());
        shard.tic_sub = Some(
            shard
                .conn
                .subscription_builder()
                .subscribe(["SELECT * FROM tic_meta".to_string()]),
        );
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

    // ── cold objects: the zone's ColdRows from THIS shard's `cold` table (object model —
    // biome-tile ground + biome-thing scatter, biome in the subtype). Same connection as
    // the hot `state`; seeded from `zone_cold_objects` on apply. Additive alongside the
    // legacy tiles/things subs while the client migrates.
    let cold_seed_conn = shard.conn.clone();
    let cold_seed_pool = pool.clone();
    let cold_error = out_tx.clone();
    let cold_handle = shard
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            let wg = cold_seed_pool.current_worldgen();
            seed_cold_if_empty(&cold_seed_conn, wg.as_deref(), zone_id);
        })
        .on_error(move |_ctx, err| send(&cold_error, err_frame(&format!("cold sub {sid}: {err}"))))
        // Cold rows carry `macro_position` (region|zone) — realm is the shard's, never repeated
        // per row — so the zone filter is the zone_id's macro slice, not the whole zone_id.
        .subscribe([format!(
            "SELECT * FROM cold WHERE macro_position = {}",
            resonantdust_codec::packed::zone_macro_position(zone_id)
        )]);

    subs.insert(sid, ZoneSub { endpoint, handle, cold_handle });
}

/// Materialize an entity through the tick pipeline: append an `ACTION_SPAWN` event
/// targeting `entity_key`. Work-gen creates the pending row for the new target, `ACTION_SPAWN`
/// fills in its `kind` + placement, and the worker promotes it to `state` — so a pawn is
/// minted through the same event path a move travels, not a privileged direct write. The
/// caller chooses `entity_key` (typically `pack_minted_entity`) so it can address the entity
/// for later moves. Zone (0,0) only for now (like [`handle_move`]); must be logged in and the
/// zone's shard connected.
#[allow(clippy::too_many_arguments)]
async fn handle_spawn(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    player_id: Option<u32>,
    entity_key: u64,
    kind: u16,
    tile_x: i32,
    tile_y: i32,
) {
    use resonantdust_codec::packed::cell;

    if player_id.is_none() {
        send(out_tx, err_frame("spawn: not logged in"));
        return;
    }
    let dest_zone = 0u32;
    let location = cell(tile_x.rem_euclid(16) as u8, tile_y.rem_euclid(16) as u8);
    let endpoint = match resolve_zone_or_default(&pool.index, &pool.cfg, dest_zone) {
        Ok(e) => e,
        Err(err) => {
            send(out_tx, err_frame(&format!("spawn: route zone {dest_zone}: {err}")));
            return;
        }
    };
    let Some(shard) = shards.get(&endpoint) else {
        send(out_tx, err_frame("spawn: shard not connected (subscribe the zone first)"));
        return;
    };
    // Build a SPAWN word-stream program and append it targeting the (caller-minted) entity_key.
    // `encode_spawn` carries the kind so the materialized entity is non-tombstone and renders.
    let actions = resonantdust_tick::vm::encode_spawn(kind, dest_zone, location, 0, 0);
    if let Err(err) = shard.conn.reducers.append(actions, vec![entity_key]) {
        send(out_tx, err_frame(&format!("spawn: request failed: {err}")));
    } else {
        tracing::info!(zone = dest_zone, location, kind, entity_key, "spawn → materialized entity");
    }
}

/// Turn a client move intent into an `ACTION_MOVE` event on the shard's tick pipeline.
/// `pawn` selects the target: `None` moves **this player's own object** (a hot entity keyed by
/// `player_id`, a self-move — the object materializes on its first move via work-gen's
/// default base); `Some(entity_key)` moves that specific entity (an automated player driving
/// its wolves). Thin intent check: must be logged in. **No ownership check** — today any
/// logged-in session may move any pawn; the ownership seam lands with per-npc ownership.
async fn handle_move(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    player_id: Option<u32>,
    pawn: Option<u64>,
    tile_x: i32,
    tile_y: i32,
) {
    use resonantdust_codec::packed::cell;
    use resonantdust_codec::refs::{pack_hot_entity, SERVER_REF_NONE};

    let Some(player_id) = player_id else {
        send(out_tx, err_frame("move: not logged in"));
        return;
    };
    // Keep the object inside zone (0,0) so it stays on the client's subscribed grid — wrap
    // the clicked world tile into the 16×16 zone. (A real move would honor the tile's actual
    // zone; cross-zone movement is a later phase.)
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
    // The target: an explicit pawn (automated player), else the player's own entity (a
    // self-move). The player's own key is not shard-minted: SERVER_REF_NONE as the qualifying
    // server + the (globally-unique) player_id as the hot_reference.
    let target = pawn
        .unwrap_or_else(|| pack_hot_entity(SERVER_REF_NONE, player_id));
    // Build a MOVE word-stream program and append it targeting the pawn.
    let actions = resonantdust_tick::vm::encode_move(dest_zone, location, 0, 0);
    if let Err(err) = shard.conn.reducers.append(actions, vec![target]) {
        send(out_tx, err_frame(&format!("move: request failed: {err}")));
    }
}

/// Turn a client **interact** intent (a click on a cold thing) into an `unpack` on the
/// object module — the edge-triggered cold→hot bridge. Finds the `biome-thing` cold object
/// at the clicked cell in the shard's cached `cold` rows, decodes its kind, and unpacks it:
/// it becomes a hot `state` entity (a free object) the pipeline then ticks; the cold row
/// loses it. Zone (0,0) only for now (like [`handle_move`]).
async fn handle_interact(
    pool: &Arc<Pool>,
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    player_id: Option<u32>,
    tile_x: i32,
    tile_y: i32,
) {
    use bindings::shard::cold_table::ColdTableAccess;
    use resonantdust_codec::object::{
        kind_pos_ref_tile, type_ref_type_id, TYPE_BIOME_THING,
    };
    use resonantdust_codec::packed::cell;

    if player_id.is_none() {
        send(out_tx, err_frame("interact: not logged in"));
        return;
    }
    let zone = 0u32;
    let zone_macro = resonantdust_codec::packed::zone_macro_position(zone);
    let (x, y) = (tile_x.rem_euclid(16) as u8, tile_y.rem_euclid(16) as u8);
    let endpoint = match resolve_zone_or_default(&pool.index, &pool.cfg, zone) {
        Ok(e) => e,
        Err(err) => {
            send(out_tx, err_frame(&format!("interact: route zone {zone}: {err}")));
            return;
        }
    };
    let Some(shard) = shards.get(&endpoint) else {
        send(out_tx, err_frame("interact: shard not connected (subscribe the zone first)"));
        return;
    };
    // Find the cold object an interact means at (x, y): a **biome-thing on layer 0** — never the
    // ground beneath it. Select the row by `(zone, type_id, layer_id)` and match the entry by its
    // `tile_reference`; the uniqueness rule (one object per (type, layer, tile), subtype-agnostic)
    // makes that exactly one object, across however many subtype (biome) rows the zone has.
    const INTERACT_LAYER: u8 = 0;
    let tile = cell(x, y);
    let found = shard
        .conn
        .db()
        .cold()
        .iter()
        .filter(|c| {
            c.macro_position == zone_macro
                && type_ref_type_id(c.type_reference) == TYPE_BIOME_THING
                && c.layer_id == INTERACT_LAYER
        })
        .find_map(|c| {
            c.kinds
                .iter()
                .find(|&&k| kind_pos_ref_tile(k) == tile)
                .map(|_| c.type_reference)
        });
    let Some(type_reference) = found else {
        send(out_tx, err_frame(&format!("interact: no cold thing at ({x},{y})")));
        return;
    };
    // Target the cold object by its POSITIONAL entity_reference; the worker's enqueue
    // find-or-mint promotes it hot (kind from cold) before the action runs (issue 005). An
    // empty action program just makes it hot (interact = "bring it to life"); richer verbs later.
    // The target is a geographic cold_reference (region|zone|tile|layer) qualified by the shard's
    // realm (server_reference).
    use resonantdust_codec::object::{pack_cold_reference, pack_layer_reference};
    use resonantdust_codec::packed::{zone_realm, zone_region, zone_zone};
    use resonantdust_codec::refs::{pack_cold_entity, pack_server_reference};
    let type_id = type_ref_type_id(type_reference);
    let cold_reference = pack_cold_reference(
        zone_region(zone),
        zone_zone(zone),
        tile,
        pack_layer_reference(type_id, INTERACT_LAYER),
    );
    let target = pack_cold_entity(pack_server_reference(zone_realm(zone), 0), cold_reference);
    if let Err(err) = shard.conn.reducers.append(Vec::new(), vec![target]) {
        send(out_tx, err_frame(&format!("interact: request failed: {err}")));
    } else {
        tracing::info!(zone, x, y, target, "interact → cold target queued (worker will mint)");
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

/// Install insert/update/delete callbacks on the `shard` module's `cold` table (the
/// object model — cold objects live on the object module beside the hot `state`); each
/// fired row relays as a [`RowData::ColdObjects`] frame (demux-by-zone, `sid: 0`).
fn wire_cold_relay(conn: &Arc<bindings::shard::DbConnection>, out: mpsc::UnboundedSender<String>, realm: u8) {
    use bindings::shard::cold_table::ColdTableAccess;

    let t = conn.db().cold();
    let (ci, cu, cd) = (out.clone(), out.clone(), out);
    t.on_insert(move |_c, r| relay(&ci, RowOp::Insert, RowData::ColdObjects(cold_row(realm, r))));
    t.on_update(move |_c, _o, r| relay(&cu, RowOp::Update, RowData::ColdObjects(cold_row(realm, r))));
    t.on_delete(move |_c, r| relay(&cd, RowOp::Delete, RowData::ColdObjects(cold_row(realm, r))));
}

/// Copy a `cold` row into the serde wire mirror. The row carries `macro_position` (realm is the
/// shard's — never repeated per row), but the client demuxes by the world-global `zone_id`, so
/// reconstruct it here: `realm | region | zone`. `realm` comes from the zone this connection was
/// subscribed for (every zone on one shard shares its realm).
fn cold_row(realm: u8, r: &bindings::shard::cold_type::Cold) -> ColdObjectsRow {
    ColdObjectsRow {
        zone_id: ((realm as u32) << 24) | ((r.macro_position as u32) << 8),
        type_reference: r.type_reference,
        layer_id: r.layer_id,
        kinds: r.kinds.clone(),
    }
}

/// Relay the shard's freeze flag (`tic_meta.paused`) to this client. `tic_meta` updates every
/// tic (the master bumps `master_tic`), so only forward a [`ServerMsg::Paused`] when `paused`
/// actually flips — on insert (the first row: the initial state) and on an update that changes
/// it. Keeps `/pause` authoritative (reflects the real shard flag) without per-tic spam.
fn wire_paused_relay(conn: &Arc<bindings::shard::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::shard::tic_meta_table::TicMetaTableAccess;

    let t = conn.db().tic_meta();
    let (pi, pu) = (out.clone(), out);
    t.on_insert(move |_c, r| send(&pi, ServerMsg::Paused { paused: r.paused }));
    t.on_update(move |_c, old, new| {
        if old.paused != new.paused {
            send(&pu, ServerMsg::Paused { paused: new.paused });
        }
    });
}

/// Debug freeze: relay the client's `/pause` to every shard this session has connected,
/// calling `set_paused`. The master then stops advancing those shards' tics, and the change
/// relays back to all subscribers via [`wire_paused_relay`].
fn handle_set_paused(
    out_tx: &mpsc::UnboundedSender<String>,
    shards: &HashMap<ShardEndpoint, ShardConn>,
    session: Option<u32>,
    paused: bool,
) {
    if session.is_none() {
        send(out_tx, err_frame("set_paused before login"));
        return;
    }
    if shards.is_empty() {
        send(out_tx, err_frame("set_paused before subscribing a zone"));
        return;
    }
    for sc in shards.values() {
        if let Err(err) = sc.conn.reducers.set_paused(paused) {
            tracing::warn!(%err, paused, "set_paused request failed");
        }
    }
    tracing::info!(paused, shards = shards.len(), "set_paused");
}

/// Seed a zone's cold OBJECTS (biome-tile ground + biome-thing scatter) from worldgen if
/// the zone has no `cold` rows yet — the object-model path (`docs/components/shared/codec/design/object-model.md`).
/// Each `ColdRow` of [`Worldgen::zone_cold_objects`] becomes one `cold` row via
/// `seed_cold_row` (insert-if-absent per `(zone, type_reference)`, so a worldgen re-run
/// never clobbers a mutated row). No-op when worldgen is disabled.
fn seed_cold_if_empty(
    conn: &Arc<bindings::shard::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    zone_id: u32,
) {
    use bindings::shard::cold_table::ColdTableAccess;

    let Some(wg) = worldgen else { return };
    let macro_position = resonantdust_codec::packed::zone_macro_position(zone_id);
    if conn.db().cold().iter().any(|c| c.macro_position == macro_position) {
        return;
    }
    let rows = wg.zone_cold_objects(zone_id);
    let n = rows.len();
    for row in rows {
        if let Err(err) =
            conn.reducers.seed_cold_row(macro_position, row.type_reference, row.layer_id, row.kinds)
        {
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
