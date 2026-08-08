//! torches — the torch-perf measurement brain (torch-perf F2): mint/adopt
//! `NPC_TORCHES` debug_torch pawns around a home and WANDER THEM FOREVER — a fresh
//! `move_to` on every authoritative arrival (deadline-superseded, the group law).
//! Deliberately NOTHING else: no needs, no drives, no payload eval beyond pace — the
//! brain exists to keep N moving hot lights moving while the client is measured
//! ([I2](../../../../docs/work/2026-08-08-torch-perf/issues.md#i2): an idle hot light
//! still re-bakes, but the user asked for the MOVING case). The mint/adopt/retry
//! posture is the bunnies' verbatim (spawn-authority I2/I6).

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

/// One torch pawn's ephemeral trip state — everything durable lives in the shards.
struct Mind {
    at: (i32, i32),
    dest: Option<(i32, i32)>,
    deadline: Instant,
}

pub struct Torches {
    rng: Rng,
    home: (i32, i32),
    radius: i32,
    count: usize,
    def: u32,
    kind: u16,
    bundle: Option<Bundle>,
    created: usize,
    spawn_after: Instant,
    minds: HashMap<u32, Mind>,
    payloads: HashMap<u32, Vec<u32>>,
}

impl Torches {
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
            std::env::var("NPC_TORCHES").ok().and_then(|s| s.parse().ok()).unwrap_or(8);
        Self {
            rng,
            home,
            radius,
            count,
            def: 0,
            kind: 0,
            bundle: None,
            created: 0,
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

    fn issue_move(&mut self, bot: &Bot, id: u32, dest: (i32, i32)) {
        let Some(bundle) = &self.bundle else { return };
        let Some(iref) = bundle.gameplay_reference("interaction", "move_to") else { return };
        let program = vec![
            EXECUTE_INTERACTION, iref, 0, 2, id, tile_to_position(dest.0, dest.1),
        ];
        if bot.client.queue(program).is_err() {
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

impl Brain for Torches {
    async fn on_start(&mut self, bot: &mut Bot) {
        let home_macro = position_macro(tile_to_position(self.home.0, self.home.1));
        bot.anchor_and_wait(
            "torches",
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
            Ok(bundle) => match crate::resolve_thing_in(&bundle, "debug_torch") {
                Ok(def) => {
                    self.def = def;
                    self.kind = bundle.thing_object_id("debug_torch").unwrap_or(0);
                    tracing::info!(def = format!("{def:#010x}"), count = self.count,
                        home = ?self.home, radius = self.radius,
                        "debug_torch def resolved — the procession lights");
                    self.bundle = Some(bundle);
                }
                Err(err) => tracing::error!(%err, "debug_torch def resolution failed"),
            },
            Err(err) => tracing::error!(%err, "corpus fetch failed"),
        }
    }

    fn on_event(&mut self, _bot: &Bot, event: &Event) {
        match event {
            Event::StateObject {
                entity_reference, definition_reference, tile_x, tile_y, removed, ..
            } => {
                if *removed {
                    if self.minds.remove(entity_reference).is_some() {
                        tracing::info!(torch = format!("{entity_reference:#010x}"),
                            left = self.minds.len(), "a torch went out");
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
                            tracing::info!(torch = format!("{entity_reference:#010x}"), ?at,
                                "torch adopted");
                            Mind { at, dest: None, deadline: Instant::now() }
                        });
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

    fn tick(&mut self, bot: &Bot) {
        if self.bundle.is_none() {
            return;
        }
        // Mint up to `count`, one per tick, after the adopt-first window; adopted
        // strangers count toward the cap; an unanswered request re-rolls
        // (spawn-authority I2/I6 — the bunnies' posture verbatim).
        if self.created > self.minds.len()
            && Instant::now() > self.spawn_after
            && self.created > 0
        {
            tracing::warn!(created = self.created, adopted = self.minds.len(),
                "spawn request(s) unanswered — re-rolling (spawn-authority I2)");
            self.created = self.minds.len();
        }
        if self.minds.len() < self.count
            && self.created < self.count
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
            if bot.client.queue(program.to_vec()).is_ok() {
                self.created += 1;
                self.spawn_after = Instant::now() + Duration::from_secs(10);
                tracing::info!(?spawn, requested = self.created, "torch SPAWN_REQUEST queued");
            }
        }
        // Wander forever: no dest (arrived) or a blown deadline → the next trip.
        let ids: Vec<u32> = self.minds.keys().copied().collect();
        for id in ids {
            let m = &self.minds[&id];
            if m.dest.is_none() || Instant::now() > m.deadline {
                let d = self.pick_dest();
                self.issue_move(bot, id, d);
            }
        }
    }
}
