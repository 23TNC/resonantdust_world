//! npc — the **automated-player** library. An npc drives the very same `client` (client/core)
//! command/event API a human does, so every action travels login → client → edge → spacetime
//! and is subject to the same validation and sync; there is no privileged direct-to-shard path.
//!
//! One npc process is one automated player running ONE [`Brain`] (first-pawns P4): the harness
//! ([`Bot`] + [`run_brain`]) owns login, anchoring, the event pump, and pause tracking; a brain
//! owns only behavior. A wolves-brain wanders its wolf, a future villager-brain runs a
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
    /// One world event (already logged by the harness). `act` is the ACTOR seam (npc-host
    /// F11): the client commands queue through — the brain's own session under the host
    /// (ownership attribution), the bot's own client standalone.
    fn on_event(&mut self, bot: &Bot, act: &Client, event: &Event);
    /// Periodic think. Not called while the simulation is paused.
    fn tick(&mut self, bot: &Bot, act: &Client);
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
                    brain.on_event(&bot, &bot.client, &event);
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
        brain.tick(&bot, &bot.client);
    }
}

/// One hosted module (npc-host P2, F1): a PLAYER — its own login (the brain def's name, the
/// F10 law), whose anchor is its position and whose mints are its pawns. The module session
/// is command-only: the world model lives on the HOST's session; this one carries identity,
/// command attribution, and the module's OWN player-pawn fan (its wolf_count etc.).
pub struct HostModule {
    /// The brain def's name — the login name, the def, the identity.
    pub name: String,
    /// The module's position in the world (F2: realized as a named anchor on the HOST session).
    pub center: (i32, i32),
    /// The operational-area radius in tiles (F3), read from the brain's `area_of_influence`
    /// bind through the STAT lane (F10) — 8.0 when unresolvable.
    pub radius: f64,
    /// The module-player's own session (command-only).
    pub session: Bot,
    /// The module's group brain (npc-host F11) — `None` until a brain exists for its kind.
    pub brain: Option<crate::brains::bunnies::Bunnies>,
    /// The module's own player-pawn, learned from its session's 0x40… need fan.
    pub player_pawn: Option<u32>,
}

