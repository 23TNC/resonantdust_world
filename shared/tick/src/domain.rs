//! The small plain types resolution operates on — the SDK-free projection of a
//! `state_log`/`event_log` row's *game* payload.
//!
//! The worker reads SpacetimeDB rows (with their generated types), projects the game
//! fields into these, resolves, and writes the result back via the `resolve` reducer.
//! Nothing here knows about `dirty`, `tic`, fencing, or subscriptions — those are the
//! worker's/module's concern; this is only "given the prior state and the events, what
//! is the new state." The per-action composition (`ACTION_MOVE`, `ACTION_DAMAGE`, …)
//! is filled in as actions land (Phase A2/A3); A0 defines the shapes.

/// Move to a new position. `data[0]` = `zone_id:32 << 8 | location:8`; `data[1]` =
/// `rotation:8 | offset:8` (low bits). Composed in Phase A2.
pub const ACTION_MOVE: u16 = 1;

/// Deal damage to the target. `data[0]` = amount. Reads the actor (to void if the
/// actor died this tic). Composed in Phase A3.
pub const ACTION_DAMAGE: u16 = 2;

/// The game payload of one entity's resolved state — the fields that live in a
/// `state`/`state_log` row alongside its `entity_key`/`tic`/bookkeeping. `data` is the
/// per-kind opaque state (e.g. `data[0]` = hp), interpreted by the action code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EntityState {
    pub kind: u16,
    pub zone_id: u32,
    pub location: u8,
    pub rotation: u8,
    pub offset: u8,
    pub data: [u64; 2],
}

/// The composable payload of one `event_log` row targeting an entity. The worker
/// supplies these already ordered by `event_reference`; resolution folds them onto the
/// prior [`EntityState`] in that order, reading `actor_key`'s resolved state where an
/// action consumes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub action: u16,
    pub actor_key: u64,
    pub data: [u64; 2],
}

/// Encode a `move`'s event data: `data[0] = zone_id<<8 | location`, `data[1] =
/// rotation<<8 | offset`. The one place the layout is defined, so injector and worker
/// agree.
pub fn pack_move(zone_id: u32, location: u8, rotation: u8, offset: u8) -> [u64; 2] {
    [
        ((zone_id as u64) << 8) | location as u64,
        ((rotation as u64) << 8) | offset as u64,
    ]
}

/// Fold one event onto an entity's state. Pure per-action composition; unknown actions
/// are ignored (an intent the sim doesn't implement is a no-op, not a fault). `DAMAGE`
/// and the actor-reading actions land in Phase A3.
pub fn apply_event(mut state: EntityState, ev: &Event) -> EntityState {
    match ev.action {
        ACTION_MOVE => {
            state.zone_id = (ev.data[0] >> 8) as u32;
            state.location = (ev.data[0] & 0xFF) as u8;
            state.rotation = ((ev.data[1] >> 8) & 0xFF) as u8;
            state.offset = (ev.data[1] & 0xFF) as u8;
        }
        _ => {}
    }
    state
}

/// Resolve an entity's new state: fold its events (already ordered by `event_reference`)
/// onto its prior state. Phase A2 covers position-only actions; A3 threads resolved
/// actor states in for actions that read them.
pub fn resolve_events(base: EntityState, events: &[Event]) -> EntityState {
    events.iter().fold(base, |s, e| apply_event(s, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn move_sets_position_from_event() {
        let base = EntityState { kind: 1, zone_id: 0, location: 10, ..Default::default() };
        let ev = Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(7, 200, 2, 5) };
        let out = apply_event(base, &ev);
        assert_eq!((out.zone_id, out.location, out.rotation, out.offset), (7, 200, 2, 5));
        assert_eq!(out.kind, 1, "move leaves kind untouched");
    }

    #[test]
    fn events_fold_in_order_last_move_wins() {
        let base = EntityState::default();
        let events = [
            Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, 10, 0, 0) },
            Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, 20, 0, 0) },
        ];
        assert_eq!(resolve_events(base, &events).location, 20);
    }

    #[test]
    fn unknown_action_is_a_noop() {
        let base = EntityState { location: 42, ..Default::default() };
        let ev = Event { action: 9999, actor_key: 0, data: [0, 0] };
        assert_eq!(apply_event(base, &ev), base);
    }
}
