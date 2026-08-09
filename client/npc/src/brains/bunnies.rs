//! bunnies — the GROUP herbivore brain (food-chain F8): mint/adopt `NPC_BUNNIES`
//! bunnies around a home, then run each on the wolves' pattern — wander, drink from
//! the shore when thirsty, eat PLANT MATTER when hungry (the herbivore gate refuses
//! meat by itself). One brain, many pawns: the per-entity row maps the events already
//! carry (`PawnParts` / `PawnNeed`) key everything, so a bunny costs a `Mind`, not a
//! process. Death (corpus ≤ 0 → the worker's need-write trigger) reaches us as the
//! removal `StateObject` — the mind is dropped, and the warren REPLENISHES: the mint
//! guard re-mints while minds < count, bounded at `count` CREATEs per run (prey respawns;
//! a massacre outpacing the bound thins the pack until restart).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use client::world::tile_to_position;
use resonantdust_codec::action::EXECUTE_INTERACTION;
use resonantdust_codec::object::{def_kind_id, position_macro};
use resonantdust_codec::payload::{payload_conditions, payload_traits};
use resonantdust_codec::speed::DEFAULT_TICS_PER_TILE;
use resonantdust_codec::tic::TIC_HZ;
use resonantdust_content::loader::Bundle;
use resonantdust_content::needs_eval::{self, ActiveCondition};
use resonantdust_content::{path_eval, stat_eval};

use crate::{Bot, Brain, Event, Rng};

/// One bunny's ephemeral drives — everything durable lives in the shards.
struct Mind {
    at: (i32, i32),
    dest: Option<(i32, i32)>,
    deadline: Instant,
    drink_issued: bool,
    eat_issued: bool,
    /// Hold-still window while a DURATION act runs (eat = 20 tics): wandering off
    /// mid-act no-ops the completion (lumberjack F1). Expiry re-arms the eat latch.
    busy_until: Option<Instant>,
    active: Vec<u32>,
    active_set: Vec<ActiveCondition>,
}

pub struct Bunnies {
    rng: Rng,
    home: (i32, i32),
    radius: i32,
    count: usize,
    def: u32,
    kind: u16,
    thirst: u32,
    hunger: u32,
    bundle: Option<Bundle>,
    created: usize,
    spawn_after: Instant,
    minds: HashMap<u32, Mind>,
    payloads: HashMap<u32, Vec<u32>>,
    need_rows: HashMap<u32, HashMap<u32, (u64, u16)>>,
    /// HOSTED mode (npc-host F11): the OWNERSHIP set — only refs the module's player minted
    /// (fed by `Event::OwnedPawn`) are adopted; `None` = the legacy kind-scan (standalone).
    owned: Option<std::collections::HashSet<u32>>,
    /// HOSTED mode: the module's own player-pawn + its count need — the F8 bookkeeping
    /// target (`bunny_count` written on drift).
    group: Option<(u32, u32, f64, f64)>,
    /// The last count written (avoid re-writing an unchanged value every tick).
    last_count: Option<usize>,
    /// The governed pawn KIND (npc-host P3, F6/F11): "bunny" standalone; hosted modules pass
    /// their brain's kind ("wolf" for wolf_pack). The DIET is not a parameter — the corpus's
    /// affordance gates decide what each kind may eat (`usable_eat` asks, never assumes).
    kind_name: String,
}

