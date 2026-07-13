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

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use futures::channel::mpsc;
use futures::stream::{SplitSink, SplitStream};
use futures::{select, SinkExt, StreamExt};
use gloo_timers::future::IntervalStream;
use ws_stream_wasm::{WsMessage, WsMeta, WsStream};

use crate::api::{CallStat, Command, Event, EventSink, ServerInfo, SubStat};
use crate::clock::Clock;
use crate::config::ClientConfig;
use crate::protocol::{ClientMsg, RowData, RowOp, ServerMsg};
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
}

impl Client {
    /// Spawn a client engine on the current thread's microtask executor (via
    /// [`spawn_local`](wasm_bindgen_futures::spawn_local)) and return its handle.
    /// `sink` receives every [`Event`] the client emits.
    pub fn spawn(config: ClientConfig, sink: impl EventSink) -> Client {
        let (cmd_tx, cmd_rx) = mpsc::unbounded();
        let (engine, frame_rx) = Engine::new(config, Arc::new(sink));
        wasm_bindgen_futures::spawn_local(engine.run(cmd_rx, frame_rx));
        Client { cmd_tx }
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
        surface: u8,
        radii: AnchorRadii,
        soul: u32,
    ) -> Result<(), SendError> {
        self.send(Command::SetAnchor {
            name: name.into(),
            tile_x,
            tile_y,
            surface,
            radii,
            soul,
        })
    }

    /// Convenience: remove the anchor named `name`.
    pub fn remove_anchor(&self, name: impl Into<String>) -> Result<(), SendError> {
        self.send(Command::RemoveAnchor { name: name.into() })
    }

    /// Convenience: move the controllable thing toward global tile `(tile_x, tile_y)`.
    pub fn move_to(&self, tile_x: i32, tile_y: i32) -> Result<(), SendError> {
        self.send(Command::Move { tile_x, tile_y })
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
        ClientMsg::SubZone { .. } => "sub_zone",
        ClientMsg::Unsub { .. } => "unsub",
        ClientMsg::Ping { .. } => "ping",
        ClientMsg::Move { .. } => "move",
    }
}

/// Running data counters for one relayed-row table, feeding the debug HUD's "subs"
/// tab: how many `Row` frames of this table arrived and their total bytes.
#[derive(Default)]
struct SubCounters {
    rows: u32,
    rx: u64,
}

/// The wire tag of a relayed row — the key its data tally accrues under, matching
/// the server's `#[serde(tag = "table", rename_all = "snake_case")]` discriminator.
fn row_table_tag(row: &RowData) -> &'static str {
    match row {
        RowData::State(_) => "state",
        RowData::ColdObjects(_) => "cold_objects",
    }
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
    next_cid: u32,
    server_url: Option<String>,
    /// Write half of the live socket, if connected.
    write: Option<WsSink>,
    /// The socket's control handle, retained so [`Self::disconnect`] can close it.
    meta: Option<WsMeta>,
    pending: Option<Pending>,
    player_id: Option<u32>,
    zones: ZoneManager,
    subs: HashMap<u32, u32>,
    next_sid: u32,
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
}

