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
    // Where to anchor. (0,0) subscribes terrain but fans NO pawn rows, so a probe there sees a
    // world with no movers — and, before the engine started stamping its estimate in, no tic at
    // all. Default to the fluffle so the read surface has something to answer about.
    let anchor_x: i32 = std::env::args().nth(3).and_then(|v| v.parse().ok()).unwrap_or(124);
    let anchor_y: i32 = std::env::args().nth(4).and_then(|v| v.parse().ok()).unwrap_or(75);

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

    // Hand core the corpus off disk so the derived answers (pace, pathability) work. Core does
    // not fetch it itself yet — see the stream's D3; a dev probe reading `content/` is enough to
    // exercise the read surface without a `#[cfg]` fork inside core.
    match resonantdust_content::content::read_content_dir(std::path::Path::new("content"))
        .map_err(|e| e.to_string())
        .and_then(|src| resonantdust_content::load(&src).map_err(|e| format!("{e:?}")))
    {
        Ok(bundle) => {
            info!("corpus loaded; core can answer derived reads");
            client.set_corpus(std::sync::Arc::new(bundle));
        }
        Err(e) => info!(error = %e, "no corpus on disk — positions only, no pace"),
    }

    let mut logged_in = false;
    // shared-simulation P2c/P2d: the READ surface, exercised. This binary is the only place a
    // headless consumer's view of core can be observed without a brain in the way, so it reports
    // what core ANSWERS — where each pawn is, at what tic, and one of its need rows — rather than
    // only what the wire said. Two of the stream's acceptance criteria are this output.
    let mut report = tokio::time::interval(std::time::Duration::from_secs(5));
    report.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let code = loop {
        tokio::select! {
            _ = report.tick(), if logged_in => {
                let Some(now) = client.now_tic() else {
                    info!("core has no tic yet — the estimate has not anchored");
                    continue;
                };
                // No guard is held here — `mover_entities` returns owned ids precisely so this
                // loop cannot re-enter the lock (I9, which I then repeated right here).
                let movers: Vec<(u32, (f64, f64))> = client
                    .mover_entities()
                    .into_iter()
                    .filter_map(|e| client.pawn_point(e).map(|p| (e, p)))
                    .collect();
                info!(now_tic = now, movers = movers.len(), "core answers");
                for (entity, (x, y)) in movers.iter().take(4) {
                    let pace = client.pawn_pace(*entity);
                    // One need row read back through the handle — the value, not the row count,
                    // because "the accessor exists" and "the accessor answers" are different
                    // claims and only the second one is worth a tick.
                    let needs = client.pawn_needs(*entity);
                    let first = needs.first().map(|(row, tic)| {
                        (resonantdust_codec::object::row_reference(*row),
                         resonantdust_codec::object::row_data(*row), *tic)
                    });
                    info!(
                        entity = format!("{entity:#010x}"),
                        x = format!("{x:.3}"), y = format!("{y:.3}"),
                        ?pace, need_rows = needs.len(), first_need = ?first,
                        "  pawn",
                    );
                }
            }
            event = ev_rx.recv() => match event {
                Some(event) => {
                    log_event(&event);
                    match event {
                        Event::LoggedIn { .. } => {
                            logged_in = true;
                            if anchor {
                                info!(tile_x = anchor_x, tile_y = anchor_y, "anchoring to subscribe terrain");
                                let _ = client.send(Command::SetAnchor {
                                    name: "headless:0".to_string(),
                                    tile_x: anchor_x,
                                    tile_y: anchor_y,
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
        // The stated route (server-chords P1). Logged with its ENDPOINTS decoded, because the
        // whole P1 gate is `stated dest tic` vs the arrival that actually lands — a chord count
        // alone would not answer it.
        Event::MoveChords { macro_position, entity_reference, serial, event_tic, stamps } => {
            use resonantdust_codec::object::position_to_tile;
            let (sx, sy) = position_to_tile(stamps[0]);
            let (dx, dy) = position_to_tile(stamps[stamps.len() - 2]);
            info!(
                macro_position = format!("{macro_position:#06x}"),
                entity_reference = format!("{entity_reference:#010x}"),
                serial,
                event_tic,
                chords = stamps.len() / 2 - 1,
                src = format!("{sx},{sy}@{}", stamps[1]),
                dest = format!("{dx},{dy}@{}", stamps[stamps.len() - 1]),
                "chord route"
            )
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
