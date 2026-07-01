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
use crate::bindings::region_shard::seed_cold_zone as _; // reducer trait → `reducers.seed_cold_zone`
use crate::connections::{await_ready, connect_players, connect_shard, Pool};
use crate::index::{resolve_zone_or_default, ShardEndpoint};
use crate::protocol::{ClientMsg, ColdZoneRow, HotCellRow, RowData, RowOp, ServerMsg};

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

/// A live shard upstream plus whether its row callbacks are installed. One per
/// distinct endpoint this client touches.
struct ShardConn {
    conn: Arc<bindings::region_shard::DbConnection>,
    /// Row callbacks (cold + 3 hot tables) are wired once per connection — they
    /// relay every subscribed zone's rows, demuxed client-side by `zone_id`.
    wired: bool,
}

/// A live zone subscription: the one handle covering its four table queries, and
/// the endpoint it lives on (so teardown knows which shard it used).
struct ZoneSub {
    #[allow(dead_code)]
    endpoint: ShardEndpoint,
    handle: bindings::region_shard::SubscriptionHandle,
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

    // Single outbound funnel: handlers here and SDK callbacks on upstream threads
    // both push text frames; one writer task drains them to the WS sink.
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(txt) = out_rx.recv().await {
            if sink.send(Message::Text(txt.into())).await.is_err() {
                break;
            }
        }
    });

    // Players upstream, built eagerly: login is the first thing most clients do,
    // and the row read needs the subscription warm. A failure here leaves
    // `players = None`; login then reports the outage instead of hanging.
    let players = build_players(&pool, &out_tx).await;

    let mut session: Option<u32> = None;
    let mut shards: HashMap<ShardEndpoint, ShardConn> = HashMap::new();
    let mut subs: HashMap<u32, ZoneSub> = HashMap::new();

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
            ClientMsg::Login {
                cid,
                client_time_ms,
                name,
            } => {
                handle_login(players.as_ref(), &out_tx, &mut session, cid, client_time_ms, name)
                    .await;
            }
            ClientMsg::SubZone { sid, zone_id } => {
                handle_sub_zone(&pool, &out_tx, &mut shards, &mut subs, sid, zone_id).await;
            }
            ClientMsg::Unsub { sid } => {
                if let Some(zs) = subs.remove(&sid) {
                    let _ = zs.handle.unsubscribe();
                } else {
                    tracing::debug!(sid, "unsub for unknown sid");
                }
            }
        }
    }

    // Teardown: drop zone subscriptions, then disconnect every upstream. The
    // writer task is aborted last (its senders are scattered across the SDK
    // callbacks we're tearing down).
    for (_sid, zs) in subs.drain() {
        let _ = zs.handle.unsubscribe();
    }
    for (_endpoint, sc) in shards.drain() {
        let _ = sc.conn.disconnect();
    }
    if let Some(conn) = &players {
        let _ = conn.disconnect();
    }
    writer.abort();
    tracing::debug!(player = ?session, "client session ended");
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
    let seed_wg = pool.worldgen.clone();
    let handle = shard
        .conn
        .subscription_builder()
        .on_applied(move |_ctx| {
            send(&applied_tx, ServerMsg::Applied { sid });
            seed_zone_if_empty(&seed_conn, seed_wg.as_deref(), zone_id);
        })
        .on_error(move |_ctx, err| send(&error_tx, err_frame(&format!("sub {sid}: {err}"))))
        .subscribe([
            q("cold_zones"),
            q("hot_tiles"),
            q("hot_things"),
        ]);

    subs.insert(sid, ZoneSub { endpoint, handle });
}

/// Seed a fresh zone's terrain if the shard has no cold row for it yet. Called
/// once the zone subscription applies (so the cold cache reflects what's stored):
/// an existing row means the zone is already settled — leave it alone, so we
/// never clobber edits. A missing row means a never-generated zone, so worldgen
/// builds its grass/dirt terrain and `seed_cold_zone` stores it; the resulting
/// insert relays back to every subscriber. No-op when worldgen is disabled
/// (content failed to load).
fn seed_zone_if_empty(
    conn: &Arc<bindings::region_shard::DbConnection>,
    worldgen: Option<&crate::worldgen::Worldgen>,
    zone_id: u32,
) {
    use bindings::region_shard::cold_zones_table::ColdZonesTableAccess;

    let Some(wg) = worldgen else { return };
    // The cache holds only this connection's subscribed rows (possibly several
    // zones on one shard), so match by `zone_id`.
    if conn.db().cold_zones().iter().any(|z| z.zone_id == zone_id) {
        return;
    }
    let tiles = wg.zone_tiles(zone_id);
    match conn.reducers.seed_cold_zone(now_ms(), zone_id, tiles, Vec::new()) {
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
fn wire_shard_relay(conn: &Arc<bindings::region_shard::DbConnection>, out: mpsc::UnboundedSender<String>) {
    use bindings::region_shard::cold_zones_table::ColdZonesTableAccess;
    use bindings::region_shard::hot_things_table::HotThingsTableAccess;
    use bindings::region_shard::hot_tiles_table::HotTilesTableAccess;

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

/// Copy a generated `ColdZone` row into the serde wire mirror.
fn cold_row(r: &bindings::region_shard::cold_zone_type::ColdZone) -> ColdZoneRow {
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
hot_row_impl!(bindings::region_shard::hot_tile_type::HotTile);
hot_row_impl!(bindings::region_shard::hot_thing_type::HotThing);

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
