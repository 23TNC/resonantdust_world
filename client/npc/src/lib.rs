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
    /// The latest tic anchor `(tic, received_at, tics_per_sec)` off [`Event::TicAnchor`] — the
    /// engine's LEARNED rate estimate (never raw `TIC_HZ`), stamped at event receipt. Feeds
    /// [`Bot::now_tic`], which is what lets a brain evaluate lazy needs (needs-moodlets F4).
    tic_anchor: Option<(u16, std::time::Instant, f64)>,
}

impl Bot {
    /// Spawn a client engine, log in as `name`, and wait for the session to go live. Returns
    /// `None` if login fails (reported to the log).
    pub async fn login(config: ClientConfig, name: &str) -> Option<Bot> {
        let (ev_tx, events) = mpsc::unbounded_channel::<Event>();
        let client = Client::spawn(config, move |event: Event| {
            let _ = ev_tx.send(event);
        });
        let mut bot = Bot { client, events, paused: false, server_url: None, tic_anchor: None };
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

    /// Track harness-level state off an event (the pause flag + the tic anchor).
    pub(crate) fn note(&mut self, event: &Event) {
        match event {
            Event::Paused { paused } => {
                if *paused != self.paused {
                    tracing::info!(paused, "simulation freeze changed (/pause)");
                }
                self.paused = *paused;
            }
            Event::TicAnchor { tic, tics_per_sec, .. } => {
                self.tic_anchor = Some((*tic, std::time::Instant::now(), *tics_per_sec));
            }
            _ => {}
        }
    }

    /// The current sim tic, extrapolated from the latest anchor at its LEARNED rate — `None`
    /// until the first anchor lands. Receipt-stamped, so it carries event-delivery jitter
    /// (~a tick of the pump), which is fine for band evaluation; it is never a movement clock.
    pub fn now_tic(&self) -> Option<u16> {
        let (tic, at, rate) = self.tic_anchor?;
        let elapsed = at.elapsed().as_secs_f64() * rate;
        Some(tic.wrapping_add(elapsed as u16))
    }
}

/// Resolve a pawn kind's `(packed definition_reference, tics-per-tile)` from the world server's
/// `/content` corpus — the SAME corpus the browser renders with (first-pawns F5: content is the
/// authority; a pinned `KIND_*` constant drifts the moment `things.toml` reorders). The def is the
/// REAL packed form (human-pawns P0): `TYPE_PAWN | species | kind | variant 0`, species read off
/// the kind's texture stem (`pawn/<species>/…` — the folder taxonomy and the definition fields
/// are 1:1) through the code-owned palette. Speed is content too (pawn-movement F1/F5): the
/// authored tics-per-tile, already resolved through `codec::speed::resolve` (unauthored →
/// default). The payload is `{ "toml": [[name, source], …] }`, loaded through shared/content.
pub async fn resolve_thing(server_url: &str, name: &str) -> Result<(u32, u16), String> {
    let bundle = fetch_corpus(server_url).await?;
    resolve_thing_in(&bundle, name)
}

/// Fetch + load the world server's `/content` corpus into a [`Bundle`] — the def-id, speed
/// AND needs authority (needs-moodlets P4: the Brain keeps the bundle to run `needs_eval`
/// on its pawn's payload, the same eval the client reaches through wasm — F3).
pub async fn fetch_corpus(server_url: &str) -> Result<resonantdust_content::loader::Bundle, String> {
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
    let files =
        body["toml"].as_array().ok_or_else(|| format!("{url}: no `toml` array"))?;
    let sources: Vec<(String, String)> = files
        .iter()
        .filter_map(|pair| {
            Some((pair.get(0)?.as_str()?.to_string(), pair.get(1)?.as_str()?.to_string()))
        })
        .collect();
    let bundle = resonantdust_content::loader::load(&sources)
        .map_err(|errs| format!("corpus load: {} error(s), first: {:?}", errs.len(), errs.first()))?;

    // definition-registry P5: bind the registry's numbering, so a name resolves through the TABLE
    // rather than through whatever the corpus happens to say. Best-effort — an older server with no
    // `/definitions`, or an unseeded index, leaves the corpus's own ids answering, which is exactly
    // today's behaviour.
    match fetch_registry(base, &bundle).await {
        Ok(map) if !map.is_empty() => {
            tracing::info!(definitions = map.len(), "definitions bound from the registry");
            Ok(bundle.with_registry(map))
        }
        Ok(_) => Ok(bundle),
        Err(err) => {
            tracing::warn!(%err, "definition registry unavailable; resolving through the corpus");
            Ok(bundle)
        }
    }
}

/// Fetch `/definitions` and bind it to the corpus by NAME, the same pairing the edge and the webgl
/// client do: `name → tuple` is authored in the corpus, `tuple → id` is the registry.
async fn fetch_registry(
    base: &str,
    bundle: &resonantdust_content::loader::Bundle,
) -> Result<std::collections::HashMap<(bool, String), u16>, String> {
    let url = format!("{base}/definitions");
    let body: serde_json::Value = reqwest::get(&url)
        .await
        .map_err(|e| format!("GET {url}: {e}"))?
        .json()
        .await
        .map_err(|e| format!("{url}: bad JSON: {e}"))?;
    let rows = body["definitions"].as_array().ok_or_else(|| format!("{url}: no `definitions`"))?;

    // tuple → highest-version id (F6: newest wins for a NAME lookup).
    let mut newest: std::collections::HashMap<(String, String, String, String), (u32, u64)> =
        std::collections::HashMap::new();
    for r in rows {
        let f = |i: usize| r.get(i).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let (Some(id), Some(version)) =
            (r.get(0).and_then(|v| v.as_u64()), r.get(1).and_then(|v| v.as_u64()))
        else {
            continue;
        };
        let e = newest.entry((f(2), f(3), f(4), f(5))).or_insert((id as u32, version));
        if version > e.1 {
            *e = (id as u32, version);
        }
    }

    let mut out = std::collections::HashMap::new();
    let mut bind = |is_tile: bool, names: &[String], tax: &dyn Fn(u16) -> Option<resonantdust_content::loader::Taxonomy>| {
        for (i, name) in names.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            let Some(t) = tax((i + 1) as u16) else { continue };
            let Some((sub, variant)) =
                t.tuples().first().map(|(s, v)| (s.to_string(), v.to_string()))
            else {
                continue;
            };
            if let Some((id, _)) = newest.get(&(t.type_name.clone(), sub, t.kind.clone(), variant)) {
                out.insert((is_tile, name.clone()), resonantdust_codec::object::def_kind_id(*id));
            }
        }
    };
    let tiles: Vec<String> = bundle.tile_names().to_vec();
    let things: Vec<String> = bundle.thing_names().to_vec();
    bind(true, &tiles, &|id| bundle.tile_taxonomy(id).cloned());
    bind(false, &things, &|id| bundle.thing_taxonomy(id).cloned());
    Ok(out)
}

