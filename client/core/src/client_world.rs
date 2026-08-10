//! **The headless client's world model** (shared-simulation P2/P2b) — one object a host drives.
//!
//! `client/core`'s intent doc promises *"one contract for every host"*. [`WorldView`] and
//! [`MoverTrack`] each deliver half of it, and a host that had to wire them together itself would
//! be writing the third copy of the join: which probe feeds the track, when paces get re-derived,
//! what a closing zone drops. This is that join, written once.
//!
//! A host feeds it what the wire said and asks it questions. It owns no transport and no
//! rendering, so `client/npc` and the browser (through `shared/wasm`) drive the identical object.
//!
//! **What it deliberately does not do:** smooth anything. [`ClientWorld::pawn_point`] returns
//! where the pawn IS, which is a step function corrected at every anchor. A viewer that wants
//! that to look pleasant owns the smoothing — the stream's F2 line, and the reason the
//! render-chase stays in webgl.

use crate::movers::MoverTrack;
use crate::world_view::WorldView;
use resonantdust_content::loader::Bundle;
use std::sync::{Arc, Mutex};

/// The model, shared between the engine that folds into it and the handle that reads it.
pub type SharedWorld = Arc<Mutex<ClientWorld>>;

/// The composed world plus every pawn in it.
#[derive(Debug, Default)]
pub struct ClientWorld {
    pub world: WorldView,
    pub movers: MoverTrack,
    /// Payload and need rows, keyed by ENTITY — accepted before a pawn has a position, and for
    /// references that never will have one (I8).
    pub rows: crate::gameplay_rows::GameplayRows,
    /// The corpus every derived answer needs. Held HERE, not passed per call: a `&Bundle`
    /// argument on `pathable` is an invitation for two hosts to pass two different corpora,
    /// which is this stream's defect wearing a parameter.
    corpus: Option<Arc<Bundle>>,
    /// The engine's current tic, stamped into the model on every fold. **The model owns no
    /// estimator**: the engine already has THE `TicEstimate`, with its learned rate, its
    /// poison-streak guards and its re-anchor window, and a second one here would be a second
    /// clock — which is the defect this phase removes. Two earlier attempts got this wrong (an
    /// open-coded anchor tuple, then a duplicate estimator); this holds the ANSWER, not a way to
    /// compute it.
    tic: Option<u16>,
}

