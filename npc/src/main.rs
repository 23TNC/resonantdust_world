//! NPC driver — a headless bot that logs in like a player and drives NPCs by
//! issuing the same command verbs a real client does.
//!
//! It proves the architecture's key simplification: **NPCs are just automated
//! clients.** There is no server-side simulation to stall — an NPC's "AI" runs
//! here, in a client, and reaches the world through the identical path a player's
//! pixijs does: npc → [`client`] → server → spacetime (the sole authority).
//!
//! This first cut drives the debug mover (a tree): it logs in, anchors near a
//! home tile (so the gate subscribes the zone + connects the object shard — the
//! move relay needs that), and every few seconds picks a random nearby tile and
//! walks the mover there. Real pawns and a *set* of individually-addressable
//! owned NPCs are the next step (they need a per-object move command); the AI
//! loop here is already shaped as "manage a list of NPCs" with one member.
//!
//! Run (host, against the dev gateway's exposed port):
//! ```text
//! CLIENT_GATEWAY_URL=http://127.0.0.1:9473 npc/target/release/npc wolves
//! ```
//! or in-network via `npc/compose.yml`'s `run` service.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tracing::{error, info, warn};

use client::{AnchorRadii, Client, ClientConfig, Event};

/// How often the NPC picks a new destination for each of its charges.
const WANDER_INTERVAL: Duration = Duration::from_secs(3);

/// Home tile the NPC's charges wander around, and how far (tiles) they roam. Kept
/// clear of the world origin so the roam box stays in the valid (non-negative)
/// world — negative global tiles wrap to the far region and would fling the mover
/// across the map. Near enough to the origin to sit in a player's default view.
const HOME: (i32, i32) = (6, 6);
const ROAM: i32 = 5;

/// Subscription reach around home (tiles) — enough to keep the home zone (and its
/// object shard) live so move commands relay.
const RADII: AnchorRadii = AnchorRadii {
    active: 10,
    hot: 12,
    warm: 14,
    cold: 16,
};

#[tokio::main]
async fn main() {
    init_tracing();

    let config = ClientConfig::from_env();
    let name = std::env::args().nth(1).unwrap_or_else(|| "npc".to_string());
    info!(%name, gateway = %config.gateway_url, "npc: starting");

    // The client calls the sink for every event; forward them into a channel the
    // driver loop selects on.
    let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
    let client = Client::spawn(config, move |ev: Event| {
        let _ = tx.send(ev);
    });

    if client.login(name.clone()).is_err() {
        error!("npc: client engine gone before login");
        return;
    }

    let mut rng = seed();
    let mut logged_in = false;
    let mut wander = tokio::time::interval(WANDER_INTERVAL);

    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                Some(Event::LoggedIn { player_id, data_shard, .. }) => {
                    info!(player_id, data_shard, "npc: logged in");
                    // Anchor near home so the gate subscribes the zone and connects
                    // the object shard the move relay needs.
                    let _ = client.set_anchor("npc:home", HOME.0, HOME.1, 0, RADII, 0);
                    logged_in = true;
                }
                Some(Event::LoginFailed { reason }) => {
                    error!(%reason, "npc: login failed");
                    break;
                }
                Some(Event::Disconnected { reason }) => {
                    warn!(?reason, "npc: disconnected");
                    break;
                }
                // Zone/thing/clock events: the AI could react to them (make
                // decisions from observed state); the wander loop ignores them.
                Some(_) => {}
                None => break, // client engine stopped
            },
            _ = wander.tick() => {
                if logged_in {
                    let (x, y) = pick_destination(&mut rng);
                    info!(x, y, "npc: wander → move");
                    let _ = client.move_to(x, y);
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("npc: shutting down");
                break;
            }
        }
    }

    let _ = client.shutdown();
}

/// A random tile within [`ROAM`] tiles of [`HOME`], clamped to the non-negative
/// world (a defensive floor; with the chosen HOME/ROAM the box is already ≥ 1).
fn pick_destination(rng: &mut u64) -> (i32, i32) {
    let span = (2 * ROAM + 1) as u64;
    let dx = (next(rng) % span) as i32 - ROAM;
    let dy = (next(rng) % span) as i32 - ROAM;
    ((HOME.0 + dx).max(0), (HOME.1 + dy).max(0))
}

/// Seed an xorshift PRNG from the wall clock (no `rand` dependency for a bot that
/// just needs some wander). `| 1` guards the all-zero state.
fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1
}

/// xorshift64 — cheap, non-cryptographic randomness for destinations.
fn next(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}

/// `tracing` to stderr, level via `RUST_LOG` (default `info`).
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).with_target(false).init();
}