/// Run the HOST (npc-host P2, F1): ONE world session (the host player) carrying every
/// module's named anchor + the shared world model, plus one command-only PLAYER session per
/// module. v1 ticks drain both lanes and surface each module's own player-pawn rows — the
/// group behaviors land on this skeleton with the P3 policy.
pub async fn run_host(config: ClientConfig, host_name: &str, spec: &[(String, (i32, i32))], tick_ms: u64) {
    let Some(mut world) = Bot::login(config.clone(), host_name).await else {
        tracing::error!("host login failed; exiting for the restart policy to retry");
        std::process::exit(1);
    };
    let bundle = match &world.server_url {
        Some(url) => fetch_corpus(url).await.ok(),
        None => None,
    };
    // shared-simulation P2e: seed the estimator's RATE before the stream teaches it. A cold
    // client spends ~60 s learning what the last one already knew, and during that window every
    // tic-derived answer — pace, need bands, the walk itself — is computed against the authored
    // 6 Hz rather than the ~5.4 the durable tic actually runs at. The seam existed for exactly
    // this and no headless host had ever called it (movement-hardening F5).
    let _ = world.client.send(Command::SeedTicRate {
        tics_per_sec: f64::from(resonantdust_codec::tic::TIC_HZ),
    });

    // shared-simulation P2c: core answers nothing DERIVED until it holds the corpus — pace,
    // pathability and every stat read through it.
    if let Some(b) = bundle.as_ref() {
        world.client.set_corpus(std::sync::Arc::new(b.clone()));
    }
    let mut modules: Vec<HostModule> = Vec::new();
    for (name, center) in spec {
        let Some(session) = Bot::login(config.clone(), name).await else {
            tracing::error!(%name, "module login failed; exiting for the restart policy to retry");
            std::process::exit(1);
        };
        let radius = bundle
            .as_ref()
            .and_then(|b| {
                let id = b.brain_object_id(name)?;
                let r = b.player_trait_stat("area_radius", &b.brain_player_traits(id));
                (r > 0.0).then_some(r)
            })
            .unwrap_or(8.0);
        // F2: the module's position IS a named anchor — on the WORLD session, whose engine
        // merges all anchors into the ONE zone set the shared model reads.
        let r = (radius.ceil() as i32).clamp(2, 16);
        let _ = world.client.send(Command::SetAnchor {
            name: format!("npc:{name}"),
            tile_x: center.0,
            tile_y: center.1,
            radii: AnchorRadii { active: r, hot: r + 2, warm: r + 4, cold: r + 6 },
            soul: 0,
        });
        // The ONE group brain (F6/F11): every hosted module runs it, parameterized by the
        // governed KIND (the corpus's affordance gates carry the diet — a wolf refuses
        // plant matter by content, not by code). The live-prey HUNT stays un-ported
        // (stage-2 world-state AI is its home); hosted wolves drink + scavenge + wander.
        let pawn_kind = match name.as_str() {
            "wolf_pack" => Some("wolf"),
            "bunny_fluffle" => Some("bunny"),
            _ => None,
        };
        let brain = pawn_kind.and_then(|kind| {
            bundle.as_ref().map(|b| {
                let count = b
                    .brain_object_id(name)
                    .map(|id| b.player_trait_stat("group_size", &b.brain_player_traits(id)))
                    .filter(|v| *v > 0.0)
                    .unwrap_or(3.0) as usize;
                let mut gb = crate::brains::bunnies::Bunnies::hosted(
                    Rng(rand_seed()),
                    kind,
                    *center,
                    radius.ceil() as i32,
                    count,
                );
                gb.init_with(b);
                gb
            })
        });
        tracing::info!(%name, ?center, radius, brain = brain.is_some(), "module-player up (npc-host P2)");
        modules.push(HostModule { name: name.clone(), center: *center, radius, session, brain, player_pawn: None });
    }
    let mut ticker = tokio::time::interval(Duration::from_millis(tick_ms));
    loop {
        ticker.tick().await;
        // The world lane: note ONCE into the shared model (I1), then fan read-only to
        // every module brain (sense is shared; act is per-module).
        loop {
            match world.events.try_recv() {
                Ok(event) => {
                    world.note(&event);
                    log_event(&event);
                    for m in &mut modules {
                        if let Some(brain) = &mut m.brain {
                            brain.on_event(&world, &m.session.client, &event);
                        }
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::warn!("world engine ended; stopping host");
                    return;
                }
            }
        }
        // Each module's OWN lane: its player-pawn fan (needs on a 0x40… ref) is the group
        // state the P3 policy reads; v1 surfaces it.
        for m in &mut modules {
            loop {
                match m.session.events.try_recv() {
                    Ok(Event::PawnNeed { entity_reference, need, .. })
                        if entity_reference >> 24 == 0x40 =>
                    {
                        tracing::info!(module = %m.name, player_pawn = format!("{entity_reference:#010x}"),
                            need = format!("{need:#010x}"), "module's own player-pawn row");
                        if m.player_pawn.is_none() {
                            m.player_pawn = Some(entity_reference);
                            if let (Some(brain), Some(b)) = (&mut m.brain, bundle.as_ref()) {
                                let count_need = if m.name == "wolf_pack" { "wolf_count" } else { "bunny_count" };
                                if let Some(nref) = b.gameplay_reference("need", count_need) {
                                    let (min, max) = b
                                        .need_params_by_ref(nref)
                                        .map(|np| (np.min, np.max))
                                        .unwrap_or((0.0, 16.0));
                                    brain.set_group(entity_reference, nref, min, max);
                                }
                            }
                        }
                    }
                    // The ownership set (I11/F4): the pawns THIS module minted, replayed at
                    // login and live thereafter — what the policy commands.
                    Ok(ev @ Event::OwnedPawn { entity_reference }) => {
                        tracing::info!(module = %m.name,
                            pawn = format!("{entity_reference:#010x}"), "module owns pawn");
                        if let Some(brain) = &mut m.brain {
                            brain.on_event(&world, &m.session.client, &ev);
                        }
                    }
                    Ok(_) => {}
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => break,
                }
            }
        }
        if world.paused {
            continue;
        }
        for m in &mut modules {
            if let Some(brain) = &mut m.brain {
                brain.tick(&world, &m.session.client);
            }
        }
    }
}

/// A wall-clock rng seed (npc motion is not part of sim determinism).
fn rand_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        | 1
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
    /// Known PAWN positions (npc-host F11): `entity → world tile`, from `StateObject`
    /// events (removed = gone). What a hosted brain adopts from when the OWNERSHIP frame
    /// loses the race against a resting pawn's one-shot snapshot replay.
    pawns: std::collections::HashMap<u32, (i32, i32)>,
}