impl Bunnies {
    pub fn new(rng: Rng) -> Self {
        let home = std::env::var("NPC_HOME")
            .ok()
            .and_then(|s| {
                let mut it = s.split(',').map(|p| p.trim().parse::<i32>());
                Some((it.next()?.ok()?, it.next()?.ok()?))
            })
            .unwrap_or((100, 50));
        let radius =
            std::env::var("NPC_RADIUS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
        let count =
            std::env::var("NPC_BUNNIES").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
        Self {
            rng,
            home,
            radius,
            count,
            def: 0,
            kind: 0,
            thirst: 0,
            hunger: 0,
            bundle: None,
            created: 0,
            // Adopt-first: existing minted bunnies stream in before we mint more. 10 s,
            // not 3 — a cold edge's late snapshot minted a 4th bunny through the 3 s
            // window (spawn-authority P4's cold-boot drill, same race as the wolves').
            spawn_after: Instant::now() + Duration::from_secs(10),
            minds: HashMap::new(),
            payloads: HashMap::new(),
            need_rows: HashMap::new(),
            owned: None,
            group: None,
            last_count: None,
            kind_name: "bunny".to_string(),
        }
    }

    /// HOSTED construction (npc-host F11): explicit area + count (no env), ownership-gated
    /// adoption from the start. `init_with` supplies the corpus the HOST already fetched.
    pub fn hosted(rng: Rng, kind_name: &str, home: (i32, i32), radius: i32, count: usize) -> Self {
        let mut b = Self::new(rng);
        b.kind_name = kind_name.to_string();
        b.home = home;
        b.radius = radius;
        b.count = count;
        b.owned = Some(std::collections::HashSet::new());
        b
    }

    /// Resolve defs from an already-fetched corpus (the hosted twin of `on_start`'s fetch;
    /// the HOST anchored, so no anchor here).
    pub fn init_with(&mut self, bundle: &Bundle) {
        let kind_name = self.kind_name.clone();
        match crate::resolve_thing_in(bundle, &kind_name) {
            Ok(def) => {
                self.def = def;
                self.kind = bundle.thing_object_id(&kind_name).unwrap_or(0);
                self.thirst = bundle.gameplay_reference("need", "thirst").unwrap_or(0);
                self.hunger = bundle.gameplay_reference("need", "hunger").unwrap_or(0);
                self.bundle = Some(bundle.clone());
                tracing::info!(def = format!("{def:#010x}"), count = self.count,
                    "bunny def resolved — the hosted fluffle opens");
            }
            Err(err) => tracing::error!(%err, "hosted bunny def resolution failed"),
        }
    }

    /// HOSTED bookkeeping target (F8): the module's player-pawn + its count need
    /// (`(player_pawn_ref, need_ref, need_min, need_max)`).
    pub fn set_group(&mut self, player_pawn: u32, need_ref: u32, min: f64, max: f64) {
        self.group = Some((player_pawn, need_ref, min, max));
    }

    /// Inside the operational area? (npc-host F3 — the clamp lives in the SENSE/ACT
    /// helpers; legacy standalone brains have area == home±radius too, so one law serves.)
    fn in_area(&self, cell: (i32, i32)) -> bool {
        (cell.0 - self.home.0).abs().max((cell.1 - self.home.1).abs()) <= self.radius
    }

    fn pick_dest(&mut self) -> (i32, i32) {
        let span = (self.radius * 2 + 1) as u32;
        (
            self.home.0 - self.radius + self.rng.below(span) as i32,
            self.home.1 - self.radius + self.rng.below(span) as i32,
        )
    }

    fn rows_of(&self, id: u32) -> (Vec<u64>, Vec<(u64, u16)>, Vec<(u64, u16)>) {
        // trait-lights F5: constant binds derive through THE merged accessor.
        let traits = match &self.bundle {
            Some(b) => b.object_trait_rows(
                def_kind_id(self.def),
                &self.payloads.get(&id).map(|p| payload_traits(p)).unwrap_or_default(),
            ),
            None => self.payloads.get(&id).map(|p| payload_traits(p)).unwrap_or_default(),
        };
        let conds = self.payloads.get(&id).map(|p| payload_conditions(p)).unwrap_or_default();
        let needs =
            self.need_rows.get(&id).map(|m| m.values().copied().collect()).unwrap_or_default();
        (traits, needs, conds)
    }

    /// A need's band is active for this bunny (the wolves' is_thirsty, per member).
    fn band_active(&self, mind: &Mind, need_ref: u32) -> bool {
        let Some(bundle) = &self.bundle else { return false };
        let Some(np) = bundle.need_params_by_ref(need_ref) else { return false };
        np.bands.iter().any(|b| {
            bundle
                .gameplay_reference("condition", &b.condition)
                .is_some_and(|r| mind.active.contains(&r))
        })
    }

    fn pace(&self, id: u32) -> u16 {
        let Some(bundle) = &self.bundle else { return DEFAULT_TICS_PER_TILE };
        let (traits, _, _) = self.rows_of(id);
        let active = self.minds.get(&id).map(|m| m.active_set.clone()).unwrap_or_default();
        let v = stat_eval::stat_value(bundle, "ground_speed", &traits, &active);
        if v >= 1.0 { v.round() as u16 } else { DEFAULT_TICS_PER_TILE }
    }

    fn issue_move(&mut self, bot: &Bot, act: &client::Client, id: u32, dest: (i32, i32)) {
        let Some(bundle) = &self.bundle else { return };
        let Some(iref) = bundle.gameplay_reference("interaction", "move_to") else { return };
        let program = vec![
            EXECUTE_INTERACTION, iref, 0, 2, id, tile_to_position(dest.0, dest.1),
        ];
        if act.queue(program).is_err() {
            return;
        }
        let at = self.minds.get(&id).map(|m| m.at).unwrap_or(dest);
        let pace = u64::from(self.pace(id));
        let pathable =
            |x: i32, y: i32| bot.tile_kind_at((x, y)).is_none_or(|k| bundle.tile_pathable(k));
        let cheb = (dest.0 - at.0).abs().max((dest.1 - at.1).abs()).max(1) as u64;
        let est = path_eval::find_chords(at, dest, 0, &pathable)
            .map(|c| (path_eval::chord_len(at, &c) * pace as f64).ceil() as u64)
            .unwrap_or(cheb * 2 * pace);
        if let Some(m) = self.minds.get_mut(&id) {
            m.dest = Some(dest);
            m.deadline =
                Instant::now() + Duration::from_millis((est + pace) * 1000 / TIC_HZ as u64 + 5000);
        }
    }

    fn fire(&self, bot: &Bot, act: &client::Client, id: u32, interaction: &str, magnitude: f64, dest: (i32, i32)) {
        let Some(bundle) = &self.bundle else { return };
        let (Some(iref), Some(ip)) = (
            bundle.gameplay_reference("interaction", interaction),
            bundle.interaction_params(interaction),
        ) else {
            return;
        };
        let mut inputs = Vec::with_capacity(ip.inputs.len());
        for name in &ip.inputs {
            match name.as_str() {
                "pawn" => inputs.push(id),
                "destination" => {
                    inputs.push(tile_to_position(dest.0, dest.1));
                }
                "amount" => inputs.push((magnitude as f32).to_bits()),
                _ => return,
            }
        }
        let mut program = vec![EXECUTE_INTERACTION, iref, 0, inputs.len() as u32];
        program.extend_from_slice(&inputs);
        let _ = act.queue(program);
    }

    /// The first satisfying interaction `kind` (a THING) offers that this bunny may use.
    fn usable_eat(&self, id: u32, kind: u16, now: u16) -> Option<(String, f64)> {
        let bundle = self.bundle.as_ref()?;
        let (traits, needs, conds) = self.rows_of(id);
        let active = self.minds.get(&id).map(|m| m.active_set.clone()).unwrap_or_default();
        bundle
            .thing_interactions(kind)
            .into_iter()
            .find(|b| {
                bundle.interaction_params(&b.name).is_some_and(|ip| ip.satisfy.is_some())
                    && stat_eval::interaction_available(
                        bundle, &b.name, &traits, &needs, &conds, &active, now,
                    )
            })
            .map(|b| (b.name, b.magnitude))
    }

    /// Same, over TILE kinds (the drink source).
    fn usable_drink(&self, id: u32, kind: u16, now: u16) -> Option<(String, f64)> {
        let bundle = self.bundle.as_ref()?;
        let (traits, needs, conds) = self.rows_of(id);
        let active = self.minds.get(&id).map(|m| m.active_set.clone()).unwrap_or_default();
        bundle
            .tile_interactions(kind)
            .into_iter()
            .find(|b| {
                bundle.interaction_params(&b.name).is_some_and(|ip| ip.satisfy.is_some())
                    && stat_eval::interaction_available(
                        bundle, &b.name, &traits, &needs, &conds, &active, now,
                    )
            })
            .map(|b| (b.name, b.magnitude))
    }

    /// One bunny's think: needs re-eval → drink → eat → wander.
    fn think(&mut self, bot: &Bot, act: &client::Client, id: u32) {
        if self.bundle.is_none() {
            return;
        }
        let Some(now) = bot.now_tic() else { return };
        // Needs re-eval (the wolves' mind_needs, per member).
        {
            let (traits, needs, conds) = self.rows_of(id);
            let Some(bundle) = self.bundle.as_ref() else { return };
            let active = needs_eval::active_conditions(bundle, &traits, &needs, &conds, now);
            let refs: Vec<u32> = active.iter().map(|c| c.condition_id).collect();
            let Some(m) = self.minds.get_mut(&id) else { return };
            if refs != m.active {
                m.active = refs;
                m.drink_issued = false;
                m.eat_issued = false;
            }
            m.active_set = active;
        }
        // A DURATION act in flight: hold still until it completes (the wolves' busy
        // gate) — expiry re-arms the eat latch so a no-opped completion retries.
        if let Some(t) = self.minds[&id].busy_until {
            if Instant::now() < t {
                return;
            }
            if let Some(m) = self.minds.get_mut(&id) {
                m.busy_until = None;
                m.eat_issued = false;
            }
        }
        let at = self.minds[&id].at;
        // Drink (the shore rule): fire on an adjacent water carrier, else walk to the
        // nearest satisfying tile's best pathable neighbor.
        if self.band_active(&self.minds[&id], self.thirst) && !self.minds[&id].drink_issued {
            for oy in -1i32..=1 {
                for ox in -1i32..=1 {
                    let c = (at.0 + ox, at.1 + oy);
                    if let Some(kind) = bot.tile_kind_at(c) {
                        if let Some((i, mag)) = self.usable_drink(id, kind, now) {
                            self.fire(bot, act, id, &i, mag, c);
                            if let Some(m) = self.minds.get_mut(&id) {
                                m.drink_issued = true;
                            }
                            return;
                        }
                    }
                }
            }
            if let Some(t) =
                bot.nearest_tile(at, |kind| self.usable_drink(id, kind, now).is_some())
                    .filter(|c| self.in_area(*c))
            {
                let pathable = |c: (i32, i32)| {
                    bot.tile_kind_at(c).is_none_or(|k| {
                        self.bundle.as_ref().is_none_or(|b| b.tile_pathable(k))
                    })
                };
                let mut best: Option<(i32, i32, i32)> = None;
                for oy in -1i32..=1 {
                    for ox in -1i32..=1 {
                        let c = (t.0 + ox, t.1 + oy);
                        if !pathable(c) {
                            continue;
                        }
                        let d = (c.0 - at.0).abs().max((c.1 - at.1).abs());
                        let cand = (d, c.1, c.0);
                        if best.is_none_or(|b| cand < b) {
                            best = Some(cand);
                        }
                    }
                }
                if let Some((_, sy, sx)) = best {
                    self.issue_move(bot, act, id, (sx, sy));
                    return;
                }
            }
        }
        // Eat plant matter (F8): fire on an adjacent edible, else walk to the nearest.
        if self.band_active(&self.minds[&id], self.hunger) && !self.minds[&id].eat_issued {
            for oy in -1i32..=1 {
                for ox in -1i32..=1 {
                    let c = (at.0 + ox, at.1 + oy);
                    if let Some(kind) = bot.thing_kind_at(c) {
                        if let Some((i, mag)) = self.usable_eat(id, kind, now) {
                            let dur = self
                                .bundle
                                .as_ref()
                                .and_then(|b| b.interaction_params(&i))
                                .map(|ip| ip.duration)
                                .unwrap_or(0.0);
                            self.fire(bot, act, id, &i, mag, c);
                            if let Some(m) = self.minds.get_mut(&id) {
                                m.eat_issued = true;
                                m.busy_until = Some(
                                    Instant::now() + Duration::from_secs_f64(dur / 6.0 + 3.0),
                                );
                            }
                            return;
                        }
                    }
                }
            }
            // Skip food on impathable ground (lake drops) — the worker would refuse the
            // trip forever and the bunny would oscillate (the wolves' cell_open rule).
            if let Some(t) = bot.nearest_thing(at, |cell, kind| {
                if !self.in_area(cell) {
                    return false;
                }
                bot.tile_kind_at(cell)
                    .is_none_or(|k| self.bundle.as_ref().is_none_or(|b| b.tile_pathable(k)))
                    && self.usable_eat(id, kind, now).is_some()
            }) {
                self.issue_move(bot, act, id, t);
                return;
            }
        }
        // Wander (deadline-superseded, the wolves' rule).
        let m = &self.minds[&id];
        if m.dest.is_none() || Instant::now() > m.deadline {
            let d = self.pick_dest();
            self.issue_move(bot, act, id, d);
        }
    }
}

impl Brain for Bunnies {
    async fn on_start(&mut self, bot: &mut Bot) {
        let home_macro = position_macro(tile_to_position(self.home.0, self.home.1));
        bot.anchor_and_wait(
            "bunnies",
            self.home.0,
            self.home.1,
            self.radius as u16 + 2,
            home_macro,
            Duration::from_secs(5),
        )
        .await;
        let Some(url) = bot.server_url.clone() else {
            tracing::error!("no server url — cannot fetch the corpus");
            return;
        };
        match crate::fetch_corpus(&url).await {
            Ok(bundle) => match crate::resolve_thing_in(&bundle, "bunny") {
                Ok(def) => {
                    self.def = def;
                    self.kind = bundle.thing_object_id("bunny").unwrap_or(0);
                    self.thirst = bundle.gameplay_reference("need", "thirst").unwrap_or(0);
                    self.hunger = bundle.gameplay_reference("need", "hunger").unwrap_or(0);
                    tracing::info!(def = format!("{def:#010x}"), count = self.count,
                        "bunny def resolved — the warren opens");
                    self.bundle = Some(bundle);
                }
                Err(err) => tracing::error!(%err, "bunny def resolution failed"),
            },
            Err(err) => tracing::error!(%err, "corpus fetch failed"),
        }
    }