impl Engine {
    fn new(
        config: ClientConfig,
        sink: Arc<dyn EventSink>,
    ) -> (Self, mpsc::UnboundedReceiver<(u64, u64, FrameOutcome)>) {
        let (frame_tx, frame_rx) = mpsc::unbounded();
        let engine = Self {
            config,
            sink,
            next_cid: 1,
            server_url: None,
            write: None,
            meta: None,
            pending: None,
            player_id: None,
            zones: ZoneManager::default(),
            subs: HashMap::new(),
            next_sid: 1,
            generation: 0,
            frame_tx,
            calls: BTreeMap::new(),
            sub_data: BTreeMap::new(),
            subs_total: 0,
            clock: Clock::new(),
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
                surface,
                radii,
                soul,
            } => {
                self.handle_set_anchor(name, tile_x, tile_y, surface, radii, soul)
                    .await
            }
            Command::RemoveAnchor { name } => self.handle_remove_anchor(name).await,
            Command::Move { tile_x, tile_y } => {
                if let Err(err) = self.send_frame(&ClientMsg::Move { tile_x, tile_y }).await {
                    self.emit(Event::Status(format!("move send failed: {err}")));
                }
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
        surface: u8,
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
            .set_anchor(&name, tile_x, tile_y, surface, radii, soul, now_ms());
        self.flush_zone_intents().await;
    }

    /// Remove an anchor and push the resulting subscription changes.
    async fn handle_remove_anchor(&mut self, name: String) {
        self.zones.remove_anchor(&name, now_ms());
        self.flush_zone_intents().await;
    }

    /// Drain the zone manager's intents and translate each into a `sub_zone` /
    /// `unsub` frame, maintaining the `zone_id` → `sid` map. A closed sub also
    /// emits [`Event::ZoneClosed`] so the host drops that zone's sprites.
    async fn flush_zone_intents(&mut self) {
        let mut subs_changed = false;
        for intent in self.zones.take_intents() {
            let frame = if intent.on {
                let sid = self.take_sid();
                self.subs.insert(intent.zone_id, sid);
                self.subs_total += 1;
                subs_changed = true;
                ClientMsg::SubZone {
                    sid,
                    zone_id: intent.zone_id,
                }
            } else {
                match self.subs.remove(&intent.zone_id) {
                    Some(sid) => {
                        self.emit(Event::ZoneClosed {
                            zone_id: intent.zone_id,
                        });
                        subs_changed = true;
                        ClientMsg::Unsub { sid }
                    }
                    None => continue, // not open on the wire; nothing to drop
                }
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
                data_shard,
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
                    data_shard,
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
            ServerMsg::Applied { sid } => {
                self.record_reply("sub_zone", /*ok=*/ true, text.len());
                self.emit(Event::Status(format!("subscription {sid} applied")));
            }
            // One upstream row change. `sid` is 0 (the shard connection
            // multiplexes every zone); route by the row's own `zone_id`. A cold
            // baseline is surfaced to the host as tiles to paint; every row's byte
            // length also feeds the zone's cost accounting (load vs. retention), which
            // may release a soft-held sub.
            ServerMsg::Row { row, op, .. } => {
                let zone_id = row_zone_id(&row);
                self.record_row(row_table_tag(&row), text.len());
                match &row {
                    RowData::State(sr) => {
                        use resonantdust_codec::refs;
                        let (obj_type, object_id) = if !refs::entity_ref_is_positional(sr.entity_key) {
                            (refs::entity_ref_type(sr.entity_key), refs::entity_ref_id(sr.entity_key) as u64)
                        } else {
                            (0, 0)
                        };
                        self.emit(Event::StateObject {
                            zone_id: sr.zone_id,
                            obj_type,
                            object_id,
                            kind: sr.kind,
                            tic: sr.tic,
                            location: sr.location,
                            rotation: sr.rotation,
                            offset: sr.offset,
                            removed: matches!(op, RowOp::Delete),
                        });
                    }
                    RowData::ColdObjects(z) => {
                        self.emit(Event::ColdObjects {
                            zone_id: z.zone_id,
                            type_reference: z.type_reference,
                            kinds: z.kinds.clone(),
                        });
                    }
                }
                self.zones.note_update(zone_id, text.len() as u64, now_ms());
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
            open: self.subs.len() as u32,
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
        // Subscriptions die with the socket: drop anchors + the sid map so a
        // reconnecting host starts from an empty anchor list.
        let had_subs = !self.subs.is_empty();
        self.zones.clear();
        self.subs.clear();
        self.next_sid = 1;
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

    fn take_sid(&mut self) -> u32 {
        let sid = self.next_sid;
        self.next_sid = self.next_sid.wrapping_add(1);
        sid
    }

    fn emit(&self, event: Event) {
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

/// The `zone_id` a relayed row belongs to, whichever table it came from.
fn row_zone_id(row: &RowData) -> u32 {
    match row {
        RowData::State(r) => r.zone_id,
        RowData::ColdObjects(r) => r.zone_id,
    }
}

/// Wall-clock milliseconds since the unix epoch, from the browser clock.
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}