impl Bot {
    /// Spawn a client engine, log in as `name`, and wait for the session to go live. Returns
    /// `None` if login fails (reported to the log).
    pub async fn login(config: ClientConfig, name: &str) -> Option<Bot> {
        let (ev_tx, events) = mpsc::unbounded_channel::<Event>();
        let client = Client::spawn(config, move |event: Event| {
            let _ = ev_tx.send(event);
        });
        let mut bot = Bot {
            client,
            events,
            paused: false,
            server_url: None,
            pawns: std::collections::HashMap::new(),
        };
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
            Event::StateObject { entity_reference, tile_x, tile_y, removed, .. } => {
                if *removed {
                    self.pawns.remove(entity_reference);
                } else {
                    self.pawns.insert(*entity_reference, (*tile_x, *tile_y));
                }
            }
            Event::Paused { paused } => {
                if *paused != self.paused {
                    tracing::info!(paused, "simulation freeze changed (/pause)");
                }
                self.paused = *paused;
            }
            // The known-zone tile map (interactions P4). Layer 0 is the ground baseline; a
            // zone streams ONE ROW PER BIOME (subtype), each carrying only its own cells —
            // so rows MERGE cell-wise, nonzero winning (seen live: zone 99 = 3 rows).
            _ => {}
        }
    }

    /// The composed `kind_id` (baseline ⊕ overlay) of the known tile at world `at`, or
    /// `None` if its zone hasn't streamed.
    pub fn tile_kind_at(&self, at: (i32, i32)) -> Option<u16> {
        self.client.world().lock().ok()?.world.tile_kind_at(at)
    }

    /// The nearest known tile (Chebyshev, world coords) whose composed `kind_id`
    /// (baseline ⊕ overlay) satisfies `pred` — the brain's "nearest water" scan
    /// (interactions P4). `None` while no zone has streamed or nothing matches.
    pub fn nearest_tile(
        &self,
        from: (i32, i32),
        pred: impl Fn(&client::world_view::WorldView, u16) -> bool,
    ) -> Option<(i32, i32)> {
        self.client.world().lock().ok()?.world.nearest_tile(from, pred)
    }

    /// The composed THING kind_id at world `at` (food-chain F8) — baseline ⊕ overrides,
    /// kind-0 suppressed. `None` = empty cell or an unstreamed zone.
    pub fn thing_kind_at(&self, at: (i32, i32)) -> Option<u16> {
        self.client.world().lock().ok()?.world.thing_kind_at(at)
    }

    /// The nearest known THING (Chebyshev, world coords) whose composed kind_id AND world
    /// cell satisfy `pred` — the brains' "nearest meat / plant matter" scan (food-chain F8).
    /// The cell is in the predicate so brains can refuse UNREACHABLE food (a drowned pawn's
    /// meat in the lake): the worker refuses impathable dests (pathfinding F5), and a brain
    /// that keeps picking one oscillates forever between the refusal and its wander.
    pub fn nearest_thing(
        &self,
        from: (i32, i32),
        pred: impl Fn(&client::world_view::WorldView, (i32, i32), u16) -> bool,
    ) -> Option<(i32, i32)> {
        self.client.world().lock().ok()?.world.nearest_thing(from, pred)
    }

    /// A known pawn's world tile, or `None` if it never streamed (or was removed).
    pub fn pawn_at(&self, entity: u32) -> Option<(i32, i32)> {
        self.pawns.get(&entity).copied()
    }

    /// The current sim tic, extrapolated from the latest anchor at its LEARNED rate — `None`
    /// until the first anchor lands. Receipt-stamped, so it carries event-delivery jitter
    /// (~a tick of the pump), which is fine for band evaluation; it is never a movement clock.
    pub fn now_tic(&self) -> Option<u16> {
        // shared-simulation P2e: ONE clock. The Bot used to keep its own anchor and extrapolate,
        // which is a second answer to a question that has one — and `api.rs` was handing every
        // host the formula to build it, so the leak was written into the contract as an
        // instruction. Core owns the estimate; every host asks.
        self.client.now_tic()
    }
}

/// Resolve a pawn kind's packed `definition_reference` from the world server's `/content`
/// corpus — the SAME corpus the browser renders with (first-pawns F5: content is the
/// authority; a pinned `KIND_*` constant drifts the moment `things.toml` reorders). The def is
/// the REAL packed form (human-pawns P0): `TYPE_PAWN | species | kind | variant 0`, taken
/// whole from the DEFINITION REGISTRY — the taxonomy the corpus authors, numbered by the
/// server. Speed is no longer resolved here (input-rework F8): a pawn's pace is the DERIVED
/// `ground_speed` stat, evaluated from its rows by whoever needs it. The payload is
/// `{ "toml": [[name, source], …] }`, loaded through shared/content.
pub async fn resolve_thing(server_url: &str, name: &str) -> Result<u32, String> {
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
) -> Result<std::collections::HashMap<(bool, String), u32>, String> {
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
                out.insert((is_tile, name.clone()), *id);
            }
        }
    };
    let tiles: Vec<String> = bundle.tile_names().to_vec();
    let things: Vec<String> = bundle.thing_names().to_vec();
    bind(true, &tiles, &|id| bundle.tile_taxonomy(id).cloned());
    bind(false, &things, &|id| bundle.thing_taxonomy(id).cloned());
    Ok(out)
}

