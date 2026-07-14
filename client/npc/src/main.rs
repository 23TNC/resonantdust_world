//! npc — an **automated player**. The headless, scripted counterpart of the pixijs
//! browser client: it drives the very same `client` (client/core) command/event API a
//! human does, so every action travels login → client → edge → spacetime and is subject
//! to the same validation and sync. There is no privileged direct-to-shard path; an npc is
//! just a player whose intents come from code instead of a UI.
//!
//! One npc process is one automated player. A **wildlife** npc manages a pack of wolves; a
//! future **village** npc would manage its settlers — each is a player session with its own
//! behavior. This binary is the first (wildlife). The connect/await/anchor bones live in
//! [`Bot`] (the automated-player *harness*); the wolf behavior is [`wildlife`]. When a second
//! npc type arrives, [`Bot`] extracts into a shared `npc` lib and the behaviors become thin
//! bins on top — the seam is already drawn here so that split is a file move, not a rewrite.
//!
//! Lifecycle (phase 1–3): log in → anchor zone (0,0) so its shard connects → **spawn** 4
//! wolves through the event pipeline (`Command::Spawn` → `ACTION_SPAWN`, born hot, drawn as
//! debug circles by the pixijs mover layer) → on a timer **move** each wolf to a random cell
//! (`Command::Move` with the wolf's key). Spawn and move share one event path.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use client::{AnchorRadii, Client, ClientConfig, Command, Event};
use resonantdust_codec::refs::{pack_minted_entity, ENTITY_TYPE_PAWN, SERVER_REF_NONE};

/// The zone the pack lives in — zone (0,0), `zone_id == 0`.
const ZONE: u32 = 0;
/// Cells per zone edge (16×16 = 256 locations).
const ZONE_DIM: u32 = 16;
/// The wolf's `kind` — its `object_id` in the content thing corpus (append order in
/// `content/data/things.rd`: tree,shrub,cactus,reed,rock,flora,**wolf** = 7). The client
/// resolves this `kind` to a sprite later; for now the mover layer draws every pawn as a
/// debug circle, so the exact value only needs to be non-zero (a live, non-tombstone entity).
const KIND_WOLF: u16 = 7;
/// The first pawns: a small pack in zone (0,0). Kept tiny + single-zone so the whole pack is
/// on screen near the origin. Forest-aware, multi-zone spawning is a later expansion.
const WOLF_COUNT: u32 = 4;

/// Tiny xorshift64 — the pack's motion is not part of sim determinism, so a wall-clock seed
/// is fine (no dependency needed).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u32) -> u32 {
        (self.next() % n as u64) as u32
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// The stable key an npc addresses a wolf by. Wolves are pawns, self-minted (no shard
/// round-trip): `ENTITY_TYPE_PAWN` tags them so the client renders them as pawn markers, the
/// index `i` is the id, and `SERVER_REF_NONE` marks "not shard-minted". The npc keeps these
/// keys so it can move each wolf later.
fn wolf_key(index: u32) -> u64 {
    pack_minted_entity(ENTITY_TYPE_PAWN, index, SERVER_REF_NONE)
}

/// The automated-player harness: owns the `client` handle and its event stream, and hides the
/// connect/await/anchor boilerplate every npc behavior needs. (This is the piece that becomes
/// the shared `npc` lib once a second behavior exists.)
struct Bot {
    client: Client,
    events: mpsc::UnboundedReceiver<Event>,
    /// The simulation's freeze state, tracked from [`Event::Paused`] (debug `/pause`). While
    /// set, the driver stops issuing commands — the sim isn't advancing, so there's nothing to
    /// react to and its moves would just queue behind the freeze.
    paused: bool,
}

