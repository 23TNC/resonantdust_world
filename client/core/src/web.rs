//! The web engine — the browser transport behind the host-facing [`Client`]
//! handle, the wasm counterpart to [`crate::engine`].
//!
//! It is the same session core ([`Command`]s in, [`Event`]s out, the
//! anchor-driven [`ZoneManager`]) wearing a browser-native skin:
//!   * the **command channel** is a [`futures`] mpsc the host's handle sends on;
//!   * the **world-server socket** is a [`ws_stream_wasm`] `WsStream`, split into
//!     a write sink kept on the engine and a read half pumped by a small
//!     [`spawn_local`](wasm_bindgen_futures::spawn_local) task that forwards
//!     frames back over a second mpsc;
//!   * the **gateway round-trip** is a [`gloo_net`] `fetch`.
//!
//! Browser JS is single-threaded, so there is no tokio runtime and no
//! `Send` requirement — the engine task is `spawn_local`'d and the [`EventSink`]
//! (a `js_sys::Function` shim) need not be `Send + Sync` (see [`crate::api`]).
//!
//! The read half lives in its own task rather than the `select!` loop because a
//! `WsStream` split can't be borrowed by the loop while a command handler swaps
//! the socket on reconnect. Each connection carries a **generation**; the pump
//! tags every frame with it, and the loop drops frames from a superseded socket.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use futures::channel::mpsc;
use futures::stream::{SplitSink, SplitStream};
use futures::{select, SinkExt, StreamExt};
use gloo_timers::future::IntervalStream;
use ws_stream_wasm::{WsMessage, WsMeta, WsStream};

use crate::api::{CallStat, Command, Event, EventSink, ServerInfo, SubStat};
use crate::clock::Clock;
use crate::config::ClientConfig;
use crate::protocol::{ClientMsg, ServerMsg};
use crate::world;
use crate::zones::{AnchorRadii, ZoneManager};

/// Clock-sync ping cadence, ms. Matches the native engine's `PING_INTERVAL`: a
/// fresh session converges within a few seconds, then costs one tiny frame each
/// way every couple of seconds for the rest of the session.
const PING_INTERVAL_MS: u32 = 2_000;

/// The write half of the live socket. The read half is moved into the pump task.
type WsSink = SplitSink<WsStream, WsMessage>;

/// Returned by [`Client::send`] when the engine task is gone (client shut down or
/// dropped). The unsent command is handed back.
#[derive(Debug)]
pub struct SendError(pub Command);

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client engine is no longer running")
    }
}

impl std::error::Error for SendError {}

/// A host's handle to a running client. Cheap to clone; every clone drives the
/// same engine. Dropping the last handle closes the command channel, which stops
/// the engine after it tears the session down.
#[derive(Clone)]
pub struct Client {
    cmd_tx: mpsc::UnboundedSender<Command>,
    /// The [`ClientWorld`](crate::client_world::ClientWorld) the engine folds every event
    /// into — see [`Client::world`].
    world: crate::client_world::SharedWorld,
}

impl Client {
    /// Spawn a client engine on the current thread's microtask executor (via
    /// [`spawn_local`](wasm_bindgen_futures::spawn_local)) and return its handle.
    /// `sink` receives every [`Event`] the client emits.
    /// **Ask core.** The read half of the one contract: commands in, events out, and answers on
    /// request — the same surface the native host has, because "one contract for every host" is
    /// only true if both hosts can ask the same questions.
    pub fn world(&self) -> &crate::client_world::SharedWorld {
        &self.world
    }

    /// Where a pawn is right now, in fractional tiles.
    pub fn pawn_point(&self, entity: u32) -> Option<(f64, f64)> {
        let w = self.world.lock().ok()?;
        let now = w.now_tic_at(now_ms() as f64)?;
        w.pawn_point(entity, now)
    }

    /// What tic it is. `None` until the estimate anchors — BAIL on that, never substitute 0.
    pub fn now_tic(&self) -> Option<u16> {
        self.world.lock().ok()?.now_tic_at(now_ms() as f64)
    }

    /// May a pawn stand on this tile.
    pub fn pathable(&self, x: i32, y: i32) -> bool {
        self.world.lock().map(|w| w.pathable(x, y)).unwrap_or(true)
    }

    /// Every tracked entity. Returns owned ids, deliberately: the alternative is a host holding
    /// the world guard while it iterates, and then calling `pawn_point` inside that loop — which
    /// re-enters a non-reentrant mutex and hangs. That has now happened twice (I9 in npc's
    /// scans, then again in the headless probe), so the shape that invites it is gone.
    pub fn mover_entities(&self) -> Vec<u32> {
        self.world.lock().map(|w| w.movers.iter().map(|(e, _)| e).collect()).unwrap_or_default()
    }

