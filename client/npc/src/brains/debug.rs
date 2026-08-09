//! debug — THE parameterized measurement brain (mover-perf F2, converting torch-perf's
//! `torches`): mint/adopt `NPC_COUNT` pawns of the `NPC_KIND` kind (default
//! `debug_mover`; `debug_torch` for the lit twin) around a home and WANDER THEM
//! FOREVER — a fresh `move_to` on every authoritative arrival (deadline-superseded,
//! the group law). Deliberately NOTHING else: no needs, no drives, no payload eval
//! beyond pace — the brain exists to keep N movers moving while something is measured.
//! The mint/adopt/retry posture is the bunnies' + the torch-perf I9 outstanding gate.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use client::world::tile_to_position;
use resonantdust_codec::action::EXECUTE_INTERACTION;
use resonantdust_codec::object::{def_kind_id, position_macro};
use resonantdust_codec::payload::payload_traits;
use resonantdust_codec::speed::DEFAULT_TICS_PER_TILE;
use resonantdust_codec::tic::TIC_HZ;
use resonantdust_content::loader::Bundle;
use resonantdust_content::{path_eval, stat_eval};

use crate::{Bot, Brain, Event, Rng};

/// One pawn's ephemeral trip state — everything durable lives in the shards.
struct Mind {
    at: (i32, i32),
    dest: Option<(i32, i32)>,
    deadline: Instant,
}

pub struct Debug {
    rng: Rng,
    home: (i32, i32),
    radius: i32,
    count: usize,
    /// The pawn KIND this procession drives (`NPC_KIND`) — any corpus pawn name.
    kind_name: String,
    def: u32,
    kind: u16,
    bundle: Option<Bundle>,
    /// Requests in flight — incremented per SPAWN_REQUEST, decremented (saturating) on
    /// every NEW adoption. The mint gate is `minds + outstanding < count`, which the
    /// bunnies' `created < count` gate is NOT: a process restart zeroed `created` while
    /// pre-existing pawns streamed in slowly, and the two counters raced each other
    /// into an overshoot (found live: 19 mints for a 16 cap — torch-perf I9).
    outstanding: usize,
    spawn_after: Instant,
    minds: HashMap<u32, Mind>,
    payloads: HashMap<u32, Vec<u32>>,
}

impl Debug {
    pub fn new(rng: Rng) -> Self {
        let home = std::env::var("NPC_HOME")
            .ok()
            .and_then(|s| {
                let mut it = s.split(',').map(|p| p.trim().parse::<i32>());
                Some((it.next()?.ok()?, it.next()?.ok()?))
            })
            .unwrap_or((100, 50));
        // The wander radius keeps the whole population inside the zoom-1 window around
        // the home (torch-perf I5) — ±12 tiles default, overridable for other screens.
        let radius =
            std::env::var("NPC_RADIUS").ok().and_then(|s| s.parse().ok()).unwrap_or(12);
        let count =
            std::env::var("NPC_COUNT").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
        let kind_name =
            std::env::var("NPC_KIND").unwrap_or_else(|_| "debug_mover".to_string());
        Self {
            rng,
            home,
            radius,
            count,
            kind_name,
            def: 0,
            kind: 0,
            bundle: None,
            outstanding: 0,
            // Adopt-first (spawn-authority P4's lesson): 10 s for a cold edge's snapshot.
            spawn_after: Instant::now() + Duration::from_secs(10),
            minds: HashMap::new(),
            payloads: HashMap::new(),
        }
    }

    fn pick_dest(&mut self) -> (i32, i32) {
        let span = (self.radius * 2 + 1) as u32;
        (
            self.home.0 - self.radius + self.rng.below(span) as i32,
            self.home.1 - self.radius + self.rng.below(span) as i32,
        )
    }

