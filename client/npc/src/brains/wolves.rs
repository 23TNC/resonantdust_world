//! wolves — the first-pawns v1 brain: spawn ONE server-minted wolf (`CREATE`, def resolved
//! from the content corpus — F5) near a home tile, adopt the minted id from the zone's state
//! fan-out (F3), then wander it with single A→B `MOVE_TO`s — the WORKER chains the hops (F2);
//! this brain only picks destinations and waits for arrivals (or a timeout) before the next.

use std::collections::HashMap;
use std::time::Duration;

use client::world::tile_to_position;
use client::Event;
use resonantdust_codec::action::{CREATE, GRANT_CONDITION, PROMOTE, SET_NEED};
use resonantdust_codec::object::{def_sans_variant, position_macro, TYPE_PAWN};
use resonantdust_codec::payload::payload_needs;
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::speed::DEFAULT_TICS_PER_TILE;
use resonantdust_codec::tic::TIC_HZ;
use resonantdust_content::loader::Bundle;
use resonantdust_content::needs_eval;

use crate::{fetch_corpus, resolve_thing_in, Bot, Brain, Rng};

/// Server-minted pawn ids live in the TOP half of the object space (`pawn::SPAWN_BASE`) —
/// the adoption filter, so a legacy client-minted wolf is never adopted.
const MINTED_BASE: u32 = 0x0080_0000;

pub struct Wolves {
    rng: Rng,
    /// Home tile (env `NPC_HOME`, default the dev vantage `100,50`) — anchor + wander center.
    home: (i32, i32),
    /// Wander radius, tiles (env `NPC_RADIUS`, default 6).
    radius: i32,
    /// The wolf's def (resolved from the corpus at start; `0` until then).
    def: u32,
    /// The wolf's authored tics-per-tile (corpus, pawn-movement F1 — same value the worker
    /// spaces hops with), for deadline arithmetic.
    speed: u16,
    /// The adopted minted wolf, once its first `StateObject` lands.
    wolf: Option<u32>,
    /// The current trip's destination, if one is in flight.
    dest: Option<(i32, i32)>,
    /// Give-up wall time for the current trip — a lost chain (I3 territory) re-issues.
    deadline: std::time::Instant,
    /// Where the wolf last was, authoritatively (for trip planning).
    at: (i32, i32),
    /// Adopt-first window: CREATE only if no existing minted wolf shows up by this wall time
    /// (the zone snapshot replays resting pawns on subscribe) — restarts re-use their wolf
    /// instead of minting a pack.
    spawn_after: std::time::Instant,
    created: bool,
    /// The content corpus (needs-moodlets P4) — the def/needs authority AND the eval's
    /// parameter source (F3: the SAME `needs_eval` the client reaches through wasm).
    bundle: Option<Bundle>,
    /// The wolf kind's thirst `need_id` (from `&thing.needs`; `0` = kind carries none).
    thirst: u8,
    /// Latest RAW payload per entity — buffered pre-adoption too (the zone snapshot may
    /// fan `Payload` before the `StateObject` this brain adopts on; either order is legal).
    payloads: HashMap<u32, Vec<u32>>,
    /// Thirst-init grace: wait past this before concluding the wolf HAS no thirst row —
    /// the snapshot's payload replay must get its chance, or a restart would reset a
    /// half-drained thirst to full.
    init_after: std::time::Instant,
    /// Single-shot latch for the SET_NEED mint (the splice upserts, so a replay is safe —
    /// the latch only stops US from re-queueing every tick).
    init_queued: bool,
    /// The ACTIVE condition ids at the last evaluation — the transition detector.
    active: Vec<u16>,
    /// The decision context the successor action stream reads: current mood.
    pub mood: f64,
    /// **Drill** (env `NPC_THIRST`, `0..=255`): the satisfaction the mint writes instead of
    /// full. Thirst depletes over 21600 tics (1 h wall at 6 Hz), so waiting for a real band
    /// crossing is not a test loop — `NPC_THIRST=20` puts the wolf inside Dehydrated at once.
    /// Unset = the authored behaviour, full (`255`).
    thirst_drill: Option<u8>,
    /// **Drill** (env `NPC_GRANT`, comma-separated condition ids): TIMED grants to queue once
    /// after the mint, through `GRANT_CONDITION`. Nothing in the game issues that verb yet —
    /// the drink action is the successor stream — so this is the only way to exercise the
    /// stored-grant lane end to end, and the only way to put enough conditions on one pawn to
    /// test the details panel's card strip.
    grant_drill: Vec<u32>,
    /// Single-shot latch for `grant_drill`.
    grants_queued: bool,
}