    /// The host just queued an interaction for `entity`: hold it busy until the fan confirms, or
    /// `duration` tics pass. Closes the fire->fan gap in which the queue says idle and the pawn
    /// is not (P3b).
    pub fn arm_pending(&self, entity: u32, duration_tics: u16) {
        if let Ok(mut w) = self.world.lock() {
            let Some(now) = w.now_tic_at(0.0) else { return };
            w.intents.arm_pending(entity, now, duration_tics);
        }
    }

    /// Is this pawn committed to something right now — see [`crate::intents::IntentQueues::busy`].
    pub fn pawn_busy(&self, entity: u32) -> bool {
        let Ok(w) = self.world.lock() else { return false };
        let Some(now) = w.now_tic_at(0.0) else { return false };
        w.intents.busy(entity, now)
    }

    /// When this pawn's running act completes, if one is running.
    pub fn pawn_fires_at(&self, entity: u32) -> Option<u16> {
        self.world.lock().ok()?.intents.fires_at(entity)
    }

    /// This pawn's committed queue entries.
    pub fn pawn_queue(&self, entity: u32) -> Vec<crate::intents::QueueEntry> {
        self.world.lock().map(|w| w.intents.entries(entity).to_vec()).unwrap_or_default()
    }

    /// This pawn's DERIVED pace in tics per tile. `None` until its rows have arrived — a caller
    /// must not substitute a default, which is an 8x error on the pawns it is wrong about (I2).
    pub fn pawn_pace(&self, entity: u32) -> Option<f64> {
        self.world.lock().ok()?.movers.get(entity)?.pace
    }

    /// This entity's raw payload opcode stream — the trait/condition rows every eval reads.
    pub fn pawn_payload(&self, entity: u32) -> Vec<u32> {
        self.world.lock().map(|w| w.rows.payload(entity).to_vec()).unwrap_or_default()
    }

    /// This entity's `needs` sub-table rows as the `(row, set_tic)` pairs the evals take.
    pub fn pawn_needs(&self, entity: u32) -> Vec<(u64, u16)> {
        self.world.lock().map(|w| w.rows.needs(entity)).unwrap_or_default()
    }

    /// Hand core the corpus. Until this lands core answers nothing derived.
    pub fn set_corpus(&self, bundle: std::sync::Arc<resonantdust_content::loader::Bundle>) {
        if let Ok(mut w) = self.world.lock() {
            w.set_corpus(bundle);
        }
    }

    pub fn spawn(config: ClientConfig, sink: impl EventSink) -> Client {
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let world: crate::client_world::SharedWorld = Default::default();
        let (engine, frame_rx) = Engine::new(config, Arc::new(sink), world.clone());
        wasm_bindgen_futures::spawn_local(engine.run(cmd_rx, frame_rx));
        Client { cmd_tx, world }
    }

    /// Send a command to the engine. Errors only if the engine has stopped.
    pub fn send(&self, cmd: Command) -> Result<(), SendError> {
        self.cmd_tx
            .unbounded_send(cmd)
            .map_err(|e| SendError(e.into_inner()))
    }

    /// Convenience: log in as `name` (acquire a server, connect, authenticate).
    pub fn login(&self, name: impl Into<String>) -> Result<(), SendError> {
        self.send(Command::Login { name: name.into() })
    }

    /// Convenience: add or move the anchor named `name` (see
    /// [`Command::SetAnchor`]).
    pub fn set_anchor(
        &self,
        name: impl Into<String>,
        tile_x: i32,
        tile_y: i32,
        radii: AnchorRadii,
        soul: u32,
    ) -> Result<(), SendError> {
        self.send(Command::SetAnchor {
            name: name.into(),
            tile_x,
            tile_y,
            radii,
            soul,
        })
    }

    /// Convenience: remove the anchor named `name`.
    pub fn remove_anchor(&self, name: impl Into<String>) -> Result<(), SendError> {
        self.send(Command::RemoveAnchor { name: name.into() })
    }

    /// Convenience: queue a raw action program (see [`Command::Queue`]).
    pub fn queue(&self, actions: Vec<u32>) -> Result<(), SendError> {
        self.send(Command::Queue { actions })
    }

    /// Convenience: seed the tic estimator's rate from a persisted hint (see
    /// [`Command::SeedTicRate`]).
    pub fn seed_tic_rate(&self, tics_per_sec: f64) -> Result<(), SendError> {
        self.send(Command::SeedTicRate { tics_per_sec })
    }

