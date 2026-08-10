//! The **mover track** — where every pawn IS, at any tic (shared-simulation P2).
//!
//! The Rust client's answer to "where is that pawn". Before this, nobody in Rust had one: the
//! worker resolved positions privately for its own use, `client/webgl` re-derived them in
//! TypeScript for the screen, and `client/npc` kept only each pawn's last authoritative WHOLE
//! TILE — so every headless brain reasoned about positions that were tile-granular and up to one
//! chord stride (8 tiles) stale, and the only code that closed the gap ran in a browser tab.
//!
//! The track is deliberately thin. It holds what the wire already tells it — the last
//! authoritative point and the tic it was stamped, plus the destination of the pawn's live
//! movement intent — and asks [`resonantdust_content::move_eval::position_at`] for everything
//! else. **It computes no motion of its own**: that is the entire point of the module it calls.
//!
//! Two guards are ported from the TypeScript, which earned them live and would otherwise have
//! taken them to the grave (`MoverLayer.ts` :754-776, :839):
//!
//!   - **An older row never applies.** A zone re-subscribe replays STATE history, and a stale row
//!     landing after a fresh one drags the anchor backwards.
//!   - **A replayed intent never arms.** The same replay re-delivers minutes-old `MOVE_TO`s. A
//!     LIVE intent trails the newest row by only the queue barrier; a replay trails it by
//!     hundreds, so an intent far enough behind the freshest row is history, not an order.

use std::collections::HashMap;

/// An intent this many tics behind the pawn's freshest authoritative row is a zone re-subscribe
/// replaying history, not a live order. A live one trails by roughly the queue barrier (~4-5).
const INTENT_STALE_BEHIND_AUTH_TICS: i32 = 16;

/// Serial u16 comparison on the wrapping tic ring: is `a` strictly newer than `b`?
fn tic_newer(a: u16, b: u16) -> bool {
    ((a.wrapping_sub(b)) as i16) > 0
}

/// Signed distance from `b` forward to `a` on the tic ring.
fn tic_delta(a: u16, b: u16) -> i32 {
    i32::from((a.wrapping_sub(b)) as i16)
}

/// One tracked pawn.
#[derive(Debug, Clone)]
pub struct Mover {
    /// The last authoritative point, in fractional tiles.
    pub point: (f64, f64),
    /// The tic [`Mover::point`] was stamped — `position_at` measures elapsed time from here.
    pub tic: u16,
    /// The live movement intent's destination, if the pawn is walking.
    pub dest: Option<(f64, f64)>,
    /// The pawn's pace in tics per tile — its DERIVED `ground_speed`. `None` until the caller
    /// knows it: a pawn whose pace is unknown is held STILL rather than moved at a guess, because
    /// a default pace is an 8x error on the pawns it is wrong about.
    pub pace: Option<f64>,
    pub macro_position: u16,
    pub definition_reference: u32,
    pub facing: u8,
    /// The event tic of the intent currently armed — the replay guard's comparison point.
    intent_tic: Option<u16>,
    /// The first chord's leg from [`Mover::point`], and its pathable fraction — computed ONCE
    /// when the anchor or the intent changes, not per frame. `point_at` then walks it through
    /// the shared [`resonantdust_content::move_eval::advance_along`], so the cheap per-frame
    /// path and the full pathfinding path are the same arithmetic rather than two versions of it.
    leg: Option<((f64, f64), f64)>,
    /// The pawn's raw payload opcode stream — trait and condition rows the pace derives from.
    payload: Vec<u32>,
    /// The pawn's `needs` sub-table rows, `(row, set_tic)`.
    needs: Vec<(u64, u16)>,
}

impl Mover {
    /// Is this pawn walking — a live destination it has not yet reached?
    pub fn walking(&self) -> bool {
        self.dest.is_some()
    }
}