    fn on_event(&mut self, _bot: &Bot, act: &client::Client, event: &Event) {
        match event {
            Event::StateObject {
                entity_reference, definition_reference, tile_x, tile_y, removed, ..
            } => {
                if *removed {
                    if let Some(owned) = &mut self.owned {
                        owned.remove(entity_reference);
                    }
                    if self.minds.remove(entity_reference).is_some() {
                        tracing::info!(bunny = format!("{entity_reference:#010x}"),
                            left = self.minds.len(), "a bunny died — the pack thins");
                    }
                    return;
                }
                if self.kind != 0 && def_kind_id(*definition_reference) == self.kind {
                    // npc-host F11: hosted modules adopt ONLY their own mints.
                    if let Some(owned) = &self.owned {
                        if !owned.contains(entity_reference) {
                            return;
                        }
                    }
                    let at = (*tile_x, *tile_y);
                    self.minds
                        .entry(*entity_reference)
                        .and_modify(|m| {
                            if m.dest == Some(at) {
                                m.dest = None; // arrived
                            }
                            m.at = at;
                        })
                        .or_insert_with(|| {
                            tracing::info!(bunny = format!("{entity_reference:#010x}"), ?at,
                                "bunny adopted");
                            Mind {
                                at,
                                dest: None,
                                deadline: Instant::now(),
                                drink_issued: false,
                                eat_issued: false,
                                busy_until: None,
                                active: Vec::new(),
                                active_set: Vec::new(),
                            }
                        });
                }
            }
            Event::OwnedPawn { entity_reference } => {
                if let Some(owned) = &mut self.owned {
                    owned.insert(*entity_reference);
                }
                // The snapshot race (npc-host P3): a RESTING pawn's StateObject replays
                // once at subscribe and may beat this frame — adopt from the world model's
                // position map so ownership arriving second still lands the mind.
                if !self.minds.contains_key(entity_reference) {
                    if let Some(at) = _bot.pawn_at(*entity_reference) {
                        tracing::info!(pawn = format!("{entity_reference:#010x}"), ?at,
                            "owned pawn adopted from the world model (snapshot race)");
                        self.minds.insert(*entity_reference, Mind {
                            at,
                            dest: None,
                            deadline: Instant::now(),
                            drink_issued: false,
                            eat_issued: false,
                            busy_until: None,
                            active: Vec::new(),
                            active_set: Vec::new(),
                        });
                    }
                }
            }
            Event::PawnParts { entity_reference, payload, .. } => {
                if self.minds.contains_key(entity_reference) || self.payloads.len() < 64 {
                    self.payloads.insert(*entity_reference, payload.clone());
                }
            }
            Event::PawnNeed { entity_reference, need, set_tic, .. } => {
                let key = resonantdust_codec::object::row_reference(*need);
                self.need_rows
                    .entry(*entity_reference)
                    .or_default()
                    .insert(key, (*need, *set_tic));
            }
            _ => {}
        }
    }