    /// Convenience: order walls on the `(start..end)` rect's perimeter (see
    /// [`Command::BuildWall`]).
    pub fn build_wall(&self, start_x: i32, start_y: i32, end_x: i32, end_y: i32, object: u32) -> Result<(), SendError> {
        self.send(Command::BuildWall { start_x, start_y, end_x, end_y, object })
    }

    /// Convenience: place + promote `entity` at global tile `(tile_x, tile_y)` (see
    /// [`Command::Place`]).
    pub fn place(&self, entity: u32, tile_x: i32, tile_y: i32) -> Result<(), SendError> {
        self.send(Command::Place { entity, tile_x, tile_y })
    }

    /// Convenience: drop the world-server connection but keep the engine alive.
    pub fn logout(&self) -> Result<(), SendError> {
        self.send(Command::Logout)
    }

    /// Convenience: stop the engine.
    pub fn shutdown(&self) -> Result<(), SendError> {
        self.send(Command::Shutdown)
    }
}

/// A login awaiting its server reply: the `cid` we stamped and the name we sent.
struct Pending {
    cid: u32,
    name: String,
}

/// Running counters for one outbound command type, feeding the debug HUD's "calls"
/// tab. `requests` / `tx` accrue on every sent frame; `ok` / `err` / `rx` on the
/// correlated replies (see [`CallStat`]).
#[derive(Default)]
struct CallCounters {
    requests: u32,
    ok: u32,
    err: u32,
    tx: u64,
    rx: u64,
}

/// The wire tag of a client frame — the key its tally accrues under, matching the
/// `#[serde(rename_all = "snake_case")]` discriminator the server sees.
fn client_msg_tag(msg: &ClientMsg) -> &'static str {
    match msg {
        ClientMsg::Login { .. } => "login",
        ClientMsg::Ping { .. } => "ping",
        ClientMsg::Queue { .. } => "queue",
        ClientMsg::SubscribeZone { .. } => "subscribe_zone",
        ClientMsg::UnsubscribeZone { .. } => "unsubscribe_zone",
    }
}

/// Running data counters for one relayed-row table, feeding the debug HUD's "subs"
/// tab: how many rows of this table arrived and their total bytes.
#[derive(Default)]
struct SubCounters {
    rows: u32,
    rx: u64,
}

/// What the read pump forwards, normalized so the loop doesn't touch the
/// `ws_stream_wasm` types. Each is tagged with the connection generation by the
/// pump so the loop can drop a stale socket's tail.
enum FrameOutcome {
    /// A text frame (the only kind the protocol uses).
    Text(String),
    /// A non-text (binary) frame — ignored.
    Other,
    /// The stream ended (the socket closed).
    Closed,
}

/// The engine's owned state.
struct Engine {
    config: ClientConfig,
    sink: Arc<dyn EventSink>,
    /// The [`ClientWorld`](crate::client_world::ClientWorld). `emit` folds into it before
    /// handing the event on, so the fold happens once for every host, not once per host.
    world: crate::client_world::SharedWorld,
    next_cid: u32,
    server_url: Option<String>,
    /// Write half of the live socket, if connected.
    write: Option<WsSink>,
    /// The socket's control handle, retained so [`Self::disconnect`] can close it.
    meta: Option<WsMeta>,
    pending: Option<Pending>,
    player_id: Option<u32>,
    zones: ZoneManager,
    /// The zones currently subscribed on the wire (by `macro_position_reference`); its length is the
    /// live "open" gauge. Replaces the old `zone_id → sid` map — the new protocol keys by zone.
    open_zones: HashSet<u16>,
    /// Connection generation, bumped on every (re)connect and teardown. The pump
    /// stamps frames with the generation live when it started; the loop ignores
    /// any whose generation no longer matches (a superseded socket's tail).
    generation: u64,
    /// Cloned into each read pump so it can forward frames into the loop. Each
    /// item is `(generation, recv_ms, outcome)`: the recv timestamp is stamped at
    /// ingress in [`read_pump`] — *not* when the loop later processes the frame —
    /// so a clock-sync Pong sitting behind a backlog of row frames still yields a
    /// true round-trip (client-side head-of-line blocking otherwise inflates the
    /// RTT and biases the offset on a row-flooded connection).
    frame_tx: mpsc::UnboundedSender<(u64, u64, FrameOutcome)>,
    /// Per-command gateway-call tally for the debug HUD, keyed by wire tag. A
    /// `BTreeMap` so [`Self::emit_call_stats`] snapshots in a stable order.
    calls: BTreeMap<&'static str, CallCounters>,
    /// Per-table subscription-data tally for the debug HUD, keyed by row table tag
    /// (`BTreeMap` for stable snapshot order).
    sub_data: BTreeMap<&'static str, SubCounters>,
    /// Every `sub_zone` frame ever sent — the cumulative "total" gauge (the live
    /// "open" gauge is [`Self::subs`]`.len()`).
    subs_total: u32,
    /// Rolling clock-offset estimate, fed by ping/pong round-trips and the login
    /// seed. Reset to a fresh, unsynced estimate on every disconnect.
    clock: Clock,
    /// The wall↔tic estimate (first-pawns P3) — anchored by every `state`/`event` arrival;
    /// re-anchors surface as [`Event::TicAnchor`].
    tics: crate::ticclock::TicEstimate,
}

