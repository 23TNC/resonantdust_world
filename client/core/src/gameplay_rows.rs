//! Per-entity **gameplay rows** — the payload and need rows every derived answer reads
//! (shared-simulation P2d).
//!
//! These were the same store built five times: `MoverLayer.ts`, three npc brains, and — briefly,
//! and wrongly — hung off `Mover` in this crate. That last one is worth naming, because it was
//! made *by this stream* and is the exact pattern the stream exists to delete: the rows were put
//! where their FIRST consumer needed them (pace derivation) rather than where they belong.
//!
//! Two defects followed from that placement, both real ([I8]):
//!
//!   - **Rows arriving before a pawn's first anchor were dropped.** A zone snapshot may fan
//!     `Payload` before `StateObject`; both other hosts document that and buffer for it. Core lost
//!     the payload and left the pawn at `pace: None` — held still forever.
//!   - **`PawnNeed` also fans for `0x40…` player-pawn references**, entities with no position at
//!     all. A positional track cannot key them.
//!
//! So the rows are the ENTITY's, not the mover's. They arrive whenever they arrive, for whatever
//! reference they name, and consumers read them.

use std::collections::HashMap;

/// Payload and need rows, keyed by entity reference.
#[derive(Debug, Default)]
pub struct GameplayRows {
    payload: HashMap<u32, Vec<u32>>,
    /// The zone each entity's rows arrived under, so a closing zone can evict them. Without it
    /// the rows leak for the life of the session: a zone leaving the subscription sends no
    /// per-entity delete, so nothing else ever says the entity is gone.
    zone: HashMap<u32, u16>,
    /// `entity -> (need reference -> (row, set_tic))`. Keyed by the row's REFERENCE via
    /// `codec::object::row_reference`, not an open-coded modulo — the 48-bit law has one helper
    /// and a second spelling of it is how a lane gets truncated.
    needs: HashMap<u32, HashMap<u32, (u64, u16)>>,
}

impl GameplayRows {
    pub fn new() -> Self {
        Self::default()
    }

    /// A payload sidecar row. Accepted whether or not the entity has a position yet.
    pub fn observe_payload(&mut self, entity: u32, zone: u16, payload: Vec<u32>) {
        self.zone.insert(entity, zone);
        self.payload.insert(entity, payload);
    }

    /// One `needs` sub-table row, upserted by its reference.
    pub fn observe_need(&mut self, entity: u32, zone: u16, need: u64, set_tic: u16) {
        self.zone.insert(entity, zone);
        let key = resonantdust_codec::object::row_reference(need);
        self.needs.entry(entity).or_default().insert(key, (need, set_tic));
    }

    pub fn payload(&self, entity: u32) -> &[u32] {
        self.payload.get(&entity).map(Vec::as_slice).unwrap_or(&[])
    }

    /// This entity's need rows as the `(row, set_tic)` pairs every eval takes.
    pub fn needs(&self, entity: u32) -> Vec<(u64, u16)> {
        self.needs.get(&entity).map(|m| m.values().copied().collect()).unwrap_or_default()
    }

    pub fn has(&self, entity: u32) -> bool {
        self.payload.contains_key(&entity) || self.needs.contains_key(&entity)
    }

    /// ONE eviction rule: the entity is gone. Called on `StateGone`/removal — mirroring the only
    /// host that got eviction right (`MoverLayer.ts`), rather than the three that leaked.
    pub fn forget(&mut self, entity: u32) {
        self.payload.remove(&entity);
        self.needs.remove(&entity);
        self.zone.remove(&entity);
    }

    /// A zone left the subscription — evict every entity whose rows arrived under it.
    pub fn forget_zone(&mut self, zone: u16) {
        let gone: Vec<u32> =
            self.zone.iter().filter(|(_, &z)| z == zone).map(|(&e, _)| e).collect();
        for e in gone {
            self.forget(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this module exists to fix: rows must survive arriving BEFORE the pawn's first
    /// position row, because a zone snapshot fans them in that order.
    #[test]
    fn rows_arriving_before_any_position_are_kept() {
        let mut g = GameplayRows::new();
        g.observe_payload(7, 0, vec![1, 2, 3]);
        g.observe_need(7, 0, 0xa4fb_8001_0020, 40);
        assert_eq!(g.payload(7), &[1, 2, 3]);
        assert_eq!(g.needs(7).len(), 1);
    }

    /// `PawnNeed` fans for player-pawn references too — entities with no position ever.
    #[test]
    fn a_positionless_player_pawn_reference_is_storable() {
        let mut g = GameplayRows::new();
        g.observe_need(0x4080_0003, 0, 0x8001_0060, 12);
        assert_eq!(g.needs(0x4080_0003).len(), 1);
    }

    /// A need upserts by REFERENCE — a fresh row for the same need replaces it rather than
    /// accumulating a second copy the eval would double-count.
    #[test]
    fn a_need_upserts_by_reference() {
        let mut g = GameplayRows::new();
        g.observe_need(1, 0, 0x0001_8001_0020, 10);
        g.observe_need(1, 0, 0x00ff_8001_0020, 20);
        let rows = g.needs(1);
        assert_eq!(rows.len(), 1, "same need reference, one row");
        assert_eq!(rows[0].1, 20, "the newer write won");
    }

    /// The leak the audit caught: a closing zone must take its entities' rows with it. Nothing
    /// else ever says they are gone — a zone leaving the subscription sends no per-entity delete.
    #[test]
    fn a_closing_zone_evicts_its_entities_rows() {
        let mut g = GameplayRows::new();
        g.observe_payload(1, 7, vec![1, 2, 3]);
        g.observe_payload(2, 8, vec![4]);
        g.forget_zone(7);
        assert!(!g.has(1), "zone 7's rows leaked");
        assert!(g.has(2), "zone 8 is untouched");
    }

    #[test]
    fn forgetting_an_entity_drops_both_halves() {
        let mut g = GameplayRows::new();
        g.observe_payload(1, 0, vec![9]);
        g.observe_need(1, 0, 0x8001_0020, 5);
        g.forget(1);
        assert!(!g.has(1));
    }
}
