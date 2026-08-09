//! `headless` — a minimal CLI host for the [`client`] library.
//!
//! It's the smallest possible driver: build a [`Client`], send one
//! [`Command::Login`], and print every [`Event`] the client emits. Handy for
//! smoke-testing the gateway + world-server round-trip without webgl, and as a
//! worked example of how an external program drives the headless client.
//!
//! ```text
//! headless <name>            # log in as <name> against the configured gateway
//! headless <name> anchor     # ... then anchor at zone (0,0) to subscribe + log its rows
//! RD_ENV=claude headless Bob # ... against the claude env's gateway
//! CLIENT_GATEWAY_URL=http://127.0.0.1:9473 headless Bob
//! ```
//!
//! After a successful login it stays connected (so disconnects surface) until
//! Ctrl-C; a failed login exits non-zero.

use std::process::ExitCode;

use client::{AnchorRadii, Client, ClientConfig, Command, Event};
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

    // Optional `anchor` arg: after login, drop an anchor at the origin so the client
    // subscribes the zones around (0,0) — exercising the terrain seed + row relay so the
    // ZoneTiles / ZoneThings events show in the log (a full round-trip smoke test, not
    // just login).
    let anchor = std::env::args().nth(2).as_deref() == Some("anchor");

    let config = ClientConfig::from_env();
    info!(gateway = %config.gateway_url, %name, anchor, "logging in");

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
                        Event::LoggedIn { .. } => {
                            logged_in = true;
                            if anchor {
                                info!("anchoring at zone (0,0) to subscribe terrain");
                                let _ = client.send(Command::SetAnchor {
                                    name: "headless:0".to_string(),
                                    tile_x: 0,
                                    tile_y: 0,
                                    radii: AnchorRadii { active: 2, hot: 4, warm: 6, cold: 8 },
                                    soul: 0,
                                });
                            }
                        }
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
            player_shard_reference,
            server_url,
        } => info!(player_id, player_shard_reference, url = %server_url, "logged in"),
        Event::LoginFailed { reason } => error!(%reason, "login failed"),
        Event::Disconnected { reason } => warn!(reason = ?reason, "disconnected"),
        Event::Status(msg) => info!(%msg, "status"),
        Event::StateObject {
            macro_position,
            entity_reference,
            definition_reference,
            tile_x,
            tile_y,
            sub_x,
            sub_y,
            facing,
            tic,
            removed,
        } => {
            info!(
                macro_position = format!("{macro_position:#06x}"),
                entity_reference = format!("{entity_reference:#010x}"),
                definition_reference,
                tile_x,
                tile_y,
                sub_x,
                sub_y,
                facing,
                tic,
                removed,
                "state object"
            )
        }
        Event::PawnParts { macro_position, entity_reference, tic, parts, payload } => {
            info!(
                macro_position = format!("{macro_position:#06x}"),
                entity_reference = format!("{entity_reference:#010x}"),
                tic,
                parts = format!("{parts:?}"),
                payload = format!("{payload:?}"),
                "pawn parts"
            )
        }
        Event::OwnedPawn { entity_reference } => {
            info!(entity_reference = format!("{entity_reference:#010x}"), "owned pawn")
        }
        Event::PawnInventory { entity_reference, slot, item, .. } => {
            info!(entity_reference = format!("{entity_reference:#010x}"), slot, item, "pawn inventory row")
        }
        Event::PawnNeed { macro_position, entity_reference, need, set_tic } => {
            info!(
                macro_position = format!("{macro_position:#06x}"),
                entity_reference = format!("{entity_reference:#010x}"),
                need = format!("{need:#010x}"),
                set_tic,
                "pawn need row"
            )
        }
        Event::ColdTiles { macro_position, subtype_id, layer_id, tic, tiles } => {
            info!(macro_position = format!("{macro_position:#06x}"), subtype_id, layer_id, tic, tiles = tiles.len(), "cold tiles")
        }
        Event::ColdThings { macro_position, subtype_id, layer_id, tic, things } => {
            info!(macro_position = format!("{macro_position:#06x}"), subtype_id, layer_id, tic, things = things.len(), "cold things")
        }
        Event::ColdState { macro_position, entity_reference, position_reference, definition_reference, data, tic, removed } => {
            info!(
                macro_position = format!("{macro_position:#06x}"),
                entity_reference = format!("{entity_reference:#010x}"),
                position_reference = format!("{position_reference:#010x}"),
                definition_reference = format!("{definition_reference:#010x}"),
                data,
                tic,
                removed,
                "cold state override"
            )
        }
        Event::ZoneClosed { macro_position } => {
            info!(macro_position = format!("{macro_position:#06x}"), "zone closed")
        }
        Event::Paused { paused } => info!(paused, "simulation freeze changed"),
        Event::TicAnchor { tic, wall_ms, tics_per_sec } => info!(tic, wall_ms, tics_per_sec, "tic estimate re-anchored"),
        Event::MoveIntent { macro_position, entity_reference, tile_x, tile_y, event_tic } => {
            info!(macro_position, entity_reference, tile_x, tile_y, event_tic, "move intent")
        }
        Event::QueueState { macro_position, entity_reference, event_tic, entries } => {
            info!(macro_position, entity_reference, event_tic,
                intents = entries.len() / 4, "intent-queue snapshot")
        }
        // The web engine emits these for the webgl debug HUD; the headless driver
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