impl Engine {
    fn new(
        config: ClientConfig,
        sink: Arc<dyn EventSink>,
        world: crate::client_world::SharedWorld,
    ) -> (Self, mpsc::UnboundedReceiver<(u64, u64, FrameOutcome)>) {
        let (frame_tx, frame_rx) = mpsc::unbounded();
        let engine = Self {
            config,
            world,
            sink,
            next_cid: 1,
            server_url: None,
            write: None,
            meta: None,
            pending: None,
            player_id: None,
            zones: ZoneManager::default(),
            open_zones: HashSet::new(),
            generation: 0,
            frame_tx,
            calls: BTreeMap::new(),
            sub_data: BTreeMap::new(),
            subs_total: 0,
            clock: Clock::new(),
            tics: crate::ticclock::TicEstimate::default(),
        };
        (engine, frame_rx)
    }

    /// The select loop. Runs until the command channel closes or a
    /// [`Command::Shutdown`] arrives, then tears the session down.
    async fn run(
        mut self,
        mut cmd_rx: mpsc::UnboundedReceiver<Command>,
        mut frame_rx: mpsc::UnboundedReceiver<(u64, u64, FrameOutcome)>,
    ) {
        // Clock-sync cadence: a browser interval polled alongside the command and
        // frame streams. It ticks whether or not we're connected; `send_ping`
        // skips the send when there's no socket.
        let mut ping = IntervalStream::new(PING_INTERVAL_MS).fuse();
        loop {
            select! {
                cmd = cmd_rx.next() => match cmd {
                    Some(cmd) => {
                        if self.handle_command(cmd).await {
                            break; // Shutdown
                        }
                    }
                    None => break, // all handles dropped
                },
                frame = frame_rx.next() => {
                    if let Some((generation, recv_ms, outcome)) = frame {
                        // Drop a superseded socket's tail frames.
                        if generation == self.generation {
                            self.handle_frame(recv_ms, outcome).await;
                        }
                    }
                }
                _ = ping.next() => {
                    self.send_ping().await;
                }
            }
        }

        self.disconnect(None, /*emit=*/ false).await;
    }

    /// Process one host command. Returns `true` to stop the engine.
    async fn handle_command(&mut self, cmd: Command) -> bool {
        match cmd {
            Command::Login { name } => self.handle_login(name).await,
            Command::SetAnchor {
                name,
                tile_x,
                tile_y,
                radii,
                soul,
            } => {
                self.handle_set_anchor(name, tile_x, tile_y, radii, soul)
                    .await
            }
            Command::RemoveAnchor { name } => self.handle_remove_anchor(name).await,
            Command::Queue { actions } => self.handle_queue(actions).await,
            Command::SeedTicRate { tics_per_sec } => self.tics.seed_rate(tics_per_sec),
            Command::BuildWall { start_x, start_y, end_x, end_y, object } => {
                self.handle_queue(world::build_wall_program(start_x, start_y, end_x, end_y, object)).await;
            }
            Command::Place { entity, tile_x, tile_y } => {
                self.handle_queue(world::place_program(entity, tile_x, tile_y)).await;
            }
            Command::Logout => {
                self.disconnect(Some("logout".to_string()), /*emit=*/ true)
                    .await;
            }
            Command::Shutdown => return true,
        }
        false
    }

    /// Add or move an anchor and push the resulting subscription changes. Requires
    /// a live session; without one the command is reported and dropped.
    async fn handle_set_anchor(
        &mut self,
        name: String,
        tile_x: i32,
        tile_y: i32,
        radii: AnchorRadii,
        soul: u32,
    ) {
        if self.write.is_none() {
            self.emit(Event::Status(format!(
                "set_anchor '{name}' ignored: not logged in"
            )));
            return;
        }
        self.zones
            .set_anchor(&name, tile_x, tile_y, radii, soul, now_ms());
        self.flush_zone_intents().await;
    }