impl Wolves {
    pub fn new(rng: Rng) -> Self {
        let home = std::env::var("NPC_HOME")
            .ok()
            .and_then(|s| {
                let (x, y) = s.split_once(',')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            })
            .unwrap_or((100, 50));
        let radius: i32 = std::env::var("NPC_RADIUS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
        Self {
            rng,
            home,
            radius,
            def: 0,
            speed: DEFAULT_TICS_PER_TILE,
            wolf: None,
            dest: None,
            deadline: std::time::Instant::now(),
            at: home,
            spawn_after: std::time::Instant::now(),
            created: false,
            bundle: None,
            thirst: 0,
            payloads: HashMap::new(),
            init_after: std::time::Instant::now(),
            init_queued: false,
            active: Vec::new(),
            mood: needs_eval::MOOD_BASE,
            thirst_drill: std::env::var("NPC_THIRST").ok().and_then(|s| s.trim().parse().ok()),
            grant_drill: std::env::var("NPC_GRANT")
                .ok()
                .map(|s| s.split(',').filter_map(|p| p.trim().parse().ok()).collect())
                .unwrap_or_default(),
            grants_queued: false,
        }
    }

    fn pick_dest(&mut self) -> (i32, i32) {
        let span = (self.radius * 2 + 1) as u32;
        (
            self.home.0 - self.radius + self.rng.below(span) as i32,
            self.home.1 - self.radius + self.rng.below(span) as i32,
        )
    }

    /// The needs pass, each decision tick (needs-moodlets P4): mint thirst once on a wolf
    /// that provably has none, then EVALUATE — the lazy read (F4) of the payload's NEED +
    /// CONDITION entries through the shared `needs_eval` (F3). Band transitions are logged
    /// with the tic they were observed at; between crossings this computes and logs
    /// nothing, because nothing changed and nothing was written. Behaviour stays A→B —
    /// the conditions are the DECISION CONTEXT the successor action stream reads.
    fn mind_needs(&mut self, bot: &Bot) {
        let Some(wolf) = self.wolf else { return };
        let Some(bundle) = &self.bundle else { return };
        let payload = self.payloads.get(&wolf).cloned().unwrap_or_default();

        // Mint: no thirst row after the snapshot grace → this wolf was never initialised.
        if self.thirst != 0 && !self.init_queued && std::time::Instant::now() > self.init_after {
            let has_thirst = payload_needs(&payload).iter().any(|&(id, _, _)| id == self.thirst);
            if !has_thirst {
                let sat = self.thirst_drill.unwrap_or(255);
                let program = vec![SET_NEED, wolf, u32::from(self.thirst), u32::from(sat)];
                if bot.client.queue(program).is_err() {
                    tracing::error!("engine gone during thirst init");
                    return;
                }
                tracing::info!(wolf = format!("{wolf:#010x}"), need = self.thirst, satisfaction = sat,
                               drill = self.thirst_drill.is_some(),
                               "thirst initialised (no row after the snapshot grace)");
            }
            self.init_queued = true;
        }

        // Drill: queue the requested TIMED grants once (`NPC_GRANT=3` etc.). One program per
        // grant — `GRANT_CONDITION` takes a single condition id and upserts by it.
        if !self.grants_queued && !self.grant_drill.is_empty() && self.init_queued {
            for &id in &self.grant_drill {
                if bot.client.queue(vec![GRANT_CONDITION, wolf, id]).is_err() {
                    tracing::error!("engine gone during the grant drill");
                    return;
                }
                tracing::info!(wolf = format!("{wolf:#010x}"), condition = id,
                               "GRANT_CONDITION queued (drill)");
            }
            self.grants_queued = true;
        }

        // Evaluate — needs nothing but the row, the tic, and the corpus.
        let Some(now) = bot.now_tic() else { return };
        let needs = payload_needs(&payload);
        let grants = resonantdust_codec::payload::payload_conditions(&payload);
        let active = needs_eval::active_conditions(bundle, &needs, &grants, now);
        self.mood = needs_eval::mood(&active);
        let ids: Vec<u16> = active.iter().map(|m| m.condition_id).collect();
        if ids != self.active {
            let names: Vec<&str> =
                ids.iter().filter_map(|&id| bundle.condition_name(id)).collect();
            let next = needs_eval::next_crossing_tic(bundle, &needs, &grants, now);
            tracing::info!(tic = now, conditions = ?names, mood = self.mood, next_crossing = ?next,
                           "condition band change");
            self.active = ids;
        }
    }

    /// Issue one A→B move and arm the give-up deadline from the trip length.
    fn issue_move(&mut self, bot: &Bot, dest: (i32, i32)) {
        let Some(wolf) = self.wolf else { return };
        if bot.client.move_entity(wolf, dest.0, dest.1).is_err() {
            tracing::error!("engine gone during move");
            return;
        }
        let hops = (dest.0 - self.at.0).abs().max((dest.1 - self.at.1).abs()).max(1) as u64;
        // `hops + 1`: the seed hop promotes the start WITHOUT stepping (ACTIONS.md §Movement),
        // so the first step lands one tics_per_tile after the intent.
        let trip_ms = (hops + 1) * self.speed as u64 * 1000 / TIC_HZ as u64;
        self.deadline = std::time::Instant::now() + Duration::from_millis(trip_ms + 5000);
        self.dest = Some(dest);
        tracing::info!(wolf = format!("{wolf:#010x}"), from = ?self.at, to = ?dest, hops, "trip issued");
    }
}

impl Brain for Wolves {
    async fn on_start(&mut self, bot: &mut Bot) {
        let home_macro = position_macro(tile_to_position(self.home.0, self.home.1));
        // The active radius must cover the whole wander disc, or the wolf walks out of the
        // subscription and its state (and our adoption) goes dark.
        bot.anchor_and_wait("wolves", self.home.0, self.home.1, self.radius as u16 + 2, home_macro, Duration::from_secs(5)).await;

        // F5: the corpus is the def authority — no pinned constants. Kept whole (P4): the
        // bundle also carries the needs registry the eval reads every decision tick.
        match &bot.server_url {
            Some(url) => match fetch_corpus(url).await {
                Ok(bundle) => match resolve_thing_in(&bundle, "wolf") {
                    Ok((def, speed)) => {
                        self.def = def;
                        self.speed = speed;
                        // The kind's authored needs (`&thing.needs`) — thirst is the first.
                        let kind = bundle.thing_object_id("wolf").unwrap_or(0);
                        self.thirst = bundle.thing_needs(kind).first().copied().unwrap_or(0) as u8;
                        tracing::info!(def = format!("{def:#010x}"), speed, thirst_need = self.thirst,
                                       "wolf def + speed + needs resolved from the content corpus");
                        self.bundle = Some(bundle);
                    }
                    Err(err) => {
                        tracing::error!(%err, "wolf def resolution failed — cannot spawn");
                        return;
                    }
                },
                Err(err) => {
                    tracing::error!(%err, "corpus fetch failed — cannot spawn");
                    return;
                }
            },
            None => {
                tracing::error!("no server url captured at login — cannot resolve the wolf def");
                return;
            }
        }

        // Adopt-first: give the zone snapshot a moment to replay an EXISTING minted wolf
        // (a restart re-uses its wolf); `tick` CREATEs one only if nothing shows.
        self.spawn_after = std::time::Instant::now() + Duration::from_secs(3);
        tracing::info!("awaiting an existing minted wolf (CREATE if none appears)");
    }