/// Resolve a pawn kind's `(packed definition_reference, tics-per-tile)` from an already-loaded
/// corpus — [`resolve_thing`]'s body, split so a brain that keeps the [`Bundle`] resolves
/// through the one it holds.
pub fn resolve_thing_in(bundle: &resonantdust_content::loader::Bundle, name: &str) -> Result<(u32, u16), String> {
    let kind = bundle
        .thing_object_id(name)
        .ok_or_else(|| format!("thing `{name}` not in the corpus"))?;
    // Species = the texture stem's subtype segment (`pawn/<species>/<kind>`). No stem or an
    // unknown species is a HARD error — a mis-subtyped def would be adopted/rendered wrong
    // forever, so fail at resolve, not at draw.
    let stem = bundle
        .visual_for_object(kind)
        .and_then(|v| v.texture)
        .ok_or_else(|| format!("pawn `{name}`: no texture stem in the corpus (species unresolvable)"))?;
    let species_name = stem
        .split('/')
        .nth(1)
        .ok_or_else(|| format!("pawn `{name}`: stem `{stem}` has no subtype segment"))?
        .to_string();
    let species = resonantdust_codec::object::pawn_species_subtype_id(&species_name)
        .ok_or_else(|| format!("pawn `{name}`: species `{species_name}` not in the palette"))?;
    let def = resonantdust_codec::object::pack_definition_from_ids(
        resonantdust_codec::object::TYPE_PAWN,
        species,
        kind,
        0, // variant 0 = canonical art; the payload's PART defs carry the dressed variants
    );
    Ok((def, resonantdust_codec::speed::resolve(bundle.thing_speed(kind))))
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
        Event::PawnParts { macro_position: zone, entity_reference, .. } => {
            tracing::debug!(zone, entity_reference, "pawn parts")
        }
        Event::ColdTiles { macro_position: zone, .. } => tracing::debug!(zone, "cold tiles"),
        Event::ColdThings { macro_position: zone, .. } => tracing::debug!(zone, "cold things"),
        Event::ZoneClosed { macro_position: zone } => tracing::debug!(zone, "zone closed"),
        Event::Paused { paused } => tracing::debug!(paused, "paused"),
        Event::TicAnchor { tic, wall_ms, tics_per_sec } => {
            tracing::debug!(tic, wall_ms, tics_per_sec, "tic re-anchor");
        }
        Event::MoveIntent { macro_position: zone, entity_reference, tile_x, tile_y, event_tic } => {
            tracing::info!(zone, entity_reference, tile_x, tile_y, event_tic, "move intent")
        }
        Event::ColdState { .. } | Event::CallStats(_) | Event::SubStats { .. } | Event::ClockSync(_) => {}
    }
}

