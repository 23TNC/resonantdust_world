//! The native engine — the tokio-backed runtime behind the host-facing
//! [`Client`] handle.
//!
//! [`Client::spawn`] launches one [`Engine`] task. That task is the client's
//! single owner of mutable session state, so there are no locks: it `select!`s
//! between two inputs —
//!   * the **command channel** ([`Command`]s the host sends via the handle), and
//!   * the **world-server WS read stream** (frames the server pushes) —
//! and funnels everything outward as [`Event`]s through the host's [`EventSink`].
//!
//! Login is intentionally split across the loop: [`Command::Login`] does the
//! blocking-ish setup (gateway HTTP, WS connect, send the login frame) inline,
//! then *returns to the loop* with a pending correlation id; the matching
//! `login_ok` / `login_err` is handled when it arrives on the read stream. That
//! keeps the loop responsive and puts all reply handling in one place.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::api::{Command, Event, EventSink, ServerInfo};
use crate::clock::Clock;
use crate::config::ClientConfig;
use crate::protocol::{ClientMsg, ServerMsg};
use crate::world;
use crate::zones::{AnchorRadii, ZoneManager};

/// How often to send a clock-sync ping while connected. Fast enough that a fresh
/// session converges on a good offset within a few seconds, cheap enough to run
/// for the whole session (one tiny frame each way).
const PING_INTERVAL: Duration = Duration::from_secs(2);

/// The live world-server socket, split into its write and read halves. The read
/// half lives in the loop (so `select!` can poll it); the write half lives on the
/// engine (only command handling sends).
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWrite = SplitSink<WsStream, Message>;
type WsRead = SplitStream<WsStream>;