/// Every pawn the client knows about, and where each one is.
#[derive(Debug, Default)]
pub struct MoverTrack {
    movers: HashMap<u32, Mover>,
}

impl MoverTrack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, entity: u32) -> Option<&Mover> {
        self.movers.get(&entity)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &Mover)> {
        self.movers.iter().map(|(&e, m)| (e, m))
    }

    pub fn len(&self) -> usize {
        self.movers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.movers.is_empty()
    }

    /// An authoritative `StateObject` row: the anchor truth. Returns `false` when the row was
    /// REJECTED as replayed history — a strictly older row for a pawn we already have.
    ///
    /// Reaching the intent's destination clears it: the pawn has arrived and must stop being
    /// interpolated, or it walks on past its goal forever.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_state(
        &mut self,
        entity: u32,
        macro_position: u16,
        definition_reference: u32,
        point: (f64, f64),
        facing: u8,
        tic: u16,
        pathable: &dyn Fn(i32, i32) -> bool,
    ) -> bool {
        if let Some(m) = self.movers.get(&entity) {
            if tic_newer(m.tic, tic) {
                return false;
            }
        }
        let entry = self.movers.entry(entity).or_insert(Mover {
            point,
            tic,
            dest: None,
            pace: None,
            macro_position,
            definition_reference,
            facing,
            intent_tic: None,
            leg: None,
            payload: Vec::new(),
            needs: Vec::new(),
        });
        entry.point = point;
        entry.tic = tic;
        entry.macro_position = macro_position;
        entry.definition_reference = definition_reference;
        entry.facing = facing;
        if let Some(dest) = entry.dest {
            if tile_of(dest) == tile_of(point) {
                entry.dest = None;
            }
        }
        // Re-chord from the fresh authoritative point — the worker's own per-hop recompute,
        // mirrored at the anchor cadence rather than guessed at between them.
        entry.leg = entry
            .dest
            .and_then(|d| resonantdust_content::move_eval::first_leg(point, tile_of(d), pathable));
        true
    }

    /// A promoted `MoveIntent`. Returns `false` when the intent was REJECTED as a replay —
    /// serially older than the one already armed, or further behind the pawn's freshest
    /// authoritative row than the queue barrier can explain.
    pub fn observe_intent(
        &mut self,
        entity: u32,
        dest: (f64, f64),
        event_tic: u16,
        pathable: &dyn Fn(i32, i32) -> bool,
    ) -> bool {
        let Some(m) = self.movers.get_mut(&entity) else {
            // No anchor yet — the intent has nothing to walk FROM. Dropping it is safe: the
            // pawn's first authoritative row carries its position, and the next intent arms.
            return false;
        };
        if let Some(prev) = m.intent_tic {
            if tic_delta(event_tic, prev) <= 0 {
                return false;
            }
        }
        if tic_delta(m.tic, event_tic) > INTENT_STALE_BEHIND_AUTH_TICS {
            return false;
        }
        m.intent_tic = Some(event_tic);
        m.dest = Some(dest);
        m.leg = resonantdust_content::move_eval::first_leg(m.point, tile_of(dest), pathable);
        true
    }

    /// A pawn's payload sidecar row arrived — the trait/condition rows its pace derives from.
    pub fn observe_payload(&mut self, entity: u32, payload: Vec<u32>) {
        if let Some(m) = self.movers.get_mut(&entity) {
            m.payload = payload;
            m.pace = None; // re-derive: a trait change can move the pace
        }
    }

    /// One `needs` sub-table row. Conditions banded off needs narrow the pace, so a sip can
    /// change how fast a pawn walks.
    pub fn observe_need(&mut self, entity: u32, need: u64, set_tic: u16) {
        if let Some(m) = self.movers.get_mut(&entity) {
            let key = need % 0x1_0000_0000;
            match m.needs.iter_mut().find(|(r, _)| r % 0x1_0000_0000 == key) {
                Some(slot) => *slot = (need, set_tic),
                None => m.needs.push((need, set_tic)),
            }
            m.pace = None;
        }
    }

    /// Derive every unpaced mover's `ground_speed` through the SHARED
    /// [`resonantdust_content::move_eval::ground_speed`] — the same call the worker spaces its
    /// hops with. Cheap and idempotent: only pawns whose rows changed re-evaluate.
    ///
    /// A pawn deriving below 1 tic/tile keeps `pace: None` and is held still. There is no default
    /// here on purpose ([I2]): a guessed pace is an 8x error on the pawns it is wrong about.
    pub fn derive_paces(&mut self, bundle: &resonantdust_content::loader::Bundle, now: u16) {
        for m in self.movers.values_mut() {
            if m.pace.is_some() {
                continue;
            }
            let kind = resonantdust_content::move_eval::def_kind_id(m.definition_reference);
            let v = resonantdust_content::move_eval::ground_speed(
                bundle, kind, &m.payload, &m.needs, now,
            );
            if v >= 1.0 {
                m.pace = Some(v.round());
            }
        }
    }

    /// Drop a pawn — a `StateGone`, a removal, or a zone leaving the subscription.
    pub fn forget(&mut self, entity: u32) {
        self.movers.remove(&entity);
    }

    /// Drop every pawn in a zone that left the subscription (no per-entity delete arrives).
    pub fn forget_zone(&mut self, macro_position: u16) {
        self.movers.retain(|_, m| m.macro_position != macro_position);
    }

    /// **Where the pawn is at `now`.** The anchor point when it is not walking, or its
    /// interpolated position along the shared walk when it is.
    ///
    /// `None` = we do not track this pawn. A tracked pawn always yields a point, even if that
    /// point is simply its last anchor — a caller must never have to guess.
    pub fn point_at(&self, entity: u32, now: u16) -> Option<(f64, f64)> {
        let m = self.movers.get(&entity)?;
        let (Some((leg, clear)), Some(pace)) = (m.leg, m.pace) else { return Some(m.point) };
        Some(resonantdust_content::move_eval::advance_along(
            m.point, leg, clear, m.tic, now, pace,
        ))
    }
}

