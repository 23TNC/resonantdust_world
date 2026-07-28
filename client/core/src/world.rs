//! World-model helpers shared by both host engines — the reference-model decode of the WS
//! [`crate::protocol::StateRow`] into what a host renders, plus the global-tile ⇄
//! `position_reference` conversions.
//!
//! **Coordinates.** A `position_reference` is `region:8 | zone:8 | tile:8 | layer:8`, each spatial
//! byte `hi:4 | lo:4`. A global tile axis is therefore `region_nibble * 256 + zone_nibble * 16 +
//! tile_nibble` (0..4096). The wire addresses a zone by its `macro_position_reference`
//! (`region:8 | zone:8`) throughout — the anchor manager, the render events, and the subscription
//! frames all speak macro, so there is no `zone_id` to convert.

use resonantdust_codec::action::{MOVE_TO, PLACE, PROMOTE, PROMOTE_EVENT};

use crate::api::Event;
use crate::protocol::StateRow;

/// The global-tile ⇄ `position_reference` conversions live in the codec (first-pawns lifted
/// them so the WORKER steps on the exact same math) — re-exported here for the hosts.
pub use resonantdust_codec::object::{position_to_tile, tile_to_position};

/// The facing (`0`=south, `1`=east, `2`=north, `3`=west) packed in the top two bits of the `data`
/// byte (`rotation:2 | count:6`).
pub fn facing(data: u8) -> u8 {
    data >> 6
}

/// The action program for [`crate::api::Command::Move`] — the movement cadence's INITIAL program
/// (`ACTIONS.md` §Movement): `PROMOTE_EVENT` announces the intent once (clients speculate from
/// it), `PROMOTE` seeds the start position, `MOVE_TO` steps the first tile. The worker self-queues
/// the continuations (bare; the final hop `PROMOTE`d) — per-hop state never fans out.
pub fn move_to_program(entity: u32, tile_x: i32, tile_y: i32) -> Vec<u32> {
    vec![PROMOTE_EVENT, PROMOTE, MOVE_TO, entity, tile_to_position(tile_x, tile_y)]
}

/// The action program for [`crate::api::Command::Place`]: `PROMOTE` (prefix) then `PLACE entity dest`
/// — place the entity and make it client-visible in one event.
pub fn place_program(entity: u32, tile_x: i32, tile_y: i32) -> Vec<u32> {
    vec![PROMOTE, PLACE, entity, tile_to_position(tile_x, tile_y)]
}

/// Decode a composed `state` row into the host-facing [`Event::StateObject`]. The row's `zone`
/// (`macro_position_reference`) is carried through as-is — the render addresses zones by macro.
/// `removed` marks a delete (a `StateGone`).
pub fn state_event(row: &StateRow, removed: bool) -> Event {
    let (tile_x, tile_y) = position_to_tile(row.position_reference);
    Event::StateObject {
        macro_position: row.zone,
        entity_reference: row.entity_reference,
        definition_reference: row.definition_reference,
        tile_x,
        tile_y,
        facing: facing(row.data),
        tic: row.tic,
        removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_round_trips_through_position() {
        for &(x, y) in &[(0, 0), (5, 9), (255, 16), (4095, 4095), (300, 42)] {
            let pos = tile_to_position(x, y);
            assert_eq!(position_to_tile(pos), (x, y), "tile ({x},{y})");
        }
    }

    #[test]
    fn facing_reads_the_top_two_bits() {
        assert_eq!(facing(0b01_000000), 1); // east
        assert_eq!(facing(0b11_001010), 3); // west, count ignored
    }

    #[test]
    fn state_event_decodes_a_row_to_a_mover() {
        let row = StateRow {
            entity_reference: 0x3000_0007, // TYPE_PAWN server byte, object 7
            zone: 0x0102,                  // region 0x01, zone 0x02
            tic: 42,
            definition_reference: 9,
            position_reference: tile_to_position(300, 42),
            data: 0b01_000101, // facing east
        };
        match state_event(&row, /*removed=*/ false) {
            Event::StateObject {
                macro_position,
                entity_reference,
                definition_reference,
                tile_x,
                tile_y,
                facing,
                tic,
                removed,
            } => {
                assert_eq!(entity_reference, 0x3000_0007);
                assert_eq!(macro_position, 0x0102); // the wire macro, carried through
                assert_eq!((tile_x, tile_y), (300, 42));
                assert_eq!(definition_reference, 9);
                assert_eq!(facing, 1);
                assert_eq!(tic, 42);
                assert!(!removed);
            }
            other => panic!("expected StateObject, got {other:?}"),
        }
    }
}