    fn on_event(&mut self, _bot: &Bot, event: &Event) {
        // The payload sidecar (needs-moodlets P4): buffer the RAW stream per entity — the
        // zone snapshot may fan it before OR after the StateObject the adoption keys on.
        if let Event::PawnParts { entity_reference, payload, .. } = event {
            self.payloads.insert(*entity_reference, payload.clone());
            return;
        }
        let Event::StateObject { entity_reference, definition_reference, tile_x, tile_y, removed, .. } = event
        else {
            return;
        };
        if *removed {
            // `StateGone` is TRUSTWORTHY now — the edge swallows zone-migration deletes
            // (movement-hardening P2, closing first-pawns I4), so a removal that reaches us
            // is a real despawn (or a shard wipe — movement-hardening I2's ghost: driving a
            // deleted id re-materializes it at position 0). Honor it: drop the adoption and
            // fall back to the adopt-first-then-CREATE window, exactly like a fresh start.
            if self.wolf == Some(*entity_reference) {
                tracing::warn!(wolf = format!("{entity_reference:#010x}"), "wolf removed — dropping adoption, re-entering adopt-or-CREATE");
                self.wolf = None;
                self.dest = None;
                self.created = false;
                self.spawn_after = std::time::Instant::now() + Duration::from_secs(3);
            }
            return;
        }
        // F3: adopt the first MINTED pawn with our KIND (variant-agnostic — a wolf is a wolf
        // whichever coat it wears; server ids live in the top band, so a legacy client-minted
        // wolf never matches). A legacy raw-object_id def (pre-packed rows) never equals a
        // packed def, so old rows are simply not adopted — cleaned up at the redeploy step.
        if self.wolf.is_none()
            && def_sans_variant(*definition_reference) == def_sans_variant(self.def)
            && self.def != 0
            && entity_ref_type_id(*entity_reference) == TYPE_PAWN
            && (*entity_reference & 0x00FF_FFFF) >= MINTED_BASE
        {
            self.wolf = Some(*entity_reference);
            self.at = (*tile_x, *tile_y);
            // Thirst-init grace (P4): give the snapshot's payload replay 2 s to show an
            // EXISTING thirst row before minting one — a restart must not refill the wolf.
            self.init_after = std::time::Instant::now() + Duration::from_secs(2);
            self.init_queued = false;
            tracing::info!(wolf = format!("{entity_reference:#010x}"), at = ?self.at, "minted wolf adopted");
            return;
        }
        if self.wolf == Some(*entity_reference) {
            self.at = (*tile_x, *tile_y);
            if let Some(dest) = self.dest {
                if (*tile_x, *tile_y) == dest {
                    tracing::info!(?dest, "trip arrived (authoritative)");
                    self.dest = None;
                }
            }
        }
    }

