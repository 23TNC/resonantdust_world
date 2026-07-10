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

/// **Migration receive** (Phase C3): this (new, usually cross-shard) entity takes on the
/// actor's payload — the "attach" half of an object↔zone transfer. Reads the actor
/// (the source entity, on another shard). Position stays this entity's (a zone cell's
/// key-derived location).
pub const ACTION_RECEIVE: u16 = 3;

/// **Migration ack** (Phase C3): the source tombstones itself once the receive landed
/// (the actor = the received copy exists), so the entity isn't duplicated. Issued a tic
/// AFTER the receive so it reads the *completed* copy — cross-tic sequencing is what
/// makes a conserved transfer exactly-once (a same-tic receive+ack is a cross-shard
/// cycle whose back-edge would read a stale state and duplicate). Reads the actor.
pub const ACTION_ACK: u16 = 4;

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

/// A pipeline's *resolution policy* — the payload it carries and how events compose
/// onto it. This is the one seam that varies per data structure (pawn, object, zone, …);
/// the scheduling spine ([`priority`](crate::priority) / [`read_rule`](crate::read_rule) /
/// the [`resolve_events`] fold) is generic over it and never inspects the payload. See
/// `docs/pipeline-generalization.md`.
///
/// The methods are associated (no `self`): a `Domain` is a marker for its payload type,
/// selected by the worker from an event's `data_type` — there is no per-instance state.
pub trait Domain {
    /// The game payload this pipeline carries in each `state`/`state_log` row.
    type Payload: Copy + Default;

    /// Fold one event onto the payload, given the actor's resolved state where the
    /// action reads it (`None` otherwise). Pure per-action composition.
    fn apply_event(
        state: Self::Payload,
        ev: &Event,
        actor: Option<&Self::Payload>,
    ) -> Self::Payload;

    /// Does resolving `action` require the actor's resolved state (routing it through
    /// the read-rule + priority-DAG, possibly blocking cross-shard)?
    fn action_reads_actor(action: u16) -> bool;
}

/// The spatial payload objects and zones share today — the sole [`Domain`] until a
/// divergent payload lands (`docs/pipeline-generalization.md`, Phase 4). A zero-sized
/// marker; its logic is the free `apply_event`/`action_reads_actor` below (kept as free
/// items so the edge/npc injectors, which only touch the `ACTION_*` constants and
/// `pack_*` helpers, are unaffected).
#[derive(Clone, Copy, Debug)]
pub struct Spatial;

impl Domain for Spatial {
    type Payload = EntityState;

    fn apply_event(state: EntityState, ev: &Event, actor: Option<&EntityState>) -> EntityState {
        apply_event(state, ev, actor)
    }