    /// Remove an anchor and push the resulting subscription changes.
    async fn handle_remove_anchor(&mut self, name: String) {
        self.zones.remove_anchor(&name, now_ms());
        self.flush_zone_intents().await;
    }

    /// Drain the zone manager's intents and translate each into a `subscribe_zone` /
    /// `unsubscribe_zone` frame, keyed by the zone's `macro_position_reference` and tracking the
    /// open set. A closed sub also emits [`Event::ZoneClosed`] so the host drops that zone's sprites.
    async fn flush_zone_intents(&mut self) {
        let mut subs_changed = false;
        for intent in self.zones.take_intents() {
            let zone = intent.macro_position;
            let frame = if intent.on {
                if self.open_zones.insert(zone) {
                    self.subs_total += 1;
                    subs_changed = true;
                }
                ClientMsg::SubscribeZone { zone }
            } else {
                if self.open_zones.remove(&zone) {
                    self.emit(Event::ZoneClosed { macro_position: zone });
                    subs_changed = true;
                } else {
                    continue; // not open on the wire; nothing to drop
                }
                ClientMsg::UnsubscribeZone { zone }
            };
            if let Err(err) = self.send_frame(&frame).await {
                self.emit(Event::Status(format!("subscription send failed: {err}")));
            }
        }
        // The open/total gauge moved — refresh the "subs" HUD once for the batch.
        if subs_changed {
            self.emit_sub_stats();
        }
    }

    /// Queue an action program on the live session (mints a `cid`; the reply is surfaced only on
    /// error in [`Self::handle_server_msg`]).
    async fn handle_queue(&mut self, actions: Vec<u32>) {
        if self.write.is_none() {
            self.emit(Event::Status("queue ignored: not logged in".to_string()));
            return;
        }
        let cid = self.take_cid();
        if let Err(err) = self.send_frame(&ClientMsg::Queue { cid, actions }).await {
            self.emit(Event::Status(format!("queue send failed: {err}")));
        }
    }

    /// Acquire a server, connect, spawn the read pump, and fire off the login
    /// frame. The reply is handled later, in [`Self::handle_server_msg`].
    async fn handle_login(&mut self, name: String) {
        self.emit(Event::LoginStarted { name: name.clone() });

        // A fresh login supersedes any existing connection.
        if self.write.is_some() {
            self.disconnect(Some("reconnecting".to_string()), /*emit=*/ true)
                .await;
        }

        // 1. Gateway: which world server should we use?
        let url = crate::gateway::resolve_url(&self.config.gateway_url, self.player_id);
        let server = match fetch_resolve(&url).await {
            Ok(s) => s,
            Err(reason) => {
                self.emit(Event::LoginFailed { reason });
                return;
            }
        };
        self.emit(Event::ServerResolved(server.clone()));

        // 2. Connect to that world server's WebSocket.
        let ServerInfo { url: ws_url, .. } = server;
        let (meta, wsio) = match WsMeta::connect(&ws_url, None).await {
            Ok(pair) => pair,
            Err(err) => {
                self.emit(Event::LoginFailed {
                    reason: format!("connect to {ws_url} failed: {err}"),
                });
                return;
            }
        };
        let (write, read) = wsio.split();
        self.write = Some(write);
        self.meta = Some(meta);
        self.server_url = Some(ws_url.clone());

        // Stamp this connection's generation and pump its read half.
        self.generation = self.generation.wrapping_add(1);
        wasm_bindgen_futures::spawn_local(read_pump(read, self.generation, self.frame_tx.clone()));

        // 3. Send the login frame; record the pending correlation id.
        let cid = self.take_cid();
        let frame = ClientMsg::Login {
            cid,
            client_time_ms: now_ms(),
            name: name.clone(),
        };
        if let Err(err) = self.send_frame(&frame).await {
            self.emit(Event::LoginFailed {
                reason: format!("sending login failed: {err}"),
            });
            self.disconnect(Some("login send failed".to_string()), /*emit=*/ false)
                .await;
            return;
        }
        self.pending = Some(Pending { cid, name });
    }

    /// Handle one inbound frame outcome from the read pump. `recv_ms` is the
    /// ingress timestamp stamped when the frame left the socket, forwarded to the
    /// clock so a queued Pong is timed by arrival, not by processing.
    async fn handle_frame(&mut self, recv_ms: u64, outcome: FrameOutcome) {
        match outcome {
            FrameOutcome::Text(text) => self.handle_server_msg(&text, recv_ms).await,
            FrameOutcome::Other => {}
            FrameOutcome::Closed => {
                self.disconnect(Some("server closed connection".to_string()), true)
                    .await;
            }
        }
    }

