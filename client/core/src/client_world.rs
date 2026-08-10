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

/// The composed world plus every pawn in it.
#[derive(Debug, Default)]
pub struct ClientWorld {
    pub world: WorldView,
    pub movers: MoverTrack,
}

impl ClientWorld {
    pub fn new() -> Self {
        Self::default()
    }

    /// An authoritative pawn row. `false` = rejected as replayed history.
    pub fn observe_state(
        &mut self,
        bundle: &Bundle,
        entity: u32,
        macro_position: u16,
        definition_reference: u32,
        point: (f64, f64),
        facing: u8,
        tic: u16,
    ) -> bool {
        self.movers.observe_state(
            entity, macro_position, definition_reference, point, facing, tic, &self.world, bundle,
        )
    }

    /// A promoted move intent. `false` = rejected as a replay.
    pub fn observe_intent(
        &mut self,
        bundle: &Bundle,
        entity: u32,
        dest: (f64, f64),
        event_tic: u16,
    ) -> bool {
        self.movers.observe_intent(entity, dest, event_tic, &self.world, bundle)
    }

    /// A zone left the subscription. It sends no per-entity delete, so both halves must forget
    /// it — a mover left behind in a closed zone is a ghost that never moves again, and a stale
    /// tile row is a wall that is no longer there.
    pub fn close_zone(&mut self, macro_position: u16) {
        self.world.forget_zone(macro_position);
        self.movers.forget_zone(macro_position);
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
    pub fn derive_paces(&mut self, bundle: &Bundle, now: u16) {
        self.movers.derive_paces(bundle, now);
    }

    /// Can a pawn stand on this tile — the composed view through the corpus's own flags.
    pub fn pathable(&self, bundle: &Bundle, x: i32, y: i32) -> bool {
        self.world.pathable(bundle, x, y)
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
        let bundle = corpus();
        let mut cw = ClientWorld::new();
        let bunny = bundle.thing_object_id("bunny").expect("bunny kind");

        assert!(cw.observe_state(&bundle, 1, 0, u32::from(bunny), (10.0, 10.0), 0, 100));
        // No payload yet: the pace is unknown, so the pawn HOLDS rather than guessing.
        cw.derive_paces(&bundle, 100);
        assert!(cw.observe_intent(&bundle, 1, (30.0, 10.0), 102));
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
        cw.movers.observe_payload(1, payload);
        cw.derive_paces(&bundle, 100);
        assert_eq!(cw.movers.get(1).unwrap().pace, Some(24.0));

        let a = cw.pawn_point(1, 100).unwrap();
        let b = cw.pawn_point(1, 148).unwrap();
        assert_eq!(a, (10.0, 10.0));
        assert!(b.0 > a.0, "the pawn advanced: {a:?} -> {b:?}");
        assert!((b.0 - 12.0).abs() < 0.01, "48 tics at 24 t/t is two tiles, got {b:?}");
    }

    /// A closing zone must clear BOTH halves. A mover left in a closed zone is a ghost that
    /// never moves again; a stale tile is a wall that is no longer there.
    #[test]
    fn closing_a_zone_clears_both_halves() {
        let bundle = corpus();
        let mut cw = ClientWorld::new();
        cw.world.observe_cold_tiles(7, 0, &vec![0x10; 256]);
        cw.observe_state(&bundle, 1, 7, 0, (10.0, 10.0), 0, 100);
        cw.close_zone(7);
        assert!(cw.pawn_point(1, 200).is_none());
        assert_eq!(cw.world.known_zones(), 0);
    }
}
