//! `headless` — a minimal CLI host for the [`client`] library.
//!
//! It's the smallest possible driver: build a [`Client`], send one
//! [`Command::Login`], and print every [`Event`] the client emits. Handy for
//! smoke-testing the gateway + world-server round-trip without pixijs, and as a
//! worked example of how an external program drives the headless client.
//!
//! ```text
//! headless <name>            # log in as <name> against the configured gateway
//! RD_ENV=claude headless Bob # ... against the claude env's gateway
//! CLIENT_GATEWAY_URL=http://127.0.0.1:9473 headless Bob
//! ```
//!
//! After a successful login it stays connected (so disconnects surface) until
//! Ctrl-C; a failed login exits non-zero.

use std::process::ExitCode;

use client::{Client, ClientConfig, Command, Event};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing();

    let name = match std::env::args().nth(1) {
        Some(n) if !n.is_empty() => n,
        _ => {
            eprintln!("usage: headless <player-name>");
            return ExitCode::FAILURE;
        }
    };

    let config = ClientConfig::from_env();
    info!(gateway = %config.gateway_url, %name, "logging in");

    // The event sink forwards into a channel this task reads — so all the
    // interpretation (when login finished, success or failure) stays here.
    let (ev_tx, mut ev_rx) = mpsc::unbounded_channel::<Event>();
    let client = Client::spawn(config, move |event: Event| {
        let _ = ev_tx.send(event);
    });

    if client.send(Command::Login { name }).is_err() {
        error!("client engine failed to start");
        return ExitCode::FAILURE;
    }

    let mut logged_in = false;
    let code = loop {
        tokio::select! {
            event = ev_rx.recv() => match event {
                Some(event) => {
                    log_event(&event);
                    match event {
                        Event::LoggedIn { .. } => logged_in = true,
                        // A failed login is terminal; a disconnect after login is too.
                        Event::LoginFailed { .. } => break ExitCode::FAILURE,
                        Event::Disconnected { .. } if logged_in => break ExitCode::SUCCESS,
                        _ => {}
                    }
                }
                None => break ExitCode::SUCCESS, // engine ended
            },
            _ = tokio::signal::ctrl_c() => {
                info!("interrupted; shutting down");
                let _ = client.shutdown();
                break ExitCode::SUCCESS;
            }
        }
    };

    code
}

/// Render an event as a log line.
fn log_event(event: &Event) {
    match event {
        Event::LoginStarted { name } => info!(%name, "login started"),
        Event::ServerResolved(s) => info!(
            server_id = s.server_id,
            url = %s.url,
            reused = s.reused,
            "gateway resolved server",
        ),
        Event::LoggedIn {
            player_id,
            data_shard,
            server_url,
        } => info!(player_id, data_shard, url = %server_url, "logged in"),
        Event::LoginFailed { reason } => error!(%reason, "login failed"),
        Event::Disconnected { reason } => warn!(reason = ?reason, "disconnected"),
        Event::Status(msg) => info!(%msg, "status"),
        Event::ZoneTiles { zone_id, tiles } => {
            info!(zone_id = format!("{zone_id:#010x}"), tiles = tiles.len(), "zone tiles")
        }
        Event::ZoneThings { zone_id, things } => {
            info!(zone_id = format!("{zone_id:#010x}"), things = things.len(), "zone things")
        }
        Event::ZoneFreeThing { zone_id, object_id, removed, location, id, .. } => {
            info!(
                zone_id = format!("{zone_id:#010x}"),
                object_id, removed, location, id, "free thing"
            )
        }
        Event::ZoneClosed { zone_id } => {
            info!(zone_id = format!("{zone_id:#010x}"), "zone closed")
        }
        Event::ExperimentObject { object_id, x, y } => {
            info!(object_id, x, y, "experiment object")
        }
        // The web engine emits these for the pixijs debug HUD; the headless driver
        // has no HUD, so there's nothing to log. `ClockSync` fires every couple of
        // seconds — logging it would drown the smoke test.
        Event::CallStats(_) | Event::SubStats { .. } | Event::ClockSync(_) => {}
    }
}

/// `tracing` as the single logging path, level via `RUST_LOG` (default `info`).
fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