    /// Parse and dispatch a text frame as a [`ServerMsg`]. `recv_ms` is the
    /// frame's ingress timestamp, used to time clock-sync replies.
    async fn handle_server_msg(&mut self, text: &str, recv_ms: u64) {
        let msg: ServerMsg = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(err) => {
                self.emit(Event::Status(format!("unparseable server frame: {err}")));
                return;
            }
        };

        match msg {
            ServerMsg::LoginOk {
                cid,
                player_id,
                player_shard_reference,
                server_micros,
            } => {
                self.record_reply("login", /*ok=*/ true, text.len());
                if !self.matches_pending(cid) {
                    return;
                }
                self.pending = None;
                self.player_id = Some(player_id);
                // Coarse offset seed so the clock is usable before the first pong;
                // a real round-trip refines it within `PING_INTERVAL_MS`. Timed by
                // the frame's ingress, not by when we got round to processing it.
                self.clock.seed_login(server_micros / 1_000, recv_ms);
                self.emit(Event::ClockSync(self.clock.snapshot(now_ms())));
                let server_url = self.server_url.clone().unwrap_or_default();
                self.emit(Event::LoggedIn {
                    player_id,
                    player_shard_reference,
                    server_url,
                });
                // Prime the estimator with a real round-trip immediately (the
                // browser's ping interval otherwise fires a full period later), so
                // the windowed best-RTT filter can adopt a genuine sample over the
                // coarse login seed within a second of connecting.
                self.send_ping().await;
            }
            ServerMsg::Pong {
                client_send_ms,
                server_ms,
            } => {
                // Time the round-trip by the pong's ingress (`recv_ms`), not by
                // now — otherwise a Pong queued behind row frames reads as a long
                // RTT and biases the offset low on a busy connection.
                self.clock.on_pong(client_send_ms, server_ms, recv_ms);
                self.emit(Event::ClockSync(self.clock.snapshot(now_ms())));
            }
            ServerMsg::LoginErr { cid, error, .. } => {
                self.record_reply("login", /*ok=*/ false, text.len());
                if !self.matches_pending(cid) {
                    return;
                }
                self.pending = None;
                self.emit(Event::LoginFailed { reason: error });
            }
            ServerMsg::Error { error } => {
                self.emit(Event::Status(format!("server error: {error}")));
            }
            // A queued intent's outcome. Success is silent (the effect arrives as a `state` row);
            // a rejection is surfaced.
            ServerMsg::QueueOk { .. } => {
                self.record_reply("queue", /*ok=*/ true, text.len());
            }
            ServerMsg::QueueErr { error, .. } => {
                self.record_reply("queue", /*ok=*/ false, text.len());
                self.emit(Event::Status(format!("queue rejected: {error}")));
            }
            // A composed entity changed in a subscribed zone. Decode to a mover; every row's byte
            // length feeds the zone's cost accounting (load vs. retention), which may release a
            // soft-held sub.
            ServerMsg::State(row) => {
                self.record_row("state", text.len());
                if self.tics.observe(row.tic, now_ms() as f64) {
                    let (tic, wall_ms) = self.tics.anchor().unwrap();
                    self.emit(Event::TicAnchor { tic, wall_ms, tics_per_sec: self.tics.tics_per_sec() });
                }
                // Age the zone by its wire macro (the anchor manager keys on it); the render event
                // carries that same macro straight through.
                self.emit(world::state_event(&row, /*removed=*/ false));
                self.zones.note_update(row.zone, text.len() as u64, now_ms());
                self.flush_zone_intents().await;
            }
            // A composed entity left a subscribed zone.
            ServerMsg::StateGone { entity_reference, zone } => {
                self.emit(Event::StateObject {
                    macro_position: zone,
                    entity_reference,
                    definition_reference: 0,
                    tile_x: 0,
                    tile_y: 0,
                    sub_x: 0,
                    sub_y: 0,
                    facing: 0,
                    tic: 0,
                    removed: true,
                });
            }
            // A pawn's payload sidecar row (human-pawns P0) — decode the PART entries; the host
            // joins them to the entity's StateObjects (either may arrive first).
            ServerMsg::Payload { entity_reference, zone, tic, payload } => {
                self.emit(Event::PawnParts {
                    macro_position: zone,
                    entity_reference,
                    tic,
                    parts: resonantdust_codec::payload::payload_parts(&payload),
                    payload,
                });
            }
            // The ownership re-attach lane (npc-host I11) — a mint this player issued.
            ServerMsg::Owned { entity_reference } => {
                self.emit(Event::OwnedPawn { entity_reference });
            }
            // One needs sub-table row (stat-model F2) — a sip lands as exactly this frame.
            ServerMsg::Need { entity_reference, zone, need, set_tic } => {
                self.emit(Event::PawnNeed {
                    macro_position: zone,
                    entity_reference,
                    need,
                    set_tic,
                });
            }
            // One inventory row (inventory F2) — a pick_up/drop lands as exactly this frame.
            ServerMsg::Inventory { entity_reference, zone, slot, item, state } => {
                self.emit(Event::PawnInventory {
                    macro_position: zone,
                    entity_reference,
                    slot,
                    item,
                    state,
                });
            }
            // A settled, promoted event — the INTENT channel. Anchor the tic estimate and
            // surface any movement intents for the host to speculate from.
            ServerMsg::Event { zone, tic, actions, .. } => {
                self.record_row("event", text.len());
                if self.tics.observe(tic, now_ms() as f64) {
                    let (atic, wall_ms) = self.tics.anchor().unwrap();
                    self.emit(Event::TicAnchor { tic: atic, wall_ms, tics_per_sec: self.tics.tics_per_sec() });
                }
                for ev in world::move_intents(zone, tic, &actions) {
                    self.emit(ev);
                }
            }
            // A zone's cold ground / scatter — the terrain.
            ServerMsg::ColdTile { zone, subtype_id, layer_id, tic, tiles } => {
                self.record_row("cold_tile", text.len());
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdTiles { macro_position: zone, subtype_id, layer_id, tic, tiles });
                self.flush_zone_intents().await;
            }
            ServerMsg::ColdThing { zone, subtype_id, layer_id, tic, things } => {
                self.record_row("cold_thing", text.len());
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdThings { macro_position: zone, subtype_id, layer_id, tic, things });
                self.flush_zone_intents().await;
            }
            ServerMsg::ColdState {
                zone,
                entity_reference,
                position_reference,
                definition_reference,
                data,
                tic,
                removed,
            } => {
                // TEMP (logs-drop drill): prove receipt end to end.
                self.emit(Event::Status(format!(
                    "cold_state received: zone {zone} def {definition_reference:#010x} removed {removed}"
                )));
                self.record_row("cold_state", text.len());
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdState {
                    macro_position: zone,
                    entity_reference,
                    position_reference,
                    definition_reference,
                    data,
                    tic,
                    removed,
                });
                self.flush_zone_intents().await;
            }
        }
    }

    /// Send a clock-sync ping, if connected. Silent when there's no socket (the
    /// interval keeps ticking between sessions) and on a transport error (the read
    /// pump surfaces the disconnect). Pings aren't tallied in the "calls" HUD —
    /// they're background chatter, not a host-driven command.
    async fn send_ping(&mut self) {
        if self.write.is_none() {
            return;
        }
        let frame = ClientMsg::Ping {
            client_send_ms: now_ms(),
        };
        let text = match serde_json::to_string(&frame) {
            Ok(t) => t,
            Err(_) => return,
        };
        if let Some(write) = self.write.as_mut() {
            let _ = write.send(WsMessage::Text(text)).await;
        }
    }

    /// Serialize and send a client frame on the live socket. Tallies the frame as a
    /// request (with its serialized byte size) before the send, so the "calls" HUD
    /// reflects the attempt whether or not the socket write succeeds.
    async fn send_frame(&mut self, frame: &ClientMsg) -> Result<(), String> {
        let text = serde_json::to_string(frame).map_err(|e| format!("serialize frame: {e}"))?;
        let entry = self.calls.entry(client_msg_tag(frame)).or_default();
        entry.requests += 1;
        entry.tx += text.len() as u64;
        self.emit_call_stats();
        let write = self
            .write
            .as_mut()
            .ok_or_else(|| "not connected".to_string())?;
        write
            .send(WsMessage::Text(text))
            .await
            .map_err(|e| format!("ws send: {e}"))
    }

    /// Record a correlated reply for command `tag`: a success (`ok`) or failure
    /// (`err`), plus the reply frame's byte size, then re-emit the tally.
    fn record_reply(&mut self, tag: &'static str, ok: bool, rx_bytes: usize) {
        let entry = self.calls.entry(tag).or_default();
        if ok {
            entry.ok += 1;
        } else {
            entry.err += 1;
        }
        entry.rx += rx_bytes as u64;
        self.emit_call_stats();
    }

    /// Snapshot the running tally and emit it as [`Event::CallStats`] for the HUD.
    fn emit_call_stats(&self) {
        let stats = self
            .calls
            .iter()
            .map(|(command, c)| CallStat {
                command: (*command).to_string(),
                requests: c.requests,
                ok: c.ok,
                err: c.err,
                tx: c.tx,
                rx: c.rx,
            })
            .collect();
        self.emit(Event::CallStats(stats));
    }

    /// Record one relayed `Row` frame of table `tag` (with its byte size) and
    /// re-emit the subscription-data tally.
    fn record_row(&mut self, tag: &'static str, rx_bytes: usize) {
        let entry = self.sub_data.entry(tag).or_default();
        entry.rows += 1;
        entry.rx += rx_bytes as u64;
        self.emit_sub_stats();
    }

    /// Snapshot the subscription-data tally (live open + cumulative total gauge and
    /// the per-table breakdown) and emit it as [`Event::SubStats`] for the HUD.
    fn emit_sub_stats(&self) {
        let tables = self
            .sub_data
            .iter()
            .map(|(table, c)| SubStat {
                table: (*table).to_string(),
                rows: c.rows,
                rx: c.rx,
            })
            .collect();
        self.emit(Event::SubStats {
            open: self.open_zones.len() as u32,
            total: self.subs_total,
            tables,
        });
    }

    /// Tear down the live connection (if any) and clear session state. Bumps the
    /// generation first so the old pump's tail frames are ignored. Emits
    /// [`Event::Disconnected`] when `emit` is set and a connection existed.
    async fn disconnect(&mut self, reason: Option<String>, emit: bool) {
        let was_connected = self.write.is_some();
        // Invalidate the current pump: its remaining frames carry the old gen.
        self.generation = self.generation.wrapping_add(1);
        if let Some(mut write) = self.write.take() {
            let _ = write.close().await;
        }
        if let Some(meta) = self.meta.take() {
            let _ = meta.close().await;
        }
        self.server_url = None;
        // Subscriptions die with the socket: drop anchors + the open set so a reconnecting host
        // starts from an empty anchor list.
        let had_subs = !self.open_zones.is_empty();
        self.zones.clear();
        self.open_zones.clear();
        // The clock offset is per-session (a new server, a new clock to sync to).
        self.clock = Clock::new();
        // The live "open" gauge just fell to zero — refresh the "subs" HUD (the
        // per-table byte totals are cumulative and survive the reconnect).
        if had_subs {
            self.emit_sub_stats();
        }
        if let Some(p) = self.pending.take() {
            if was_connected {
                self.emit(Event::LoginFailed {
                    reason: format!("disconnected before '{}' login completed", p.name),
                });
            }
        }
        if emit && was_connected {
            self.emit(Event::Disconnected { reason });
        }
    }

    fn matches_pending(&self, cid: u32) -> bool {
        self.pending.as_ref().is_some_and(|p| p.cid == cid)
    }

    fn take_cid(&mut self) -> u32 {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1);
        cid
    }

    fn emit(&self, event: Event) {
        // THE fold, before the host ever sees it (shared-simulation P2c).
        if let Ok(mut w) = self.world.lock() {
            // ONE clock: the engine's estimator is the only one. It anchors off every wire tic,
            // so the model gets the answer even in a zone that fans no pawn rows — which is
            // exactly where a model-owned estimator went blind (it only ever saw `TicAnchor`).
            let now = now_ms() as f64;
            w.set_tic(self.tics.estimate_at(now).map(|t| t.rem_euclid(65536.0) as u16));
            w.observe_event(&event, now);
        }
        self.sink.emit(event);
    }
}