/// Resolve a pawn kind's packed `definition_reference` from an already-loaded corpus —
/// [`resolve_thing`]'s body, split so a brain that keeps the [`Bundle`] resolves through the
/// one it holds.
pub fn resolve_thing_in(bundle: &resonantdust_content::loader::Bundle, name: &str) -> Result<u32, String> {
    if bundle.thing_object_id(name).is_none() {
        return Err(format!("thing `{name}` not in the corpus"));
    }
    // The def comes from the REGISTRY (definition-registry P5): its id already carries the whole
    // taxonomy — type, species, kind, variant — because that is what the registry numbers. The
    // species nibble used to be recovered by splitting the texture stem on `/` and looking the
    // segment up in a code-owned palette, i.e. a definition's identity derived from where its
    // pictures live. That is gone.
    //
    // No registry (an older server, an unseeded index) is a HARD error rather than a guess: a
    // mis-subtyped pawn would be adopted and rendered wrong forever, so fail at resolve, not at
    // draw — the same stance the stem-parsing took, for the same reason.
    bundle.definition_reference(false, name).ok_or_else(|| {
        format!("pawn `{name}`: not in the definition registry (unseeded index, or an older server)")
    })
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
        Event::PawnNeed { macro_position: zone, entity_reference, need, set_tic } => {
            tracing::debug!(zone, entity_reference, need, set_tic, "pawn need row")
        }
        Event::PawnInventory { macro_position: zone, entity_reference, slot, item, .. } => {
            tracing::debug!(zone, entity_reference, slot, item, "pawn inventory row")
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
        // QueueState is a DISPLAY fan (intent-queue-ui F1) — nothing for a brain to act on.
        Event::QueueState { .. }
        | Event::ColdState { .. }
        | Event::CallStats(_)
        | Event::SubStats { .. }
        | Event::ClockSync(_) => {}
        // The ownership re-attach lane (npc-host I11) — the brain-facing arm consumes it.
        Event::OwnedPawn { entity_reference } => {
            tracing::info!(entity_reference = format!("{entity_reference:#010x}"), "owned pawn")
        }
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
    fn a_pawn_without_a_registry_fails_loudly() {
        // definition-registry P5: the species used to be recovered from the texture stem, so a
        // corpus alone could resolve a pawn. It cannot now, and that is deliberate — a guessed
        // subtype would be adopted and rendered wrong forever. The pinned VALUES moved to
        // `master::defs::the_corpus_expands_to_the_ids_it_already_has`, which checks them against
        // the same corpus the registry is seeded from.
        let Some(bundle) = corpus() else { return };
        let err = super::resolve_thing_in(&bundle, "wolf").expect_err("no registry ⇒ no def");
        assert!(err.contains("definition registry"), "{err}");
    }

    #[test]
    fn an_injected_registry_yields_the_pinned_def() {
        // The other half: WITH the registry, the wolf resolves to the value the live npc logs.
        let Some(bundle) = corpus() else { return };
        let mut map = std::collections::HashMap::new();
        map.insert((false, "wolf".to_string()), 0x3001_0070u32);
        let bundle = bundle.with_registry(map);
        let def = super::resolve_thing_in(&bundle, "wolf").expect("wolf");
        assert_eq!(def, 0x3001_0070, "the wolf's packed def moved");
    }

    #[test]
    fn the_wolfs_derived_ground_speed_is_the_old_authored_pace() {
        // input-rework F8: the `speed` field is gone — the wolf's pace is the DERIVED
        // `ground_speed` from its walks binding. 12 tics/tile is the value every stored trip
        // was spaced by; this pin catches a corpus retune moving it silently.
        // shared-simulation P2: this test had rotted against trait-rows-u32 (which DELETED
        // `TraitBind::level` in favour of the reference's variant nibble) and had been failing
        // `bin/sim check npc` since. Rewritten onto `move_eval::ground_speed` — THE pace, the
        // same call the worker spaces its hops with — so it now pins the value through the
        // shared path instead of a local reassembly of it.
        let Some(bundle) = corpus() else { return };
        let kind = bundle.thing_object_id("wolf").expect("wolf kind");
        let mut payload: Vec<u32> = Vec::new();
        for b in bundle.thing_traits(kind) {
            let Some(cat) = bundle.trait_category(&b.name) else { continue };
            let Some(cat_name) = resonantdust_codec::object::gameplay_category(cat) else {
                continue;
            };
            if let Some(r0) = bundle.gameplay_reference(cat_name, &b.name) {
                let r = (r0 & !0xF) | (u32::from(b.variant) & 0xF);
                payload.extend_from_slice(&resonantdust_codec::payload::trait_entry(r, 0));
            }
        }
        let v = resonantdust_content::move_eval::ground_speed(&bundle, kind, &payload, &[], 0);
        assert_eq!(v, 12.0);
    }
}