impl ClientWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hand core the corpus once. Every derived answer reads it from here afterwards.
    pub fn set_corpus(&mut self, bundle: Arc<Bundle>) {
        self.corpus = Some(bundle);
    }

    pub fn corpus(&self) -> Option<&Bundle> {
        self.corpus.as_deref()
    }

    /// **What tic is it.** `None` until the estimate has anchored — a caller must BAIL on that,
    /// never substitute 0: tic 0 is a real tic, and reading it as "now" makes every lazy need
    /// evaluate as though the world had just begun.
    pub fn now_tic_at(&self, _now_ms: f64) -> Option<u16> {
        self.tic
    }

    /// The engine stamps its estimate here on every fold — see [`ClientWorld::tic`].
    pub fn set_tic(&mut self, tic: Option<u16>) {
        if tic.is_some() {
            self.tic = tic;
        }
    }

    /// The tic the last anchor stated, without extrapolation — what a fold uses, since it is
    /// already running at event receipt.
    pub fn anchored_tic(&self) -> Option<u16> {
        self.tic
    }

    /// **THE fold.** Both engines call this from their single `emit` choke point, so neither host
    /// ever writes its own — the pattern that produced three composed views and two walks.
    pub fn observe_event(&mut self, ev: &crate::api::Event, now_ms: f64) {
        use crate::api::Event as E;
        use resonantdust_codec::object as obj;
        match ev {
            E::StateObject {
                macro_position, entity_reference, definition_reference,
                tile_x, tile_y, sub_x, sub_y, facing, tic, removed,
            } => {
                if *removed {
                    self.movers.forget(*entity_reference);
                    self.rows.forget(*entity_reference);
                    return;
                }
                let point = (
                    *tile_x as f64 + f64::from(*sub_x) / 16.0,
                    *tile_y as f64 + f64::from(*sub_y) / 16.0,
                );
                // A missing corpus must NOT drop the row. The position is the WIRE's truth;
                // only the derived answers (pace, the routed leg) need the corpus, and
                // `WorldView::pathable` already falls back to the unknown-cell law without one.
                // Dropping it here made a headless probe report zero movers while five pawns
                // were streaming — the fold discarding facts it merely could not enrich.
                let bundle = self.corpus.clone();
                self.movers.observe_state(
                    *entity_reference, *macro_position, *definition_reference, point, *facing,
                    *tic, &self.world, bundle.as_deref(),
                );
            }
            E::MoveIntent { entity_reference, tile_x, tile_y, event_tic, .. } => {
                let bundle = self.corpus.clone();
                self.movers.observe_intent(
                    *entity_reference, (*tile_x as f64, *tile_y as f64), *event_tic,
                    &self.world, bundle.as_deref(),
                );
            }
            E::PawnParts { entity_reference, macro_position, payload, .. } => {
                self.rows.observe_payload(*entity_reference, *macro_position, payload.clone());
                self.movers.invalidate_pace(*entity_reference);
            }
            E::PawnNeed { entity_reference, macro_position, need, set_tic } => {
                self.rows.observe_need(*entity_reference, *macro_position, *need, *set_tic);
                self.movers.invalidate_pace(*entity_reference);
            }
            E::ColdTiles { macro_position, layer_id, tiles, .. } => {
                self.world.observe_cold_tiles(*macro_position, *layer_id, tiles);
            }
            E::ColdThings { macro_position, things, .. } => {
                for &kp in things {
                    let cell = ((kp >> 8) & 0xff) as u8;
                    self.world.observe_thing_baseline(
                        *macro_position, cell, obj::kind_pos_ref_kind_reference(kp),
                    );
                }
            }
            E::ColdState {
                macro_position, position_reference, definition_reference, removed, ..
            } => {
                let cell = (obj::position_micro(*position_reference) >> 8) as u8;
                match obj::def_type_id(*definition_reference) {
                    t if t == obj::TYPE_BIOME_TILE => {
                        if *removed {
                            self.world.remove_tile_overlay(*macro_position, cell);
                        } else {
                            self.world.observe_tile_overlay(
                                *macro_position, cell, obj::def_kind_reference(*definition_reference),
                            );
                        }
                    }
                    t if t == obj::TYPE_BIOME_THING => {
                        let kr = if *removed { 0 } else { obj::def_kind_reference(*definition_reference) };
                        self.world.observe_thing(*macro_position, cell, kr);
                    }
                    _ => {}
                }
            }
            E::ZoneClosed { macro_position } => self.close_zone(*macro_position),
            _ => {}
        }
        if let (Some(bundle), Some(now)) = (self.corpus.clone(), self.now_tic_at(now_ms)) {
            self.movers.derive_paces(&bundle, &self.rows, now);
        }
    }

    /// An authoritative pawn row. `false` = rejected as replayed history.
    pub fn observe_state(
        &mut self,
        entity: u32,
        macro_position: u16,
        definition_reference: u32,
        point: (f64, f64),
        facing: u8,
        tic: u16,
    ) -> bool {
        let bundle = self.corpus.clone();
        self.movers.observe_state(
            entity, macro_position, definition_reference, point, facing, tic, &self.world,
            bundle.as_deref(),
        )
    }

    /// A promoted move intent. `false` = rejected as a replay.
    pub fn observe_intent(
        &mut self,
        entity: u32,
        dest: (f64, f64),
        event_tic: u16,
    ) -> bool {
        let bundle = self.corpus.clone();
        self.movers.observe_intent(entity, dest, event_tic, &self.world, bundle.as_deref())
    }

    /// A zone left the subscription. It sends no per-entity delete, so both halves must forget
    /// it — a mover left behind in a closed zone is a ghost that never moves again, and a stale
    /// tile row is a wall that is no longer there.
    pub fn close_zone(&mut self, macro_position: u16) {
        self.world.forget_zone(macro_position);
        self.movers.forget_zone(macro_position);
        self.rows.forget_zone(macro_position);
    }

    /// **Where a pawn is now.** `None` = not tracked.
    ///
    /// Call [`ClientWorld::derive_paces`] first, or an unpaced pawn honestly reports its last
    /// anchor rather than a guessed position.
    pub fn pawn_point(&self, entity: u32, now: u16) -> Option<(f64, f64)> {
        self.movers.point_at(entity, now)
    }

    /// Re-derive any pawn whose pace is unknown, through the shared `move_eval::ground_speed`.
    /// Cheap and idempotent — only pawns whose rows changed evaluate.
    pub fn derive_paces(&mut self, now: u16) {
        let Some(bundle) = self.corpus.clone() else { return };
        self.movers.derive_paces(&bundle, &self.rows, now);
    }

    /// Can a pawn stand on this tile — the composed view through the corpus's own flags.
    pub fn pathable(&self, x: i32, y: i32) -> bool {
        match self.corpus.as_deref() {
            Some(b) => self.world.pathable(b, x, y),
            None => crate::world_view::UNKNOWN_CELL_IS_PATHABLE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus() -> Bundle {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let sources = resonantdust_content::content::read_content_dir(&root).expect("content/");
        resonantdust_content::load(&sources).expect("corpus loads")
    }

    /// The end-to-end contract a host depends on: feed a row and an intent, and the pawn has a
    /// SUBTILE position that advances with the tic. This is the answer `client/npc` could not
    /// give at all, and the one webgl computed for itself.
    #[test]
    fn a_host_can_ask_where_a_pawn_is_between_anchors() {
        let mut cw = ClientWorld::new();
        let bundle = corpus();
        cw.set_corpus(std::sync::Arc::new(corpus()));
        let bunny = bundle.thing_object_id("bunny").expect("bunny kind");

        assert!(cw.observe_state(1, 0, u32::from(bunny), (10.0, 10.0), 0, 100));
        // No payload yet: the pace is unknown, so the pawn HOLDS rather than guessing.
        cw.derive_paces(100);
        assert!(cw.observe_intent(1, (30.0, 10.0), 102));
        assert_eq!(cw.pawn_point(1, 148), Some((10.0, 10.0)), "unpaced pawns do not move");

        // Mint the bunny's payload the way CREATE does, and the pace derives to 24 tics/tile.
        let mut payload: Vec<u32> = Vec::new();
        for b in bundle.thing_traits(bunny) {
            let Some(cat) = bundle.trait_category(&b.name) else { continue };
            let Some(cat_name) = resonantdust_codec::object::gameplay_category(cat) else {
                continue;
            };
            if let Some(r0) = bundle.gameplay_reference(cat_name, &b.name) {
                let r = (r0 & !0xF) | (u32::from(b.variant) & 0xF);
                payload.extend_from_slice(&resonantdust_codec::payload::trait_entry(r, 0));
            }
        }
        cw.rows.observe_payload(1, 0, payload);
        cw.movers.invalidate_pace(1);
        cw.derive_paces(100);
        assert_eq!(cw.movers.get(1).unwrap().pace, Some(24.0));

        let a = cw.pawn_point(1, 100).unwrap();
        let b = cw.pawn_point(1, 148).unwrap();
        assert_eq!(a, (10.0, 10.0));
        assert!(b.0 > a.0, "the pawn advanced: {a:?} -> {b:?}");
        assert!((b.0 - 12.0).abs() < 0.01, "48 tics at 24 t/t is two tiles, got {b:?}");
    }

    /// [I8](../../docs/work/2026-08-09-shared-simulation/issues.md) as a regression test: a
    /// payload that lands BEFORE the pawn's first position row must still reach the pace. The
    /// old shape dropped it and left the pawn held still forever.
    #[test]
    fn rows_before_the_anchor_still_reach_the_pace() {
        let bundle = corpus();
        let mut cw = ClientWorld::new();
        cw.set_corpus(std::sync::Arc::new(corpus()));
        let bunny = bundle.thing_object_id("bunny").expect("bunny kind");
        let mut payload: Vec<u32> = Vec::new();
        for b in bundle.thing_traits(bunny) {
            let Some(cat) = bundle.trait_category(&b.name) else { continue };
            let Some(cn) = resonantdust_codec::object::gameplay_category(cat) else { continue };
            if let Some(r0) = bundle.gameplay_reference(cn, &b.name) {
                let r = (r0 & !0xF) | (u32::from(b.variant) & 0xF);
                payload.extend_from_slice(&resonantdust_codec::payload::trait_entry(r, 0));
            }
        }
        // Payload FIRST — the zone-snapshot order that used to lose it.
        cw.rows.observe_payload(1, 0, payload);
        cw.observe_state(1, 0, u32::from(bunny), (10.0, 10.0), 0, 100);
        cw.derive_paces(100);
        assert_eq!(cw.movers.get(1).unwrap().pace, Some(24.0));
    }

    /// The read surface end-to-end, as a HOST sees it: an event goes through the fold and the
    /// answer comes back off the model. The tick audit unticked P2c item 1 for the absence of
    /// exactly this — the `emit` fold and the anchor-less `None` path had no test at all.
    #[test]
    fn an_event_folded_yields_an_answer_and_no_clock_means_none() {
        use crate::api::Event;
        let mut cw = ClientWorld::new();
        cw.set_corpus(std::sync::Arc::new(corpus()));

        // No TicAnchor yet: the clock has not anchored, so there is NO tic to answer at.
        assert_eq!(cw.now_tic_at(1000.0), None, "an unanchored clock must answer None, not 0");

        // The ENGINE stamps the tic (it owns the only `TicEstimate`); the model holds the
        // answer. Observing a `TicAnchor` here must NOT conjure one — that would be the second
        // clock again, in the model, which is what this design removed.
        cw.observe_event(&Event::TicAnchor { tic: 100, wall_ms: 0.0, tics_per_sec: 6.0 }, 0.0);
        assert_eq!(cw.now_tic_at(0.0), None, "the model must not derive a tic of its own");
        cw.set_tic(Some(100));
        assert_eq!(cw.now_tic_at(0.0), Some(100));

        cw.observe_event(&Event::StateObject {
            macro_position: 0, entity_reference: 1, definition_reference: 0,
            tile_x: 10, tile_y: 10, sub_x: 8, sub_y: 0, facing: 0, tic: 100, removed: false,
        }, 0.0);
        assert_eq!(cw.pawn_point(1, 100), Some((10.5, 10.0)), "subtile survives the fold");

        // And a removal takes both the mover and its rows.
        cw.rows.observe_payload(1, 0, vec![1, 2, 3]);
        cw.observe_event(&Event::StateObject {
            macro_position: 0, entity_reference: 1, definition_reference: 0,
            tile_x: 10, tile_y: 10, sub_x: 0, sub_y: 0, facing: 0, tic: 101, removed: true,
        }, 0.0);
        assert_eq!(cw.pawn_point(1, 101), None);
        assert!(cw.rows.payload(1).is_empty(), "rows must go with the entity");
    }

    /// A closing zone must clear BOTH halves. A mover left in a closed zone is a ghost that
    /// never moves again; a stale tile is a wall that is no longer there.
    #[test]
    fn closing_a_zone_clears_both_halves() {
        let mut cw = ClientWorld::new();
        cw.set_corpus(std::sync::Arc::new(corpus()));
        cw.world.observe_cold_tiles(7, 0, &vec![0x10; 256]);
        cw.observe_state(1, 7, 0, (10.0, 10.0), 0, 100);
        cw.close_zone(7);
        assert!(cw.pawn_point(1, 200).is_none());
        assert_eq!(cw.world.known_zones(), 0);
    }
}