    fn action_reads_actor(action: u16) -> bool {
        action_reads_actor(action)
    }
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

/// Encode a `damage` event: `data[0] = amount`.
pub fn pack_damage(amount: u64) -> [u64; 2] {
    [amount, 0]
}

/// An entity's hit points live in `data[0]`. `0` = dead.
pub fn hp(state: &EntityState) -> u64 {
    state.data[0]
}

/// Does resolving this action require the actor's resolved state? Actor-reading
/// actions are the ones that go through the read-rule + priority-DAG (they can block
/// on an actor, possibly across shards); position-only actions (`move`) resolve with no
/// cross-entity read.
pub fn action_reads_actor(action: u16) -> bool {
    matches!(action, ACTION_DAMAGE | ACTION_RECEIVE | ACTION_ACK)
}

/// A tombstoned entity — the source of a completed migration. `kind == 0` is "no
/// entity here"; the client hides these.
fn is_tombstone(state: &EntityState) -> bool {
    state.kind == 0
}

/// Fold one event onto an entity's state, given the actor's resolved state where the
/// action reads it (`None` for actions that don't, or an absent/unresolved actor —
/// which reads as dead). Pure per-action composition; unknown actions are a no-op.
///
/// `DAMAGE` is the actor-reading case: a **dead actor's blow is voided** (the
/// preemption the priority-DAG hangs on — "dead B can't hit C"); a live actor subtracts
/// `data[0]` from the target's hp (saturating at 0).
pub fn apply_event(mut state: EntityState, ev: &Event, actor: Option<&EntityState>) -> EntityState {
    match ev.action {
        ACTION_MOVE => {
            state.zone_id = (ev.data[0] >> 8) as u32;
            state.location = (ev.data[0] & 0xFF) as u8;
            state.rotation = ((ev.data[1] >> 8) & 0xFF) as u8;
            state.offset = (ev.data[1] & 0xFF) as u8;
        }
        ACTION_DAMAGE => {
            let attacker_alive = actor.map_or(false, |a| hp(a) > 0);
            if attacker_alive {
                state.data[0] = hp(&state).saturating_sub(ev.data[0]);
            }
        }
        ACTION_RECEIVE => {
            // Attach: take on the source's payload (kind + data). Our position — a zone
            // cell's key-derived location — stays ours. A tombstoned/absent source
            // transfers nothing.
            if let Some(a) = actor.filter(|a| !is_tombstone(a)) {
                state.kind = a.kind;
                state.data = a.data;
            }
        }
        ACTION_ACK => {
            // Detach: the copy exists on the other shard (actor is a live received
            // entity) → tombstone this source so the entity is exactly-once.
            if actor.map_or(false, |a| !is_tombstone(a)) {
                state.kind = 0;
                state.data = [0, 0];
            }
        }
        _ => {}
    }
    state
}

/// Resolve an entity's new state: fold its events (already ordered by `event_reference`)
/// onto its prior state, each paired with its actor's resolved state (`None` where the
/// action doesn't read an actor). The worker supplies the actor states after applying
/// the read-rule + priority-DAG. Generic over the pipeline's [`Domain`]; the fold itself
/// is unchanged — only which `apply_event` it calls is now a type parameter.
pub fn resolve_events<D: Domain>(
    base: D::Payload,
    events: &[(Event, Option<D::Payload>)],
) -> D::Payload {
    events
        .iter()
        .fold(base, |s, (e, actor)| D::apply_event(s, e, actor.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_hp(h: u64) -> EntityState {
        EntityState { data: [h, 0], ..Default::default() }
    }

    #[test]
    fn move_sets_position_from_event() {
        let base = EntityState { kind: 1, zone_id: 0, location: 10, ..Default::default() };
        let ev = Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(7, 200, 2, 5) };
        let out = apply_event(base, &ev, None); // move ignores the actor
        assert_eq!((out.zone_id, out.location, out.rotation, out.offset), (7, 200, 2, 5));
        assert_eq!(out.kind, 1, "move leaves kind untouched");
    }

    #[test]
    fn events_fold_in_order_last_move_wins() {
        let base = EntityState::default();
        let events = [
            (Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, 10, 0, 0) }, None),
            (Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, 20, 0, 0) }, None),
        ];
        assert_eq!(resolve_events::<Spatial>(base, &events).location, 20);
    }

    #[test]
    fn unknown_action_is_a_noop() {
        let base = EntityState { location: 42, ..Default::default() };
        let ev = Event { action: 9999, actor_key: 0, data: [0, 0] };
        assert_eq!(apply_event(base, &ev, None), base);
    }

    #[test]
    fn live_actor_deals_damage() {
        let ev = Event { action: ACTION_DAMAGE, actor_key: 0, data: pack_damage(3) };
        assert_eq!(hp(&apply_event(with_hp(5), &ev, Some(&with_hp(1)))), 2);
    }

    #[test]
    fn dead_or_absent_actor_deals_nothing() {
        let ev = Event { action: ACTION_DAMAGE, actor_key: 0, data: pack_damage(3) };
        // Dead actor (hp 0) → blow voided (the priority-DAG preemption).
        assert_eq!(hp(&apply_event(with_hp(5), &ev, Some(&with_hp(0)))), 5);
        // Absent (unresolved/missing) actor also voids.
        assert_eq!(hp(&apply_event(with_hp(5), &ev, None)), 5);
    }

    #[test]
    fn damage_saturates_at_zero() {
        let ev = Event { action: ACTION_DAMAGE, actor_key: 0, data: pack_damage(10) };
        assert_eq!(hp(&apply_event(with_hp(3), &ev, Some(&with_hp(1)))), 0);
    }

    #[test]
    fn receive_copies_source_payload() {
        let source = EntityState { kind: 7, data: [42, 9], ..Default::default() };
        // A fresh zone cell (kind 0) at some location receives the source's payload.
        let dest = EntityState { kind: 0, location: 200, ..Default::default() };
        let ev = Event { action: ACTION_RECEIVE, actor_key: 0, data: [0, 0] };
        let out = apply_event(dest, &ev, Some(&source));
        assert_eq!((out.kind, out.data), (7, [42, 9]), "payload migrated");
        assert_eq!(out.location, 200, "dest keeps its own (key-derived) position");
    }

    #[test]
    fn ack_tombstones_source_once_copy_exists() {
        let source = EntityState { kind: 7, data: [42, 0], ..Default::default() };
        // The received copy exists (kind 7) → source tombstones.
        let received = EntityState { kind: 7, data: [42, 0], ..Default::default() };
        let ev = Event { action: ACTION_ACK, actor_key: 0, data: [0, 0] };
        let out = apply_event(source, &ev, Some(&received));
        assert_eq!((out.kind, out.data), (0, [0, 0]), "source tombstoned — no duplicate");
    }

    #[test]
    fn ack_is_noop_when_copy_absent() {
        // If the receive never landed (actor tombstone/absent), the source is kept —
        // the transfer didn't happen, so we must not lose the entity.
        let source = EntityState { kind: 7, data: [42, 0], ..Default::default() };
        let ev = Event { action: ACTION_ACK, actor_key: 0, data: [0, 0] };
        assert_eq!(apply_event(source, &ev, None).kind, 7);
        assert_eq!(apply_event(source, &ev, Some(&EntityState::default())).kind, 7);
    }
}
