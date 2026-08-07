//! wolves — the first-pawns v1 brain: spawn ONE server-minted wolf (`CREATE`, def resolved
//! from the content corpus — F5) near a home tile, adopt the minted id from the zone's state
//! fan-out (F3), then wander it with single A→B `MOVE_TO`s — the WORKER chains the hops (F2);
//! this brain only picks destinations and waits for arrivals (or a timeout) before the next.

use std::collections::HashMap;
use std::time::Duration;

use client::world::tile_to_position;
use client::Event;
use resonantdust_codec::action::{CREATE, EXECUTE_INTERACTION, GRANT_CONDITION, PROMOTE, SET_NEED};
use resonantdust_codec::object::{
    def_sans_variant, gameplay_row_key, pack_gameplay_row, position_macro, TYPE_PAWN,
};
use resonantdust_codec::payload::{payload_conditions, payload_traits};
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::speed::DEFAULT_TICS_PER_TILE;
use resonantdust_codec::tic::TIC_HZ;
use resonantdust_codec::value::quantize;
use resonantdust_content::loader::Bundle;
use resonantdust_content::emotion_eval;
use resonantdust_content::needs_eval::{self, ActiveCondition};
use resonantdust_content::stat_eval;

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
    /// The wolf kind's thirst need — a u32 gameplay `definition_reference` (interactions F1;
    /// `0` = kind carries none).
    thirst: u32,
    /// Latest RAW payload per entity — buffered pre-adoption too (the zone snapshot may
    /// fan `Payload` before the `StateObject` this brain adopts on; either order is legal).
    /// TRAIT + CONDITION rows decode from here (stat-model F1) — runtime truth, not the def.
    payloads: HashMap<u32, Vec<u32>>,
    /// The `needs` sub-table rows per entity (stat-model F2): `need_key → (packed, set_tic)`,
    /// fed by the fanned `PawnNeed` frames — a sip lands as exactly one update here.
    need_rows: HashMap<u32, HashMap<u16, (u32, u16)>>,
    /// Thirst-init grace: wait past this before concluding the wolf HAS no thirst row —
    /// the snapshot's payload replay must get its chance, or a restart would reset a
    /// half-drained thirst to full.
    init_after: std::time::Instant,
    /// Single-shot latch for the SET_NEED mint (the splice upserts, so a replay is safe —
    /// the latch only stops US from re-queueing every tick).
    init_queued: bool,
    /// The ACTIVE condition refs at the last evaluation — the transition detector.
    active: Vec<u32>,
    /// The full active set from the last evaluation — the predicate gate's input
    /// (stat-model F5: `interaction_available` reads conditions' stat contributions too).
    active_set: Vec<ActiveCondition>,
    /// The decision context: the ACTIVE emotion's u4 index (emotions F3; 0 = fine).
    pub emotion: u8,
    /// **Drill** (env `NPC_THIRST`, f32 in the need's OWN units — thirst authors `0..100`):
    /// the satisfaction the mint writes instead of full. Thirst depletes over 21600 tics
    /// (1 h wall at 6 Hz), so waiting for a real band crossing is not a test loop —
    /// `NPC_THIRST=12` puts the wolf inside Thirsty at once. Unset = full (the need's `max`).
    thirst_drill: Option<f32>,
    /// **Drill** (env `NPC_GRANT`, comma-separated condition NAMES, e.g. `quenched`): TIMED
    /// grants to queue once after the mint, through `GRANT_CONDITION` — the forcing lane for
    /// the stored-grant machinery and the panel's card strip.
    grant_drill: Vec<String>,
    /// Single-shot latch for `grant_drill`.
    grants_queued: bool,
    // ── the DRINK behaviour (interactions P4) ──
    /// The water tile the wolf is heading to drink at, if thirst drove it there.
    drink_target: Option<(i32, i32)>,
    /// The drink pacing latch: set when a drink event is issued, cleared when the thirst
    /// ROW advances (the sip landed — `set_tic` moved) or the band set changes. The wolf
    /// therefore SIPS sequentially (+3 per landed drink) until the thirst band clears.
    drink_issued: bool,
    /// The thirst row's last seen `set_tic` — the sip-landed detector.
    last_row_tic: Option<u16>,
    /// **Drill** (env `NPC_INTERACT`, an INTERACTION name, e.g. `drink` — carriers bind
    /// interactions directly now, stat-model F9): fire ONE `EXECUTE_INTERACTION` immediately
    /// after init, WHEREVER the wolf stands — the forcing lane for the worker's validation
    /// arm (an off-water fire must be refused and logged).
    interact_drill: Option<String>,
    /// Single-shot latch for `interact_drill`.
    interact_fired: bool,
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
            wolf: None,
            dest: None,
            deadline: std::time::Instant::now(),
            at: home,
            spawn_after: std::time::Instant::now(),
            created: false,
            bundle: None,
            thirst: 0,
            payloads: HashMap::new(),
            need_rows: HashMap::new(),
            init_after: std::time::Instant::now(),
            init_queued: false,
            active: Vec::new(),
            active_set: Vec::new(),
            emotion: 0,
            thirst_drill: std::env::var("NPC_THIRST").ok().and_then(|s| s.trim().parse().ok()),
            grant_drill: std::env::var("NPC_GRANT")
                .ok()
                .map(|s| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
                .unwrap_or_default(),
            grants_queued: false,
            drink_target: None,
            drink_issued: false,
            last_row_tic: None,
            interact_drill: std::env::var("NPC_INTERACT").ok().filter(|s| !s.is_empty()),
            interact_fired: false,
        }
    }

    fn pick_dest(&mut self) -> (i32, i32) {
        let span = (self.radius * 2 + 1) as u32;
        (
            self.home.0 - self.radius + self.rng.below(span) as i32,
            self.home.1 - self.radius + self.rng.below(span) as i32,
        )
    }

    /// A wander dest that is PATHABLE by the bot's tile mirror (pathfinding I7): an
    /// impathable pick (water) would just no-op at the worker (F5) and burn a deadline.
    /// Unknown tiles read OPEN (the shared degrade law); eight rolls then give up —
    /// the worker's gate stays the authority.
    fn pick_pathable_dest(&mut self, bot: &Bot) -> (i32, i32) {
        for _ in 0..8 {
            let d = self.pick_dest();
            let open = bot.tile_kind_at(d).is_none_or(|k| {
                self.bundle.as_ref().is_none_or(|b| b.tile_pathable(k))
            });
            if open {
                return d;
            }
        }
        self.pick_dest()
    }

    /// The needs pass, each decision tick (needs-moodlets P4): mint thirst once on a wolf
    /// that provably has none, then EVALUATE — the lazy read (F4) of the payload's NEED +
    /// CONDITION entries through the shared `needs_eval` (F3). Band transitions are logged
    /// with the tic they were observed at; between crossings this computes and logs
    /// nothing, because nothing changed and nothing was written. Behaviour stays A→B —
    /// the conditions are the DECISION CONTEXT the successor action stream reads.
    /// This wolf's need rows as the eval's `(packed, set_tic)` slice (stat-model F2).
    fn wolf_needs(&self) -> Vec<(u32, u16)> {
        self.wolf
            .and_then(|w| self.need_rows.get(&w))
            .map(|m| m.values().copied().collect())
            .unwrap_or_default()
    }

    fn mind_needs(&mut self, bot: &Bot) {
        let Some(wolf) = self.wolf else { return };
        let Some(bundle) = &self.bundle else { return };
        let payload = self.payloads.get(&wolf).cloned().unwrap_or_default();
        let trait_rows = payload_traits(&payload);
        let cond_rows = payload_conditions(&payload);
        let needs = self.wolf_needs();

        // Mint: no thirst row after the snapshot grace → this wolf was never initialised
        // (CREATE mints needs now — F11 — so this is the drill lane + the re-mint safety
        // net for rows wiped under an adopted wolf).
        if self.thirst != 0 && !self.init_queued && std::time::Instant::now() > self.init_after {
            let key = (self.thirst & 0xFFFF) as u16;
            let has_thirst = needs.iter().any(|&(r, _)| gameplay_row_key(r) == key);
            // A DRILL seed overrides an existing row on purpose — the env is the explicit
            // forcing lane; the no-row grace protects only undrilled runs.
            if !has_thirst || self.thirst_drill.is_some() {
                let np = bundle.need_params_by_ref(self.thirst);
                let full = np.as_ref().map(|np| np.max as f32).unwrap_or(1.0);
                let (lo, hi) = np.as_ref().map(|np| (np.min as f32, np.max as f32)).unwrap_or((0.0, 1.0));
                let sat = self.thirst_drill.unwrap_or(full);
                let row = pack_gameplay_row(self.thirst, quantize(sat, lo, hi));
                if bot.client.queue(vec![SET_NEED, wolf, row]).is_err() {
                    tracing::error!("engine gone during thirst init");
                    return;
                }
                tracing::info!(wolf = format!("{wolf:#010x}"), need = format!("{:#010x}", self.thirst),
                               satisfaction = sat, drill = self.thirst_drill.is_some(),
                               "thirst initialised (no row after the snapshot grace)");
            }
            self.init_queued = true;
        }

        // Drill: queue the requested TIMED grants once (`NPC_GRANT=quenched` etc.). The
        // row carries remaining-at-write = the condition's duration (F3); the RE-STAMP
        // duty is OURS as the composer (F7/I12) — any need the condition modifies gets its
        // current value stamped IN THE SAME program, ahead of the grant.
        // Deferred past the seed's ROUND TRIP: the re-stamp composes from the freshest
        // fanned row, and firing it the same tick as the `NPC_THIRST` seed re-stamps the
        // PRE-seed value over it (seen live, stat-model P4).
        if !self.grants_queued
            && !self.grant_drill.is_empty()
            && self.init_queued
            && std::time::Instant::now() > self.init_after + Duration::from_secs(3)
        {
            let Some(now) = bot.now_tic() else { return };
            for name in &self.grant_drill {
                let (Some(cref), Some(cp)) =
                    (bundle.gameplay_reference("condition", name), bundle.condition_params(name))
                else {
                    tracing::warn!(condition = %name, "grant drill: unknown condition — skipped");
                    continue;
                };
                let mut program = Vec::new();
                for m in &cp.needs {
                    let Some(mref) = bundle.gameplay_reference("need", &m.need) else { continue };
                    let Some(mp) = bundle.need_params_by_ref(mref) else { continue };
                    let mkey = (mref & 0xFFFF) as u16;
                    let Some(&(mrow, mset)) =
                        needs.iter().find(|(r, _)| gameplay_row_key(*r) == mkey)
                    else {
                        continue;
                    };
                    let mval = f64::from(resonantdust_codec::value::dequantize(
                        resonantdust_codec::object::gameplay_row_data(mrow),
                        mp.min as f32,
                        mp.max as f32,
                    ));
                    let windows =
                        needs_eval::rate_windows(bundle, &m.need, &trait_rows, &cond_rows, mset);
                    let msat = needs_eval::satisfaction_at(mval, mset, &mp, now, &windows);
                    program.extend_from_slice(&[
                        SET_NEED,
                        wolf,
                        pack_gameplay_row(mref, quantize(msat as f32, mp.min as f32, mp.max as f32)),
                    ]);
                }
                program.extend_from_slice(&[
                    GRANT_CONDITION,
                    wolf,
                    pack_gameplay_row(cref, cp.duration as u16),
                ]);
                if bot.client.queue(program).is_err() {
                    tracing::error!("engine gone during the grant drill");
                    return;
                }
                tracing::info!(wolf = format!("{wolf:#010x}"), condition = %name,
                               "GRANT_CONDITION queued (drill; re-stamps composed beside it)");
            }
            self.grants_queued = true;
        }

        // Drill: fire ONE EXECUTE_INTERACTION wherever the wolf stands (`NPC_INTERACT=
        // drink`) — the forcing lane for the worker's validation arm: an off-water fire
        // must be REFUSED there and logged, never half-executed.
        if !self.interact_fired && self.init_queued {
            if let Some(name) = self.interact_drill.clone() {
                self.fire_interaction(bot, &name, 3.0, self.at);
                self.interact_fired = true;
            }
        }

        // Evaluate — nothing but the rows, the tic, and the corpus (F3: the ONE eval).
        let Some(now) = bot.now_tic() else { return };
        // Sip-landed detector: the thirst row's `set_tic` advancing means the last drink
        // (or any write) landed — re-arm the latch so a still-thirsty wolf sips again.
        let key = (self.thirst & 0xFFFF) as u16;
        let row_tic = needs.iter().find(|(r, _)| gameplay_row_key(*r) == key).map(|&(_, t)| t);
        if row_tic != self.last_row_tic {
            self.last_row_tic = row_tic;
            self.drink_issued = false;
        }
        let active = needs_eval::active_conditions(bundle, &trait_rows, &needs, &cond_rows, now);
        self.emotion = emotion_eval::active_emotion(bundle, &trait_rows, &active).0;
        let refs: Vec<u32> = active.iter().map(|m| m.condition_id).collect();
        if refs != self.active {
            let names: Vec<String> = refs
                .iter()
                .filter_map(|&r| bundle.gameplay_lookup(r).map(|(_, n)| n))
                .collect();
            let next = needs_eval::next_crossing_tic(bundle, &trait_rows, &needs, &cond_rows, now);
            tracing::info!(tic = now, conditions = ?names, emotion = self.emotion, next_crossing = ?next,
                           "condition band change");
            self.active = refs;
            // A band change re-arms the drink latch (interactions P4: once per crossing).
            self.drink_issued = false;
        }
        self.active_set = active;
    }

    /// Whether the wolf's CURRENT band set says it should drink: any active DERIVED band of
    /// the thirst need (Thirsty / Dehydrated — read from the corpus, never hard-coded).
    fn is_thirsty(&self) -> bool {
        let Some(bundle) = &self.bundle else { return false };
        let Some(np) = bundle.need_params_by_ref(self.thirst) else { return false };
        np.bands.iter().any(|b| {
            bundle
                .gameplay_reference("condition", &b.condition)
                .is_some_and(|r| self.active.contains(&r))
        })
    }

    /// The first interaction tile kind `kind` OFFERS that would SATISFY a need and that
    /// this wolf may use (input-rework I11: with `move_to` on every ground tile, "any
    /// usable interaction" matched GRASS — the drink pass filters to `satisfy` carriers).
    /// Availability is the SAME predicate gate the worker enforces (stat-model I2).
    fn usable_drink(&self, kind: u16, now: u16) -> Option<(String, f64)> {
        let bundle = self.bundle.as_ref()?;
        let wolf = self.wolf?;
        let trait_rows = self.payloads.get(&wolf).map(|p| payload_traits(p)).unwrap_or_default();
        let cond_rows =
            self.payloads.get(&wolf).map(|p| payload_conditions(p)).unwrap_or_default();
        let need_rows = self.wolf_needs();
        bundle
            .tile_interactions(kind)
            .into_iter()
            .find(|b| {
                bundle.interaction_params(&b.name).is_some_and(|ip| ip.satisfy.is_some())
                    && stat_eval::interaction_available(
                        bundle,
                        &b.name,
                        &trait_rows,
                        &need_rows,
                        &cond_rows,
                        &self.active_set,
                        now,
                    )
            })
            .map(|b| (b.name, b.magnitude))
    }

    /// Compose + queue `EXECUTE_INTERACTION` for `interaction`, binding its signature by
    /// the F5 RESERVED vocabulary (`pawn` = this wolf, `amount` = `magnitude` as f32 bits)
    /// — the same rule the pie menu's composer applies. An unbindable name refuses loudly
    /// (input-rework I11: the hardcoded 3-input drink shape mis-fired move_to live).
    fn fire_interaction(&self, bot: &Bot, interaction: &str, magnitude: f64, dest: (i32, i32)) {
        let Some(wolf) = self.wolf else { return };
        let Some(bundle) = &self.bundle else { return };
        let (Some(iref), Some(ip)) = (
            bundle.gameplay_reference("interaction", interaction),
            bundle.interaction_params(interaction),
        ) else {
            tracing::warn!(%interaction, "fire_interaction: unresolvable interaction");
            return;
        };
        let mut inputs = Vec::with_capacity(ip.inputs.len());
        for name in &ip.inputs {
            match name.as_str() {
                "pawn" => inputs.push(wolf),
                // The CARRIER cell (pathfinding: water is impathable, so the wolf
                // drinks FROM THE SHORE — adjacency is cheb ≤ 1 inclusive, and the
                // destination names the water, not where the wolf stands).
                "destination" => inputs
                    .push(resonantdust_codec::object::tile_to_position(dest.0, dest.1)),
                "amount" => inputs.push((magnitude as f32).to_bits()),
                other => {
                    tracing::warn!(%interaction, input = other,
                                   "fire_interaction: unbindable input — not fired");
                    return;
                }
            }
        }
        let mut program = vec![EXECUTE_INTERACTION, iref, 0, inputs.len() as u32];
        program.extend_from_slice(&inputs);
        if bot.client.queue(program).is_err() {
            tracing::error!("engine gone during interaction fire");
            return;
        }
        tracing::info!(wolf = format!("{wolf:#010x}"), %interaction, magnitude, at = ?self.at,
                       "EXECUTE_INTERACTION queued");
    }

    /// The drink pass (interactions P4): when a thirst band is active and unhandled, walk to
    /// the nearest AVAILABLE water and fire the interaction on arrival. Unreachable or
    /// unknown water logs and leaves the wolf to its wander — never a spin.
    fn mind_drink(&mut self, bot: &Bot) {
        if self.thirst == 0 || self.drink_issued || !self.is_thirsty() {
            return;
        }
        let Some(now) = bot.now_tic() else { return };
        // A satisfying carrier within reach already? Drink it. The worker's adjacency
        // law is cheb ≤ 1 INCLUSIVE (lumberjack), and water is IMPATHABLE now
        // (pathfinding): the wolf drinks FROM THE SHORE — the destination names the
        // water cell, never where the wolf stands.
        for oy in -1i32..=1 {
            for ox in -1i32..=1 {
                let c = (self.at.0 + ox, self.at.1 + oy);
                if let Some(kind) = bot.tile_kind_at(c) {
                    if let Some((interaction, magnitude)) = self.usable_drink(kind, now) {
                        self.fire_interaction(bot, &interaction, magnitude, c);
                        self.drink_issued = true;
                        self.drink_target = None;
                        return;
                    }
                }
            }
        }
        // Otherwise head for the SHORE of the nearest satisfying carrier (I11): the best
        // PATHABLE cell in the carrier's 3×3 by the tile mirror — deterministic pick
        // (nearest to the wolf, ties by (y, x)). All-water surroundings = no shore known;
        // leave the wolf to its wander, never a spin.
        let target = bot.nearest_tile(self.at, |kind| self.usable_drink(kind, now).is_some());
        match target {
            Some(t) => {
                if self.drink_target != Some(t) || self.dest.is_none() {
                    let pathable = |c: (i32, i32)| {
                        bot.tile_kind_at(c).is_none_or(|k| {
                            self.bundle.as_ref().is_none_or(|b| b.tile_pathable(k))
                        })
                    };
                    let mut best: Option<(i32, i32, i32)> = None; // (cheb, y, x)
                    for oy in -1i32..=1 {
                        for ox in -1i32..=1 {
                            let c = (t.0 + ox, t.1 + oy);
                            if !pathable(c) {
                                continue;
                            }
                            let d = (c.0 - self.at.0).abs().max((c.1 - self.at.1).abs());
                            let cand = (d, c.1, c.0);
                            if best.is_none_or(|b| cand < b) {
                                best = Some(cand);
                            }
                        }
                    }
                    let Some((_, sy, sx)) = best else {
                        tracing::debug!(water = ?t, "thirsty but the water has no known shore");
                        return;
                    };
                    tracing::info!(water = ?t, shore = ?(sx, sy), from = ?self.at,
                                   "thirsty — heading to the shore");
                    self.drink_target = Some(t);
                    self.issue_move(bot, (sx, sy));
                }
            }
            None => {
                tracing::debug!("thirsty but no known water tile in the streamed zones");
            }
        }
    }

    /// The wolf's pace in tics/tile: the DERIVED `ground_speed` from its payload rows +
    /// active conditions (input-rework F8 — the `speed` field is gone). The codec default
    /// covers only the pre-fan window.
    fn derived_speed(&self) -> u16 {
        let Some(bundle) = &self.bundle else { return DEFAULT_TICS_PER_TILE };
        let rows = self
            .wolf
            .and_then(|w| self.payloads.get(&w))
            .map(|p| payload_traits(p))
            .unwrap_or_default();
        let v = stat_eval::stat_value(bundle, "ground_speed", &rows, &self.active_set);
        if v >= 1.0 { v.round() as u16 } else { DEFAULT_TICS_PER_TILE }
    }

    /// Issue one A→B move — through the `move_to` INTERACTION (input-rework F2/F3: the raw
    /// MOVE_TO door is closed; this is the same event the pie menu composes) — and arm the
    /// give-up deadline from the trip length.
    fn issue_move(&mut self, bot: &Bot, dest: (i32, i32)) {
        let Some(wolf) = self.wolf else { return };
        let Some(bundle) = &self.bundle else { return };
        let Some(iref) = bundle.gameplay_reference("interaction", "move_to") else {
            tracing::error!("move_to is not in the corpus — cannot move");
            return;
        };
        // move_to's signature is (pawn, destination) — inputs bind IN ORDER (F5).
        let program = vec![
            EXECUTE_INTERACTION, iref, 0, 2, wolf, tile_to_position(dest.0, dest.1),
        ];
        if bot.client.queue(program).is_err() {
            tracing::error!("engine gone during move");
            return;
        }
        // The trip time is the SHARED CHORD schedule's (chord-movement F6, superseding
        // pathfinding I7's hop count): `ceil(chord_len × tics_per_tile)` over the bot's
        // TILE mirror — Euclidean, exactly the worker's arithmetic. The bot holds no
        // THING occupancy (tiles-only; the thing-aware npc view is a recorded
        // successor), so tree detours still read optimistic; unknown tiles read OPEN;
        // no route → cheb × 2 hop-times, the honest guess.
        let pace = u64::from(self.derived_speed());
        let cheb = (dest.0 - self.at.0).abs().max((dest.1 - self.at.1).abs()).max(1) as u64;
        let pathable =
            |x: i32, y: i32| bot.tile_kind_at((x, y)).is_none_or(|k| bundle.tile_pathable(k));
        let est_tics = resonantdust_content::path_eval::find_chords(self.at, dest, 0, &pathable)
            .map(|c| {
                (resonantdust_content::path_eval::chord_len(self.at, &c) * pace as f64).ceil()
                    as u64
            })
            .unwrap_or(cheb * 2 * pace);
        // `+ pace`: the seed slot — the first hop lands one stride after the intent.
        let trip_ms = (est_tics + pace) * 1000 / TIC_HZ as u64;
        self.deadline = std::time::Instant::now() + Duration::from_millis(trip_ms + 5000);
        self.dest = Some(dest);
        tracing::info!(wolf = format!("{wolf:#010x}"), from = ?self.at, to = ?dest, est_tics, "trip issued");
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
                    Ok(def) => {
                        self.def = def;
                        // The kind's authored needs (`needs = [...]`) — thirst is the first.
                        // Trait TRUTH is the pawn's payload rows (stat-model F11 — minted at
                        // CREATE); the def's bindings are logged for the boot record only.
                        // Speed is DERIVED per trip (input-rework F8), never resolved here.
                        let kind = bundle.thing_object_id("wolf").unwrap_or(0);
                        self.thirst = bundle.thing_needs(kind).first().copied().unwrap_or(0);
                        tracing::info!(def = format!("{def:#010x}"),
                                       thirst_need = format!("{:#010x}", self.thirst),
                                       def_traits = ?bundle.thing_traits(kind),
                                       "wolf def + needs resolved from the corpus");
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
        // The needs sub-table fan (stat-model F2): one row per frame, keyed by the row's
        // low 16 — a sip lands as exactly one update here.
        if let Event::PawnNeed { entity_reference, need, set_tic, .. } = event {
            self.need_rows
                .entry(*entity_reference)
                .or_default()
                .insert(gameplay_row_key(*need), (*need, *set_tic));
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
        self.mind_drink(bot);
        match self.dest {
            None => {
                // A drink trip owns the destination slot — the wander yields to it
                // (mind_drink re-issues the water trip on its own).
                if self.drink_target.is_none() {
                    let dest = self.pick_pathable_dest(bot);
                    self.issue_move(bot, dest);
                }
            }
            Some(dest) if std::time::Instant::now() > self.deadline => {
                // SAFE under chain supersession (movement-hardening F1): the new intent's
                // seed re-stamps the trip-serial, so a still-alive old chain dies at its
                // next hop — a re-issue can no longer duplicate chains. Pick a FRESH dest
                // (the old one may be exactly why the trip stalled).
                tracing::warn!(?dest, "trip deadline passed — superseding with a fresh trip");
                let fresh = self.pick_pathable_dest(bot);
                self.issue_move(bot, fresh);
            }
            Some(_) => {}
        }
    }
}
