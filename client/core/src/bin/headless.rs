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
    let mode = std::env::args().nth(2).unwrap_or_default();
    let anchor = mode == "anchor" || mode == "chords";
    // `chords`: the server-chords P2 survey — once the terrain is in, run `find_chords` over
    // random pairs in three distance bands against core's OWN composed view (the same view the
    // worker's pathability derives from) and report the chord-count distribution. The number the
    // design turns on is the fraction needing more than the queue depth: that fraction IS the
    // re-request stutter rate.
    let survey = mode == "chords";
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

    // **The survey has to generate the terrain it measures.** A zone comes into existence when a
    // client anchors on it and not before, so an anchor at the fluffle yields exactly one 16x16
    // zone and every longer trip is drawn through space that has never existed (I4). Lay a GRID
    // of anchors — distinct names, all retained, so the nested-radii release does not evict the
    // early ones behind the late ones — and let generation catch up before sampling.
    const ZONE_TILES: i32 = 16;
    let reach: i32 = std::env::var("SURVEY_REACH").ok().and_then(|v| v.parse().ok()).unwrap_or(48);
    let survey_at = survey.then(|| std::time::Instant::now() + std::time::Duration::from_secs(20));

    let code = loop {
        tokio::select! {
            _ = report.tick(), if logged_in => {
                if let Some(at) = survey_at {
                    if std::time::Instant::now() >= at {
                        chord_survey(&client, anchor_x, anchor_y);
                        break ExitCode::SUCCESS;
                    }
                }
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
                            if survey {
                                let mut n = 0;
                                let mut y = anchor_y - reach;
                                while y <= anchor_y + reach {
                                    let mut x = anchor_x - reach;
                                    while x <= anchor_x + reach {
                                        let _ = client.send(Command::SetAnchor {
                                            name: format!("gen:{n}"),
                                            tile_x: x,
                                            tile_y: y,
                                            radii: AnchorRadii { active: 1, hot: 1, warm: 1, cold: 1 },
                                            soul: 0,
                                        });
                                        n += 1;
                                        x += ZONE_TILES;
                                    }
                                    y += ZONE_TILES;
                                }
                                info!(anchors = n, reach, "seeding terrain for the survey");
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

/// **The chord-count survey** (server-chords P2): how many chords does a real trip need?
///
/// The design queues a route into a bounded intent queue, so a trip needing more chords than the
/// queue holds must be TRUNCATED and re-requested mid-walk. That re-request is a stutter, and its
/// frequency is the fraction of trips over the cap — not a number worth guessing, because a
/// plausible value (most trips need more than 10) makes this design worse than what it replaces.
///
/// Pathability comes from core's composed view, which is the same derivation the worker uses
/// (corpus flags × composed cells, unknown = OPEN), so this measures the map the pawns walk.
fn chord_survey(client: &Client, home_x: i32, home_y: i32) {
    // Bands in tiles: within a zone, across a zone, across several.
    const BANDS: [(i32, i32); 3] = [(3, 12), (12, 40), (40, 90)];
    const PER_BAND: usize = 1000;
    const CAP: usize = 10; // the proposed queue depth

    let pathable = |x: i32, y: i32| client.pathable(x, y);

    // **Sample only cells the view actually HAS.** The world is generated on demand, so most of
    // the coordinate space around any anchor has never existed; `pathable` calls an unknown cell
    // OPEN, and a pair drawn from that space string-pulls to one straight chord no matter what
    // the real terrain would be. The first run of this survey reported "p50 = 1 chord, 0% over
    // cap" from a square that was 99.3% unknown — a confident number about nothing.
    let reach: i32 = std::env::var("SURVEY_REACH").ok().and_then(|v| v.parse().ok()).unwrap_or(48);
    let known: Vec<(i32, i32)> = {
        let mut v = Vec::new();
        if let Ok(w) = client.world().lock() {
            for dx in -reach..=reach {
                for dy in -reach..=reach {
                    let at = (home_x + dx, home_y + dy);
                    if w.world.tile_kind_at(at).is_some() {
                        v.push(at);
                    }
                }
            }
        }
        v
    };
    let blocked = known.iter().filter(|(x, y)| !pathable(*x, *y)).count();
    let (ex0, ex1) = (
        known.iter().map(|c| c.0).min().unwrap_or(0),
        known.iter().map(|c| c.0).max().unwrap_or(0),
    );
    let (ey0, ey1) = (
        known.iter().map(|c| c.1).min().unwrap_or(0),
        known.iter().map(|c| c.1).max().unwrap_or(0),
    );
    info!(
        known = known.len(),
        impathable = format!("{blocked} ({:.2}%)", 100.0 * blocked as f64 / known.len().max(1) as f64),
        extent = format!("{ex0}..{ex1} x {ey0}..{ey1}"),
        "generated terrain in reach — pairs are drawn ONLY from these cells",
    );
    if known.len() < 2000 {
        warn!(
            known = known.len(),
            "too little generated terrain for a meaningful survey — the long bands cannot be \
             sampled at all; treat any result below as provisional",
        );
    }
    // A fixed multiplicative-congruential stream: the survey must be RE-RUNNABLE against a
    // changed map and give a comparable answer, which a wall-clock seed would not.
    let mut s: u64 = 0x2545_F491_4F6C_DD1D;
    let mut rnd = move |n: i32| -> i32 {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 33) % (n as u64).max(1)) as i32
    };

    info!(home_x, home_y, "chord survey — {PER_BAND} pairs per band");
    for (lo, hi) in BANDS {
        let (mut counts, mut refused, mut trivial) = (Vec::new(), 0usize, 0usize);
        let mut tried = 0usize;
        // Sampling is rejection-based: a pair whose endpoints are not both standable is not a
        // trip any pawn could be given, so it is re-drawn rather than counted as a refusal.
        while counts.len() + refused < PER_BAND && tried < PER_BAND * 200 {
            tried += 1;
            if known.is_empty() {
                break;
            }
            let a = known[rnd(known.len() as i32) as usize];
            let b = known[rnd(known.len() as i32) as usize];
            let d = (((b.0 - a.0).pow(2) + (b.1 - a.1).pow(2)) as f64).sqrt();
            if d < lo as f64 || d > hi as f64 {
                continue;
            }
            if !pathable(a.0, a.1) || !pathable(b.0, b.1) {
                continue;
            }
            match resonantdust_content::path_eval::find_chords(a, b, 0, &pathable) {
                Some(c) if c.is_empty() => trivial += 1,
                Some(c) => counts.push(c.len()),
                None => refused += 1,
            }
        }
        if counts.is_empty() {
            warn!(lo, hi, tried, refused, "band produced no routes");
            continue;
        }
        counts.sort_unstable();
        let pct = |p: usize| counts[(p * (counts.len() - 1)) / 100];
        let over = counts.iter().filter(|&&c| c > CAP).count();
        info!(
            band = format!("{lo}-{hi} tiles"),
            n = counts.len(),
            refused,
            trivial,
            p50 = pct(50),
            p90 = pct(90),
            p99 = pct(99),
            max = counts[counts.len() - 1],
            over_cap = format!("{over} ({:.2}%)", 100.0 * over as f64 / counts.len() as f64),
            "chords",
        );
    }
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