#[cfg(test)]
mod def_fixture {
    //! The PACKED-COMPOSITION half of the definition-registry P0 oracle.
    //!
    //! `shared/content`'s golden fixture pins `name → kind_id`, but it cannot pin the packed
    //! `definition_reference` — that crate deliberately has no codec dependency. This does, and it
    //! is also where the composition actually happens, so the pinning lives beside the code that
    //! will change.
    //!
    //! What it guards: [`resolve_thing_in`] derives a pawn's SPECIES by string-parsing the texture
    //! stem's second segment through a code-owned palette. P5 deletes that and reads the species
    //! from the authored taxonomy instead. These values must not move when it does — a re-subtyped
    //! pawn would be adopted and rendered wrong forever.

    use resonantdust_content::loader::load;

    /// The repo's authored corpus, or `None` in a packaged build without it.
    fn corpus() -> Option<resonantdust_content::loader::Bundle> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let sources = resonantdust_content::content::read_content_dir(&root).ok()?;
        if sources.is_empty() {
            return None;
        }
        Some(load(&sources).expect("the repo corpus loads clean"))
    }

    #[test]
    fn packed_pawn_defs_are_pinned() {
        let Some(bundle) = corpus() else { return };
        // (name, packed definition_reference, tics-per-tile) as of 2026-08-04. The wolf's
        // `0x30010070` is the value the live npc logs on every boot.
        for (name, want_def, want_speed) in
            [("wolf", 0x3001_0070u32, 12u16), ("human_female", 0x3002_00A0, 16), ("human_male", 0x3002_00B0, 16)]
        {
            let (def, speed) = super::resolve_thing_in(&bundle, name).expect(name);
            assert_eq!(def, want_def, "{name}: packed def moved (was {want_def:#010x}, now {def:#010x})");
            assert_eq!(speed, want_speed, "{name}: speed moved");
        }
    }

    #[test]
    fn species_comes_from_the_stem_today() {
        // The coupling P5 removes, asserted so its removal is a deliberate, visible change:
        // the species nibble is the texture stem's SECOND segment, not an authored field.
        let Some(bundle) = corpus() else { return };
        let (def, _) = super::resolve_thing_in(&bundle, "wolf").unwrap();
        assert_eq!(
            resonantdust_codec::object::def_subtype_id(def),
            resonantdust_codec::object::pawn_species_subtype_id("animal").unwrap(),
        );
    }
}