    fn tick(&mut self, bot: &Bot, act: &client::Client) {
        if self.bundle.is_none() {
            return;
        }
        // HOSTED bookkeeping (npc-host F11/F8): the group's live count IS a need on the
        // module's player-pawn — written on drift, so mint/death move `bunny_count` and the
        // banded `packless` state derives server-side for free.
        if let Some((pp, nref, min, max)) = self.group {
            let n = self.minds.len();
            if self.last_count != Some(n) {
                let q = resonantdust_codec::value::quantize(n as f32, min as f32, max as f32);
                // The wire splits the row (trait-rows-u32 F1): reference word + data word.
                if act
                    .queue(vec![resonantdust_codec::action::SET_NEED, pp, nref, u32::from(q)])
                    .is_ok()
                {
                    self.last_count = Some(n);
                    tracing::info!(count = n, player_pawn = format!("{pp:#010x}"),
                        "group count written (npc-host F8)");
                }
            }
        }
        // Mint up to `count`, one per tick, after the adopt-first window. Adopted
        // strangers count toward the cap. spawn-authority I2/I6: a REQUEST can be
        // refused invisibly — an unanswered window hands the counter back so the
        // warren re-rolls instead of wedging short.
        // HOSTED mode paces on the OWNERSHIP LEDGER, not the created counter (npc-host
        // F11): `owned` grows only on REAL mints (the edge's Owned fan off the spawn
        // ledger) and prunes on death, so the guard can never double-mint on stream-in
        // lag — the exact race the legacy re-roll papers over (torch-perf I9).
        let hosted_pool = self.owned.as_ref().map(|o| o.len());
        if hosted_pool.is_none()
            && self.created > self.minds.len()
            && Instant::now() > self.spawn_after
            && self.created > 0
        {
            tracing::warn!(created = self.created, adopted = self.minds.len(),
                "spawn request(s) unanswered — re-rolling (spawn-authority I2)");
            self.created = self.minds.len();
        }
        let below_target = match hosted_pool {
            Some(owned) => owned < self.count,
            None => self.minds.len() < self.count && self.created < self.count,
        };
        if below_target
            && self.def != 0
            && Instant::now() > self.spawn_after
        {
            // The PATHABLE picker (attack I2) stays as POLITENESS — the server's gate
            // is the protection now (spawn-authority F2).
            let spawn = {
                let mut s = self.pick_dest();
                for _ in 0..8 {
                    let open = bot.tile_kind_at(s).is_none_or(|k| {
                        self.bundle.as_ref().is_none_or(|b| b.tile_pathable(k))
                    });
                    if open {
                        break;
                    }
                    s = self.pick_dest();
                }
                s
            };
            let program = resonantdust_codec::action::pack_spawn_request(
                spawn.0 as u16,
                spawn.1 as u16,
                0,
                self.def,
                &[],
            );
            if act.queue(program.to_vec()).is_ok() {
                self.created += 1;
                self.spawn_after = Instant::now() + Duration::from_secs(10);
                tracing::info!(?spawn, requested = self.created, "bunny SPAWN_REQUEST queued");
            }
        }
        let ids: Vec<u32> = self.minds.keys().copied().collect();
        for id in ids {
            self.think(bot, act, id);
        }
    }
}