    fn tick(&mut self, bot: &Bot) {
        if self.wolf.is_none() {
            // No wolf adopted: mint ONE, once, after the adopt-first window (ACTIONS.md
            // §CREATE — the minted id comes back through the zone's state fan-out, F3).
            if !self.created && self.def != 0 && std::time::Instant::now() > self.spawn_after {
                let spawn = self.pick_dest();
                // CREATE is variable-arity (human-pawns F2): def, position, count, payload×count.
                // The wolf carries no payload (its variant rides the def) — count 0.
                let program = vec![PROMOTE, CREATE, self.def, tile_to_position(spawn.0, spawn.1), 0];
                if bot.client.queue(program).is_err() {
                    tracing::error!("engine gone during spawn");
                    return;
                }
                self.created = true;
                tracing::info!(?spawn, def = self.def, "wolf CREATE queued; awaiting the minted id");
            }
            return;
        }
        self.mind_needs(bot);
        match self.dest {
            None => {
                let dest = self.pick_dest();
                self.issue_move(bot, dest);
            }
            Some(dest) if std::time::Instant::now() > self.deadline => {
                // SAFE under chain supersession (movement-hardening F1): the new intent's
                // seed re-stamps the trip-serial, so a still-alive old chain dies at its
                // next hop — a re-issue can no longer duplicate chains. Pick a FRESH dest
                // (the old one may be exactly why the trip stalled).
                tracing::warn!(?dest, "trip deadline passed — superseding with a fresh trip");
                let fresh = self.pick_dest();
                self.issue_move(bot, fresh);
            }
            Some(_) => {}
        }
    }
}
