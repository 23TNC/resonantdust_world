//! npc — the **automated-player** library. An npc drives the very same `client` (client/core)
//! command/event API a human does, so every action travels login → client → edge → spacetime
//! and is subject to the same validation and sync; there is no privileged direct-to-shard path.
//!
//! One npc process is one automated player running ONE [`Brain`] (first-pawns P4): the harness
//! ([`Bot`] + [`run_brain`]) owns login, anchoring, the event pump, and pause tracking; a brain
//! owns only behavior. A wolves-brain wanders wildlife, a future villager-brain runs a
//! village's settlers, a region-steward-brain minds a region in players' absence — each is an
//! impl of the same trait, not a new harness. One container = one brain (`bin/sim run npc`).

use std::time::Duration;

use tokio::sync::mpsc;

use client::{AnchorRadii, Client, ClientConfig, Command, Event};

pub mod brains;

/// Tiny xorshift64 — npc motion is not part of sim determinism, so a wall-clock seed is fine.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

/// A behavior over the harness. `on_start` runs once after login (anchor, spawn); `on_event`
/// sees every world event; `tick` fires on the runner's cadence while the sim isn't paused.
pub trait Brain {
    /// One-time setup after login. The bot is logged in; the zone stream is NOT yet open —
    /// call [`Bot::anchor_and_wait`] here.
    fn on_start(&mut self, bot: &mut Bot) -> impl std::future::Future<Output = ()>;
    /// One world event (already logged by the harness).
    fn on_event(&mut self, bot: &Bot, event: &Event);
    /// Periodic think. Not called while the simulation is paused.
    fn tick(&mut self, bot: &Bot);
}

/// Drive `brain` over `bot` until the client engine ends: drain events (pause tracking +
/// `on_event`), then `tick` every `tick_ms`.
pub async fn run_brain<B: Brain>(mut bot: Bot, mut brain: B, tick_ms: u64) {
    brain.on_start(&mut bot).await;
    let mut ticker = tokio::time::interval(Duration::from_millis(tick_ms));
    loop {
        ticker.tick().await;
        loop {
            match bot.events.try_recv() {
                Ok(event) => {
                    bot.note(&event);
                    log_event(&event);
                    brain.on_event(&bot, &event);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::warn!("client engine ended; stopping brain");
                    return;
                }
            }
        }
        if bot.paused {
            continue;
        }
        brain.tick(&bot);
    }
}

/// The automated-player harness: owns the `client` handle and its event stream, and hides the
/// connect/await/anchor boilerplate every brain needs.
pub struct Bot {
    pub client: Client,
    pub(crate) events: mpsc::UnboundedReceiver<Event>,
    /// The simulation's freeze state, tracked from [`Event::Paused`] (debug `/pause`). While
    /// set, the runner withholds `tick` — the sim isn't advancing, so moves would just queue
    /// behind the freeze.
    pub paused: bool,
    /// The world server this session logged into (from [`Event::LoggedIn`]) — the base URL its
    /// `/content` corpus is fetched from (the def-id authority, first-pawns F5).
    pub server_url: Option<String>,
}

impl Bot {
    /// Spawn a client engine, log in as `name`, and wait for the session to go live. Returns
    /// `None` if login fails (reported to the log).
    pub async fn login(config: ClientConfig, name: &str) -> Option<Bot> {
        let (ev_tx, events) = mpsc::unbounded_channel::<Event>();
        let client = Client::spawn(config, move |event: Event| {
            let _ = ev_tx.send(event);
        });
        let mut bot = Bot { client, events, paused: false, server_url: None };
        if bot.client.login(name).is_err() {
            tracing::error!("client engine failed to start");
            return None;
        }
        while let Some(event) = bot.events.recv().await {
            log_event(&event);
            match event {
                Event::LoggedIn { player_id, ref server_url, .. } => {
                    bot.server_url = Some(server_url.clone());
                    tracing::info!(player_id, %name, "npc logged in");
                    return Some(bot);
                }
                Event::LoginFailed { .. } | Event::Disconnected { .. } => return None,
                _ => {}
            }
        }
        None
    }