/// The tile a fractional point sits in — the shared floor rule.
fn tile_of(p: (f64, f64)) -> (i32, i32) {
    resonantdust_content::move_eval::tile_of(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(_x: i32, _y: i32) -> bool {
        true
    }

    fn track_with_pawn() -> MoverTrack {
        let mut t = MoverTrack::new();
        assert!(t.observe_state(1, 0, 0, (10.0, 10.0), 0, 100, &open));
        t.movers.get_mut(&1).unwrap().pace = Some(24.0);
        t
    }

    /// The whole reason the track exists: between anchors a walking pawn has a position, and it
    /// is a SUBTILE one. This is what `client/npc` could not answer.
    #[test]
    fn a_walking_pawn_moves_between_anchors() {
        let mut t = track_with_pawn();
        assert!(t.observe_intent(1, (30.0, 10.0), 102, &open));
        let at_start = t.point_at(1, 100).unwrap();
        let mid = t.point_at(1, 112).unwrap();
        assert_eq!(at_start, (10.0, 10.0));
        assert!(mid.0 > 10.4 && mid.0 < 10.6, "12 tics at 24 t/t is half a tile, got {mid:?}");
    }

    /// A pawn whose pace we do not know yet is held STILL, not moved at a default. A default is
    /// an 8x error on the pawns it is wrong about (shared-simulation I2).
    #[test]
    fn an_unpaced_pawn_holds_position() {
        let mut t = MoverTrack::new();
        t.observe_state(1, 0, 0, (10.0, 10.0), 0, 100, &open);
        t.observe_intent(1, (30.0, 10.0), 102, &open);
        assert_eq!(t.point_at(1, 400), Some((10.0, 10.0)));
    }

    /// A zone re-subscribe replays STATE history; a stale row applying after a fresh one drags
    /// the anchor backwards, which reads downstream as a teleport.
    #[test]
    fn an_older_row_is_rejected() {
        let mut t = track_with_pawn();
        assert!(t.observe_state(1, 0, 0, (12.0, 10.0), 0, 132, &open));
        assert!(!t.observe_state(1, 0, 0, (10.0, 10.0), 0, 100, &open));
        assert_eq!(t.get(1).unwrap().point, (12.0, 10.0));
    }

    /// The same replay re-delivers minutes-old intents. A live one trails the freshest row by the
    /// queue barrier; a replay trails it by hundreds.
    #[test]
    fn a_replayed_intent_is_rejected() {
        let mut t = track_with_pawn();
        assert!(!t.observe_intent(1, (30.0, 10.0), 100u16.wrapping_sub(500), &open));
        assert!(!t.get(1).unwrap().walking());
        // ...while an intent inside the barrier arms normally.
        assert!(t.observe_intent(1, (30.0, 10.0), 104, &open));
        assert!(t.get(1).unwrap().walking());
        // ...and a serially older one no longer supersedes it.
        assert!(!t.observe_intent(1, (5.0, 10.0), 103, &open));
    }

    /// Arrival clears the intent. Without this the pawn keeps being interpolated toward a
    /// destination it is standing on.
    #[test]
    fn arriving_clears_the_walk() {
        let mut t = track_with_pawn();
        t.observe_intent(1, (12.0, 10.0), 102, &open);
        assert!(t.get(1).unwrap().walking());
        t.observe_state(1, 0, 0, (12.0, 10.0), 1, 150, &open);
        assert!(!t.get(1).unwrap().walking());
        assert_eq!(t.point_at(1, 900), Some((12.0, 10.0)));
    }

    /// A zone leaving the subscription sends no per-entity delete.
    #[test]
    fn a_closed_zone_drops_its_pawns() {
        let mut t = MoverTrack::new();
        t.observe_state(1, 7, 0, (10.0, 10.0), 0, 100, &open);
        t.observe_state(2, 8, 0, (20.0, 20.0), 0, 100, &open);
        t.forget_zone(7);
        assert!(t.get(1).is_none());
        assert!(t.get(2).is_some());
    }
}

/// The pace derivation, against the REAL corpus — the acceptance for shared-simulation P2's
/// "derive pace through `stat_eval`, never a constant". Skipped when the corpus is not on disk
/// (a packaged build), because a test that silently passes on a missing fixture is worse than one
/// that says why it did nothing.
#[cfg(test)]
mod pace_tests {
    use super::*;

    fn corpus() -> Option<resonantdust_content::loader::Bundle> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let sources = resonantdust_content::content::read_content_dir(&root).ok()?;
        resonantdust_content::load(&sources).ok()
    }

    /// A pawn's pace comes from its OWN authored traits through the shared evaluation, so the
    /// two species that share a walk must still walk at different speeds. `walks` authors
    /// `ground_speed add = [24, 12, 6]`; the bunny is level 1, the wolf level 2.
    #[test]
    fn bunny_and_wolf_derive_their_authored_paces() {
        let Some(bundle) = corpus() else {
            eprintln!("content/ not on disk — pace derivation not exercised");
            return;
        };
        for (name, expected) in [("bunny", 24.0_f64), ("wolf", 12.0)] {
            let Some(kind) = bundle.thing_names().iter().position(|n| n == name) else {
                panic!("no `{name}` in the corpus");
            };
            let kind = (kind + 1) as u16;
            // The payload a CREATE mints: the def's non-constant trait binds, tier in the
            // reference's variant nibble.
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
            let pace =
                resonantdust_content::move_eval::ground_speed(&bundle, kind, &payload, &[], 0);
            assert_eq!(
                pace, expected,
                "`{name}` derived {pace} tics/tile, expected {expected}",
            );
        }
    }
}