    /// Pace from the MERGED traits (trait-lights F5) — walks rides the payload rows,
    /// emit_light derives from the def; the eval is the one everything shares.
    fn pace(&self, id: u32) -> u16 {
        let Some(bundle) = &self.bundle else { return DEFAULT_TICS_PER_TILE };
        let rows = bundle.object_trait_rows(
            self.kind,
            &self.payloads.get(&id).map(|p| payload_traits(p)).unwrap_or_default(),
        );
        let v = stat_eval::stat_value(bundle, "ground_speed", &rows, &[]);
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
}

impl Brain for Debug {
    async fn on_start(&mut self, bot: &mut Bot) {
        let home_macro = position_macro(tile_to_position(self.home.0, self.home.1));
        bot.anchor_and_wait(
            "debug",
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
            Ok(bundle) => match crate::resolve_thing_in(&bundle, &self.kind_name) {
                Ok(def) => {
                    self.def = def;
                    self.kind = bundle.thing_object_id(&self.kind_name).unwrap_or(0);
                    tracing::info!(def = format!("{def:#010x}"), kind = %self.kind_name,
                        count = self.count, home = ?self.home, radius = self.radius,
                        "debug kind resolved — the procession starts");
                    self.bundle = Some(bundle);
                }
                Err(err) => tracing::error!(%err, kind = %self.kind_name,
                    "debug kind resolution failed"),
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
                    if self.minds.remove(entity_reference).is_some() {
                        tracing::info!(pawn = format!("{entity_reference:#010x}"),
                            left = self.minds.len(), "a debug pawn left");
                    }
                    return;
                }
                if self.kind != 0 && def_kind_id(*definition_reference) == self.kind {
                    let at = (*tile_x, *tile_y);
                    self.minds
                        .entry(*entity_reference)
                        .and_modify(|m| {
                            if m.dest == Some(at) {
                                m.dest = None; // arrived — tick issues the next trip
                            }
                            m.at = at;
                        })
                        .or_insert_with(|| {
                            tracing::info!(pawn = format!("{entity_reference:#010x}"), ?at,
                                "debug pawn adopted");
                            Mind { at, dest: None, deadline: Instant::now() }
                        });
                    // An adoption retires one in-flight request (saturating — a
                    // pre-existing pawn adopting during the window costs nothing).
                    self.outstanding = self.outstanding.saturating_sub(1);
                }
            }
            Event::PawnParts { entity_reference, payload, .. } => {
                if self.minds.contains_key(entity_reference) || self.payloads.len() < 64 {
                    self.payloads.insert(*entity_reference, payload.clone());
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, bot: &Bot, act: &client::Client) {
        if self.bundle.is_none() {
            return;
        }
        // Mint after the adopt-first window; adopted strangers count toward the cap.
        // The gate is minds + OUTSTANDING < count (I9 — the overshoot fix); a request
        // still unadopted when its window lapses re-rolls (spawn-authority I2/I6).
        if self.outstanding > 0 && Instant::now() > self.spawn_after {
            tracing::warn!(outstanding = self.outstanding, adopted = self.minds.len(),
                "spawn request(s) unanswered — re-rolling (spawn-authority I2)");
            self.outstanding = 0;
        }
        if self.minds.len() + self.outstanding < self.count
            && self.def != 0
            && Instant::now() > self.spawn_after
        {
            // The PATHABLE picker stays as POLITENESS — the server's gate protects.
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
                self.outstanding += 1;
                self.spawn_after = Instant::now() + Duration::from_secs(10);
                tracing::info!(?spawn, outstanding = self.outstanding,
                    "debug SPAWN_REQUEST queued");
            }
        }
        // Wander forever: no dest (arrived) or a blown deadline → the next trip.
        let ids: Vec<u32> = self.minds.keys().copied().collect();
        for id in ids {
            let m = &self.minds[&id];
            if m.dest.is_none() || Instant::now() > m.deadline {
                let d = self.pick_dest();
                self.issue_move(bot, act, id, d);
            }
        }
    }
}