/// Read frames off the socket's read half and forward them (tagged with the
/// connection `generation`) to the engine loop, until the stream ends. A send
/// failure means the engine is gone — stop.
async fn read_pump(
    mut read: SplitStream<WsStream>,
    generation: u64,
    tx: mpsc::UnboundedSender<(u64, u64, FrameOutcome)>,
) {
    while let Some(msg) = read.next().await {
        let outcome = match msg {
            WsMessage::Text(t) => FrameOutcome::Text(t),
            WsMessage::Binary(_) => FrameOutcome::Other,
        };
        // Stamp arrival here, at the socket edge, so a clock-sync Pong is timed by
        // when it landed — not by when the (possibly backlogged) engine loop gets
        // to it. This is the client-side half of keeping pings off the busy path.
        if tx.unbounded_send((generation, now_ms(), outcome)).is_err() {
            return;
        }
    }
    let _ = tx.unbounded_send((generation, now_ms(), FrameOutcome::Closed));
}

/// The gateway round-trip over `fetch` (`gloo-net`), delegating the parse to the
/// shared [`crate::gateway::parse_resolve`].
async fn fetch_resolve(url: &str) -> Result<ServerInfo, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| format!("gateway request to {url} failed: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("unreadable gateway response: {e}"))?;
    crate::gateway::parse_resolve(status, &body)
}

/// Wall-clock milliseconds since the unix epoch, from the browser clock.
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}