/// Returned by [`Client::send`] when the engine task is already gone (the client
/// was shut down or dropped). The unsent command is handed back.
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
    /// Spawn a client engine on the current tokio runtime and return its handle.
    /// `sink` receives every [`Event`] the client emits (a closure is the usual
    /// choice — see [`EventSink`]).
    ///
    /// Must be called from within a tokio runtime (e.g. under `#[tokio::main]`):
    /// it uses [`tokio::spawn`].
    pub fn spawn(config: ClientConfig, sink: impl EventSink) -> Client {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let engine = Engine::new(config, Arc::new(sink));
        tokio::spawn(engine.run(cmd_rx));
        Client { cmd_tx }
    }

    /// Send a command to the engine. Errors only if the engine has stopped.
    pub fn send(&self, cmd: Command) -> Result<(), SendError> {
        self.cmd_tx.send(cmd).map_err(|e| SendError(e.0))
    }

    /// Convenience: log in as `name` (acquire a server, connect, authenticate).
    pub fn login(&self, name: impl Into<String>) -> Result<(), SendError> {
        self.send(Command::Login { name: name.into() })
    }

    /// Convenience: add or move the anchor named `name` (see
    /// [`Command::SetAnchor`]). `tile_x`/`tile_y` are global tile coordinates;
    /// `radii` are the per-tier reach in tiles; `soul` is `0` for a viewport.
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

    /// Convenience: remove the anchor named `name` (see [`Command::RemoveAnchor`]).
    pub fn remove_anchor(&self, name: impl Into<String>) -> Result<(), SendError> {
        self.send(Command::RemoveAnchor { name: name.into() })
    }

    /// Convenience: queue a raw action program (see [`Command::Queue`]).
    pub fn queue(&self, actions: Vec<u32>) -> Result<(), SendError> {
        self.send(Command::Queue { actions })
    }

    /// Convenience: move `entity` toward global tile `(tile_x, tile_y)` (see [`Command::Move`]).
    pub fn move_entity(&self, entity: u32, tile_x: i32, tile_y: i32) -> Result<(), SendError> {
        self.send(Command::Move { entity, tile_x, tile_y })
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

/// A login awaiting its server reply: the `cid` we stamped and the name we sent,
/// so an arriving `login_ok` / `login_err` can be matched and reported.
struct Pending {
    cid: u32,
    name: String,
}

/// The engine's owned state.
struct Engine {
    config: ClientConfig,
    sink: Arc<dyn EventSink>,
    /// Monotonic source of login correlation ids.
    next_cid: u32,
    /// The connected world server's WS url, retained so [`Event::LoggedIn`] can
    /// report which server the session lives on.
    server_url: Option<String>,
    /// Write half of the live socket, if connected.
    write: Option<WsWrite>,
    /// The in-flight login, if one is awaiting its reply.
    pending: Option<Pending>,
    /// The established player id, set on login. Carried back to the gateway on a
    /// subsequent login for reconnect affinity.
    player_id: Option<u32>,
    /// The anchor-driven zone subscription manager. Decides which zones to keep open; the engine
    /// maps its `macro_position` intents straight to `subscribe_zone` / `unsubscribe_zone` frames.
    zones: ZoneManager,
    /// Rolling clock-offset estimate, fed by ping/pong round-trips and the login
    /// seed. Reset (to a fresh, unsynced estimate) on every disconnect.
    clock: Clock,
}

impl Engine {
    fn new(config: ClientConfig, sink: Arc<dyn EventSink>) -> Self {
        Self {
            config,
            sink,
            next_cid: 1,
            server_url: None,
            write: None,
            pending: None,
            player_id: None,
            zones: ZoneManager::default(),
            clock: Clock::new(),
        }
    }

    /// The select loop. Runs until the command channel closes or a
    /// [`Command::Shutdown`] arrives, then tears the session down.
    async fn run(mut self, mut cmd_rx: mpsc::UnboundedReceiver<Command>) {
        // The read half lives here, not on `self`: `select!` polls it each
        // iteration while command handling (which may replace it) borrows `self`.
        let mut read: Option<WsRead> = None;
        // Clock-sync cadence. The first tick fires immediately (a ping right after
        // login); subsequent ticks pace the session. Skipped while disconnected.
        let mut ping = tokio::time::interval(PING_INTERVAL);

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => match cmd {
                    Some(cmd) => {
                        if self.handle_command(cmd, &mut read).await {
                            break; // Shutdown
                        }
                    }
                    None => break, // all handles dropped
                },
                frame = next_frame(&mut read) => {
                    self.handle_frame(frame, &mut read).await;
                }
                _ = ping.tick() => {
                    self.send_ping().await;
                }
            }
        }

        self.disconnect(&mut read, None, /*emit=*/ false).await;
        tracing::debug!("client engine stopped");
    }

    /// Process one host command. Returns `true` to stop the engine.
    async fn handle_command(&mut self, cmd: Command, read: &mut Option<WsRead>) -> bool {
        match cmd {
            Command::Login { name } => self.handle_login(name, read).await,
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
            Command::Move { entity, tile_x, tile_y } => {
                self.handle_queue(world::move_to_program(entity, tile_x, tile_y)).await;
            }
            Command::Place { entity, tile_x, tile_y } => {
                self.handle_queue(world::place_program(entity, tile_x, tile_y)).await;
            }
            Command::Logout => {
                self.disconnect(read, Some("logout".to_string()), /*emit=*/ true)
                    .await;
            }
            Command::Shutdown => return true,
        }
        false
    }

    /// Add or move an anchor and push the resulting subscription changes. A live
    /// session is required (subscriptions ride the world-server socket); without
    /// one the command is reported and dropped, so a host sets anchors after
    /// [`Event::LoggedIn`].
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

    /// Drain the zone manager's pending intents and translate each into a `subscribe_zone` /
    /// `unsubscribe_zone` frame, keyed by the zone's `macro_position_reference`. Only runs
    /// meaningfully while connected; a send failure is surfaced and skipped.
    async fn flush_zone_intents(&mut self) {
        for intent in self.zones.take_intents() {
            let zone = intent.macro_position;
            let frame = if intent.on {
                ClientMsg::SubscribeZone { zone }
            } else {
                self.emit(Event::ZoneClosed { macro_position: zone });
                ClientMsg::UnsubscribeZone { zone }
            };
            if let Err(err) = self.send_frame(&frame).await {
                self.emit(Event::Status(format!("subscription send failed: {err}")));
            }
        }
    }

    /// Queue an action program on the live session. Mints a `cid`; the `QueueOk`/`QueueErr` reply is
    /// surfaced (only errors, to keep the stream quiet) in [`Self::handle_server_msg`].
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

    /// Acquire a server, connect, and fire off the login frame. The reply is
    /// handled later, in [`Self::handle_frame`].
    async fn handle_login(&mut self, name: String, read: &mut Option<WsRead>) {
        self.emit(Event::LoginStarted { name: name.clone() });

        // A fresh login supersedes any existing connection.
        if self.write.is_some() {
            self.disconnect(read, Some("reconnecting".to_string()), /*emit=*/ true)
                .await;
        }

        // 1. Gateway: which world server should we use?
        let server = match crate::gateway::resolve_server(&self.config.gateway_url, self.player_id)
            .await
        {
            Ok(s) => s,
            Err(reason) => {
                self.emit(Event::LoginFailed { reason });
                return;
            }
        };
        self.emit(Event::ServerResolved(server.clone()));

        // 2. Connect to that world server's WebSocket.
        let ServerInfo { url, .. } = server;
        let socket = match connect_async(url.as_str()).await {
            Ok((socket, _resp)) => socket,
            Err(err) => {
                self.emit(Event::LoginFailed {
                    reason: format!("connect to {url} failed: {err}"),
                });
                return;
            }
        };
        let (write, new_read) = socket.split();
        self.write = Some(write);
        *read = Some(new_read);
        self.server_url = Some(url.clone());

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
            self.disconnect(read, Some("login send failed".to_string()), /*emit=*/ false)
                .await;
            return;
        }
        self.pending = Some(Pending { cid, name });
    }

    /// Handle one inbound frame outcome from the read stream.
    async fn handle_frame(&mut self, frame: FrameOutcome, read: &mut Option<WsRead>) {
        match frame {
            FrameOutcome::Text(text) => self.handle_server_msg(&text).await,
            // Control + non-text frames carry no protocol payload.
            FrameOutcome::Other => {}
            FrameOutcome::Closed => {
                self.disconnect(read, Some("server closed connection".to_string()), true)
                    .await;
            }
            FrameOutcome::Err(err) => {
                self.disconnect(read, Some(format!("connection error: {err}")), true)
                    .await;
            }
        }
    }

    /// Parse and dispatch a text frame as a [`ServerMsg`].
    async fn handle_server_msg(&mut self, text: &str) {
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
                if !self.matches_pending(cid) {
                    return;
                }
                self.pending = None;
                self.player_id = Some(player_id);
                // Coarse offset seed so the clock is usable before the first pong;
                // a real round-trip refines it within `PING_INTERVAL`.
                self.clock.seed_login(server_micros / 1_000, now_ms());
                self.emit(Event::ClockSync(self.clock.snapshot(now_ms())));
                let server_url = self.server_url.clone().unwrap_or_default();
                self.emit(Event::LoggedIn {
                    player_id,
                    player_shard_reference,
                    server_url,
                });
                // Prime the estimator with a real round-trip immediately, rather
                // than waiting a full ping interval — the windowed best-RTT filter
                // then has a genuine sample to adopt over the coarse login seed.
                self.send_ping().await;
            }
            ServerMsg::Pong {
                client_send_ms,
                server_ms,
            } => {
                self.clock.on_pong(client_send_ms, server_ms, now_ms());
                self.emit(Event::ClockSync(self.clock.snapshot(now_ms())));
            }
            ServerMsg::LoginErr { cid, error, .. } => {
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
            ServerMsg::QueueOk { .. } => {}
            ServerMsg::QueueErr { error, .. } => {
                self.emit(Event::Status(format!("queue rejected: {error}")));
            }
            // A composed entity changed in a subscribed zone. Decode to a mover and age the zone's
            // subscription (warmth), which may evict it — flush the resulting intents.
            ServerMsg::State(row) => {
                // Age the zone's subscription warmth (the anchor manager keys on the wire macro),
                // then emit — the render event carries that same macro straight through.
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
                    facing: 0,
                    tic: 0,
                    removed: true,
                });
            }
            // Settled, promoted events aren't rendered as movers yet — a later feature. Ignore.
            ServerMsg::Event { .. } => {}
            // A zone's cold ground / scatter — the terrain. Age the zone's cost, then emit.
            ServerMsg::ColdTile { zone, subtype_id, layer_id, tiles } => {
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdTiles { macro_position: zone, subtype_id, layer_id, tiles });
                self.flush_zone_intents().await;
            }
            ServerMsg::ColdThing { zone, subtype_id, layer_id, things } => {
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdThings { macro_position: zone, subtype_id, layer_id, things });
                self.flush_zone_intents().await;
            }
            ServerMsg::ColdState {
                zone,
                entity_reference,
                position_reference,
                definition_reference,
                data,
                removed,
            } => {
                self.zones.note_update(zone, text.len() as u64, now_ms());
                self.emit(Event::ColdState {
                    macro_position: zone,
                    entity_reference,
                    position_reference,
                    definition_reference,
                    data,
                    removed,
                });
                self.flush_zone_intents().await;
            }
        }
    }

    /// Send a clock-sync ping, if connected. Silent when there's no socket (the
    /// interval keeps ticking between sessions) and on a transport error (the read
    /// side surfaces the disconnect).
    async fn send_ping(&mut self) {
        if self.write.is_none() {
            return;
        }
        let _ = self
            .send_frame(&ClientMsg::Ping {
                client_send_ms: now_ms(),
            })
            .await;
    }

    /// Serialize and send a client frame on the live socket.
    async fn send_frame(&mut self, frame: &ClientMsg) -> Result<(), String> {
        let text = serde_json::to_string(frame).map_err(|e| format!("serialize frame: {e}"))?;
        let write = self
            .write
            .as_mut()
            .ok_or_else(|| "not connected".to_string())?;
        write
            .send(Message::Text(text))
            .await
            .map_err(|e| format!("ws send: {e}"))
    }

    /// Tear down the live connection (if any) and clear session state. Emits
    /// [`Event::Disconnected`] when `emit` is set and a connection actually
    /// existed — suppressed for internal teardown that already reported a failure,
    /// and on engine shutdown.
    async fn disconnect(
        &mut self,
        read: &mut Option<WsRead>,
        reason: Option<String>,
        emit: bool,
    ) {
        let was_connected = self.write.is_some();
        if let Some(mut write) = self.write.take() {
            // Best-effort close; the socket is being dropped regardless.
            let _ = write.close().await;
        }
        *read = None;
        self.server_url = None;
        // Subscriptions die with the socket: drop anchors so a reconnecting host starts from an
        // empty anchor list (no stale subs against a fresh server session).
        self.zones.clear();
        // The clock offset is per-session (a new server, a new clock to sync to).
        self.clock = Clock::new();
        // A pending login that never got its reply dies with the connection.
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

    /// Does `cid` match the in-flight login? A mismatch is a stale/duplicate reply
    /// and is ignored.
    fn matches_pending(&self, cid: u32) -> bool {
        self.pending.as_ref().is_some_and(|p| p.cid == cid)
    }

    fn take_cid(&mut self) -> u32 {
        let cid = self.next_cid;
        self.next_cid = self.next_cid.wrapping_add(1);
        cid
    }

    fn emit(&self, event: Event) {
        self.sink.emit(event);
    }
}

/// What the read stream yielded, normalized so [`Engine::handle_frame`] doesn't
/// touch the tungstenite types directly.
enum FrameOutcome {
    /// A text frame (the only kind the protocol uses).
    Text(String),
    /// A non-text or control frame — ignored.
    Other,
    /// The stream ended (clean close or EOF).
    Closed,
    /// A transport error.
    Err(String),
}

/// Await the next frame from the read half. When there's no connection this is a
/// future that never resolves, so `select!` simply waits on the command channel.
async fn next_frame(read: &mut Option<WsRead>) -> FrameOutcome {
    match read {
        Some(stream) => match stream.next().await {
            Some(Ok(Message::Text(t))) => FrameOutcome::Text(t),
            Some(Ok(Message::Close(_))) => FrameOutcome::Closed,
            Some(Ok(_)) => FrameOutcome::Other,
            Some(Err(e)) => FrameOutcome::Err(e.to_string()),
            None => FrameOutcome::Closed,
        },
        None => std::future::pending().await,
    }
}

/// Wall-clock milliseconds since the unix epoch — the client clock sample sent
/// with login (the server stamps authoritative time itself).
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
