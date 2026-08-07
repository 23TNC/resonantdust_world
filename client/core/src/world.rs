//! World-model helpers shared by both host engines — the reference-model decode of the WS
//! [`crate::protocol::StateRow`] into what a host renders, plus the global-tile ⇄
//! `position_reference` conversions.
//!
//! **Coordinates.** A `position_reference` is `region:8 | zone:8 | tile:8 | layer:8`, each spatial
//! byte `hi:4 | lo:4`. A global tile axis is therefore `region_nibble * 256 + zone_nibble * 16 +
//! tile_nibble` (0..4096). The wire addresses a zone by its `macro_position_reference`
//! (`region:8 | zone:8`) throughout — the anchor manager, the render events, and the subscription
//! frames all speak macro, so there is no `zone_id` to convert.

use resonantdust_codec::action::{MOVE_TO, PLACE, PROMOTE};

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

// `move_to_program` DIED with the raw MOVE_TO front door (input-rework F3): hosts compose
// `EXECUTE_INTERACTION(move_to)` and the WORKER queues the seed. The speculation intent
// (`move_intents` below) still reads the fanned seed — that channel is unchanged.

/// The action program for [`crate::api::Command::Place`]: `PROMOTE` (prefix) then `PLACE entity dest`
/// — place the entity and make it client-visible in one event.
pub fn place_program(entity: u32, tile_x: i32, tile_y: i32) -> Vec<u32> {
    vec![PROMOTE, PLACE, entity, tile_to_position(tile_x, tile_y)]
}

/// The action program for [`crate::api::Command::BuildWall`] (build-walls D5): bare
/// `BUILD_WALL start end object` — no PROMOTE (the verb writes nothing; the worker expands
/// the rect's perimeter and queues a `PROMOTE SET` per tile, each routing to its own zone).
pub fn build_wall_program(start_x: i32, start_y: i32, end_x: i32, end_y: i32, object: u32) -> Vec<u32> {
    vec![
        resonantdust_codec::action::BUILD_WALL,
        tile_to_position(start_x, start_y),
        tile_to_position(end_x, end_y),
        object,
    ]
}

/// Decode a settled, promoted `event` row's program into [`Event::MoveIntent`]s — one per
/// `MOVE_TO` instruction (`ACTIONS.md` §Movement: the intent channel clients speculate from) —
/// and [`Event::QueueState`]s — one per `QUEUE_STATE` snapshot (intent-queue-ui F1: the
/// details panel's strip; entries ride flat, stride 4). Other actions are simply skipped.
pub fn move_intents(zone: u16, event_tic: u16, actions: &[u32]) -> Vec<Event> {
    let mut out = Vec::new();
    for inst in resonantdust_codec::action::program(actions) {
        let Ok(inst) = inst else { break };
        if inst.action == resonantdust_codec::action::QUEUE_STATE {
            // QUEUE_STATE pawn _reserved count entry-words×count.
            if let [pawn, _reserved, _count, entries @ ..] = inst.operands {
                out.push(Event::QueueState {
                    macro_position: zone,
                    entity_reference: *pawn,
                    event_tic,
                    entries: entries.to_vec(),
                });
            }
            continue;
        }
        if inst.action != MOVE_TO {
            continue;
        }
        if let [obj, dest] = inst.operands {
            let (tile_x, tile_y) = position_to_tile(*dest);
            out.push(Event::MoveIntent {
                macro_position: zone,
                entity_reference: *obj,
                tile_x,
                tile_y,
                event_tic,
            });
        }
    }
    out
}

/// Decode a composed `state` row into the host-facing [`Event::StateObject`]. The row's `zone`
/// (`macro_position_reference`) is carried through as-is — the render addresses zones by macro.
/// `removed` marks a delete (a `StateGone`).
pub fn state_event(row: &StateRow, removed: bool) -> Event {
    let (tile_x, tile_y) = position_to_tile(row.position_reference);
    let (sub_x, sub_y) = resonantdust_codec::object::position_subtile(row.position_reference);
    Event::StateObject {
        macro_position: row.zone,
        entity_reference: row.entity_reference,
        definition_reference: row.definition_reference,
        tile_x,
        tile_y,
        sub_x,
        sub_y,
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
                sub_x,
                sub_y,
                facing,
                tic,
                removed,
            } => {
                assert_eq!(entity_reference, 0x3000_0007);
                assert_eq!(macro_position, 0x0102); // the wire macro, carried through
                assert_eq!((tile_x, tile_y), (300, 42));
                assert_eq!((sub_x, sub_y), (0, 0), "a whole-tile position");
                assert_eq!(definition_reference, 9);
                assert_eq!(facing, 1);
                assert_eq!(tic, 42);
                assert!(!removed);
            }
            other => panic!("expected StateObject, got {other:?}"),
        }
    }
}