    /// Open a viewport anchor at `(tile_x, tile_y)` so the client subscribes the zones around
    /// it — which is what connects that zone's shard on the edge, the prerequisite for any
    /// spawn/move. Then wait until the zone (`macro_position`) is actually streaming so the
    /// first spawn doesn't race the shard connect. Falls back after `timeout` and proceeds.
    pub async fn anchor_and_wait(
        &mut self,
        name: &str,
        tile_x: i32,
        tile_y: i32,
        active_radius: u16,
        macro_position: u16,
        timeout: Duration,
    ) {
        let r = i32::from(active_radius.max(2));
        let _ = self.client.send(Command::SetAnchor {
            name: name.to_string(),
            tile_x,
            tile_y,
            radii: AnchorRadii { active: r, hot: r + 2, warm: r + 4, cold: r + 6 },
            soul: 0,
        });
        tracing::info!(macro_position, "anchored; waiting for the zone to stream");
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(event)) => {
                    let ready = matches!(&event,
                        Event::ColdTiles { macro_position: z, .. }
                            | Event::ColdThings { macro_position: z, .. }
                            | Event::StateObject { macro_position: z, .. }
                            if *z == macro_position);
                    self.note(&event);
                    log_event(&event);
                    if ready {
                        tracing::info!(macro_position, "zone is streaming; shard connected");
                        return;
                    }
                }
                Ok(None) => return, // engine ended
                Err(_) => {
                    tracing::warn!(macro_position, "timed out waiting for the zone; proceeding anyway");
                    return;
                }
            }
        }
    }

    /// Track harness-level state off an event (the pause flag).
    pub(crate) fn note(&mut self, event: &Event) {
        if let Event::Paused { paused } = event {
            if *paused != self.paused {
                tracing::info!(paused, "simulation freeze changed (/pause)");
            }
            self.paused = *paused;
        }
    }
}

/// Resolve a thing's def id from the world server's `/content` corpus — the SAME corpus the
/// browser renders with (first-pawns F5: content is the authority; a pinned `KIND_*` constant
/// drifts the moment `things.rd` reorders). The payload is `{ "rd": [[name, source], …] }`,
/// loaded with the shared DSL and answered by `thing_object_id`.
pub async fn resolve_thing_def(server_url: &str, name: &str) -> Result<u16, String> {
    // The login hands back the WS endpoint (`ws://host:port/ws`); the corpus lives on the same
    // server's HTTP side. Swap the scheme and drop the `/ws` path.
    let base = server_url
        .replacen("ws://", "http://", 1)
        .replacen("wss://", "https://", 1);
    let base = base.trim_end_matches('/').trim_end_matches("/ws").trim_end_matches('/');
    let url = format!("{base}/content");
    let body: serde_json::Value = reqwest::get(&url)
        .await
        .map_err(|e| format!("GET {url}: {e}"))?
        .json()
        .await
        .map_err(|e| format!("{url}: bad JSON: {e}"))?;
    let rd = body["rd"].as_array().ok_or_else(|| format!("{url}: no `rd` array"))?;
    let sources: Vec<(String, String)> = rd
        .iter()
        .filter_map(|pair| {
            Some((pair.get(0)?.as_str()?.to_string(), pair.get(1)?.as_str()?.to_string()))
        })
        .collect();
    let bundle = resonantdust_dsl::loader::load(&sources)
        .map_err(|errs| format!("corpus load: {} error(s), first: {:?}", errs.len(), errs.first()))?;
    bundle
        .thing_object_id(name)
        .ok_or_else(|| format!("thing `{name}` not in the corpus"))
}

/// Log one client event at an appropriate level (concise — an npc mostly cares about the
/// login/disconnect lifecycle + movement; row traffic is debug).
pub fn log_event(event: &Event) {
    match event {
        Event::LoginStarted { name } => tracing::info!(%name, "login started"),
        Event::ServerResolved(s) => tracing::info!(server_id = s.server_id, url = %s.url, "server resolved"),
        Event::LoggedIn { player_id, .. } => tracing::info!(player_id, "logged in"),
        Event::LoginFailed { reason } => tracing::error!(%reason, "login failed"),
        Event::Disconnected { reason } => tracing::warn!(?reason, "disconnected"),
        Event::Status(msg) => tracing::debug!(%msg, "status"),
        Event::StateObject { macro_position: zone, entity_reference, tile_x, tile_y, tic, .. } => {
            tracing::debug!(zone, entity_reference, tile_x, tile_y, tic, "state object")
        }
        Event::ColdTiles { macro_position: zone, .. } => tracing::debug!(zone, "cold tiles"),
        Event::ColdThings { macro_position: zone, .. } => tracing::debug!(zone, "cold things"),
        Event::ZoneClosed { macro_position: zone } => tracing::debug!(zone, "zone closed"),
        Event::Paused { paused } => tracing::debug!(paused, "paused"),
        Event::TicAnchor { tic, wall_ms } => tracing::debug!(tic, wall_ms, "tic re-anchor"),
        Event::MoveIntent { macro_position: zone, entity_reference, tile_x, tile_y, event_tic } => {
            tracing::info!(zone, entity_reference, tile_x, tile_y, event_tic, "move intent")
        }
        Event::ColdState { .. } | Event::CallStats(_) | Event::SubStats { .. } | Event::ClockSync(_) => {}
    }
}
