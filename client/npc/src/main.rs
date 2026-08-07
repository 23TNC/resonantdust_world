//! npc — the automated-player BINARY: a thin dispatcher over the [`npc`] library (first-pawns
//! P4). Picks a [`npc::Brain`] by name (argv[1] or env `NPC_BRAIN`, default `wolves`), logs in
//! as the automated player (`NPC_NAME`), and hands the session to [`npc::run_brain`]. One
//! process = one brain; the supervised runner is `bin/sim run npc` (container `rd-npc`).

use std::time::{SystemTime, UNIX_EPOCH};

use client::ClientConfig;
use npc::brains::bunnies::Bunnies;
use npc::brains::wolves::Wolves;
use npc::{run_brain, Bot, Rng};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "npc=info,client=info".into()),
        )
        .init();

    let brain = std::env::args().nth(1).unwrap_or_else(|| env_or("NPC_BRAIN", "wolves"));
    let name = env_or("NPC_NAME", "Wolves");
    let tick_ms: u64 = env_or("MOVE_MS", "1000").parse().unwrap_or(1000);
    let config = ClientConfig::from_env();
    tracing::info!(%brain, %name, tick_ms, gateway = %config.gateway_url, "npc starting");

    let Some(bot) = Bot::login(config, &name).await else {
        // Exit NON-ZERO so the container's restart policy retries us (the edge may simply not
        // be up yet — sim-self-heal P4). Mid-run disconnects need no exit: the engine
        // auto-reconnects and replays the anchors.
        tracing::error!("npc login failed; exiting for the restart policy to retry");
        std::process::exit(1);
    };

    let rng = Rng(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1);

    // wildlife (the legacy client-minted pack fixture) was DELETED by input-rework F10 —
    // it drove raw MOVE_TO through the door this stream closed.
    match brain.as_str() {
        "wolves" => run_brain(bot, Wolves::new(rng), tick_ms).await,
        "bunnies" => run_brain(bot, Bunnies::new(rng), tick_ms).await,
        other => tracing::error!(%other, "unknown brain (have: wolves, bunnies)"),
    }
}