impl Bot {
    /// Spawn a client engine, log in as `name`, and wait for the session to go live. Returns
    /// `None` if login fails (reported to the log).
    async fn login(config: ClientConfig, name: &str) -> Option<Bot> {
        let (ev_tx, events) = mpsc::unbounded_channel::<Event>();
        let client = Client::spawn(config, move |event: Event| {
            let _ = ev_tx.send(event);
        });
        let mut bot = Bot { client, events, paused: false };
        if bot.client.login(name).is_err() {
            tracing::error!("client engine failed to start");
            return None;
        }
        // Wait for LoggedIn (or a terminal failure), logging events as they pass.
        while let Some(event) = bot.events.recv().await {
            log_event(&event);
            match event {
                Event::LoggedIn { player_id, .. } => {
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
    /// spawn/move (they append events on the shard connection). Then wait until the zone is
    /// actually streaming (its cold seed / first state row arrives) so the first spawn doesn't
    /// race the shard connect. Falls back after `timeout` and proceeds regardless.
    async fn anchor_and_wait(&mut self, tile_x: i32, tile_y: i32, zone: u32, timeout: Duration) {
        let _ = self.client.send(Command::SetAnchor {
            name: "wildlife".to_string(),
            tile_x,
            tile_y,
            surface: 0,
            radii: AnchorRadii { active: 2, hot: 4, warm: 6, cold: 8 },
            soul: 0,
        });
        tracing::info!(zone, "anchored; waiting for the zone to stream");
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match tokio::time::timeout_at(deadline, self.events.recv()).await {
                Ok(Some(event)) => {
                    let ready = matches!(&event,
                        Event::ColdObjects { zone_id, .. } | Event::StateObject { zone_id, .. }
                            if *zone_id == zone);
                    log_event(&event);
                    if ready {
                        tracing::info!(zone, "zone is streaming; shard connected");
                        return;
                    }
                }
                Ok(None) => return, // engine ended
                Err(_) => {
                    tracing::warn!(zone, "timed out waiting for the zone; proceeding anyway");
                    return;
                }
            }
        }
    }

    /// Drain any pending events (keeps the channel from growing and surfaces disconnects),
    /// returning `false` if the engine has ended.
    fn pump(&mut self) -> bool {
        loop {
            match self.events.try_recv() {
                Ok(event) => {
                    if let Event::Paused { paused } = event {
                        if paused != self.paused {
                            tracing::info!(paused, "simulation freeze changed (/pause)");
                        }
                        self.paused = paused;
                    }
                    log_event(&event);
                }
                Err(mpsc::error::TryRecvError::Empty) => return true,
                Err(mpsc::error::TryRecvError::Disconnected) => return false,
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc=info,client=info".into()),
        )
        .init();

    // arg1 selects the behavior (default `wolves`); the login name is the automated player's
    // account (env `NPC_NAME`, default `Wildlife`). The gateway comes from the standard client
    // env (`CLIENT_GATEWAY_URL` / `RD_GATEWAY`) via `ClientConfig::from_env`.
    let behavior = std::env::args().nth(1).unwrap_or_else(|| "wolves".into());
    let name = env_or("NPC_NAME", "Wildlife");
    let config = ClientConfig::from_env();
    tracing::info!(%behavior, %name, gateway = %config.gateway_url, "npc starting");

    let Some(bot) = Bot::login(config, &name).await else {
        tracing::error!("npc login failed; exiting");
        return;
    };

    let mut rng = Rng(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1);

    match behavior.as_str() {
        // Only wildlife for now; other automated-player behaviors (village, …) land as their
        // own bins over the shared harness once it's extracted.
        _ => wildlife(bot, &mut rng).await,
    }
}

/// The wildlife behavior: spawn a wolf pack in zone (0,0), then wander it cell-by-cell.
///
/// Every action is an ordinary player command. Spawn (phase 1) mints each wolf through the
/// event pipeline (`Command::Spawn` → `ACTION_SPAWN`); move (phase 3) relocates a specific
/// wolf by its key (`Command::Move { pawn: Some(key) }`). The pixijs mover layer draws each
/// pawn as a debug circle, so a browser logged into the same world sees the pack spawn and
/// move — the phase-4 end-to-end check.
async fn wildlife(mut bot: Bot, rng: &mut Rng) {
    let move_ms: u64 = env_or("MOVE_MS", "1000").parse().unwrap_or(1000);

    // Anchor at the zone's origin corner so its shard connects and streams before we spawn.
    bot.anchor_and_wait(0, 0, ZONE, Duration::from_secs(5)).await;

    // Spawn the pack near the origin (cells x:0..9, y:0..6) so the whole pack is on screen at
    // world (0,0) without panning. Each wolf is addressed by its stable key hereafter.
    for i in 0..WOLF_COUNT {
        let (x, y) = (rng.below(9) as i32, rng.below(6) as i32);
        if bot.client.spawn_entity(wolf_key(i), KIND_WOLF, x, y).is_err() {
            tracing::error!("engine gone during spawn; exiting");
            return;
        }
    }
    tracing::info!(wolves = WOLF_COUNT, "spawned wolf pack in zone (0,0)");

    // Wander: every `move_ms`, send each wolf to a random cell anywhere in zone (0,0). The
    // edge wraps the tile into the 16×16 zone, so any (x, y) in the zone is valid.
    let mut ticker = tokio::time::interval(Duration::from_millis(move_ms));
    loop {
        ticker.tick().await;
        if !bot.pump() {
            tracing::warn!("client disconnected; stopping wildlife");
            return;
        }
        // Paused (debug `/pause`): the tic is frozen, so hold — don't issue moves until the
        // sim resumes. This is the tic-driven pause the freeze relies on.
        if bot.paused {
            continue;
        }
        for i in 0..WOLF_COUNT {
            let (x, y) = (rng.below(ZONE_DIM) as i32, rng.below(ZONE_DIM) as i32);
            if bot.client.move_pawn(wolf_key(i), x, y).is_err() {
                tracing::error!("engine gone during move; exiting");
                return;
            }
        }
    }
}

/// Log one client event at an appropriate level (concise — the npc mostly cares about the
/// login/disconnect lifecycle; row traffic is debug).
fn log_event(event: &Event) {
    match event {
        Event::LoginStarted { name } => tracing::info!(%name, "login started"),
        Event::ServerResolved(s) => tracing::info!(server_id = s.server_id, url = %s.url, "server resolved"),
        Event::LoggedIn { player_id, .. } => tracing::info!(player_id, "logged in"),
        Event::LoginFailed { reason } => tracing::error!(%reason, "login failed"),
        Event::Disconnected { reason } => tracing::warn!(?reason, "disconnected"),
        Event::Status(msg) => tracing::debug!(%msg, "status"),
        Event::StateObject { zone_id, object_id, location, .. } => {
            tracing::debug!(zone_id, object_id, location, "state object")
        }
        Event::ColdObjects { zone_id, .. } => tracing::debug!(zone_id, "cold objects"),
        Event::ZoneClosed { zone_id } => tracing::debug!(zone_id, "zone closed"),
        Event::Paused { paused } => tracing::debug!(paused, "paused"),
        Event::CallStats(_) | Event::SubStats { .. } | Event::ClockSync(_) => {}
    }
}
