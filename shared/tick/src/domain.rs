//! The small plain types resolution operates on — the SDK-free projection of a
//! `state_log`/`event_log` row's *game* payload.
//!
//! The worker reads SpacetimeDB rows (with their generated types), projects the game
//! fields into these, resolves, and writes the result back via the `resolve` reducer.
//! Nothing here knows about `dirty`, `tic`, fencing, or subscriptions — those are the
//! worker's/module's concern; this is only "given the prior state and the events, what
//! is the new state." The per-action composition (`ACTION_MOVE`, `ACTION_DAMAGE`, …)
//! is filled in as actions land (Phase A2/A3); A0 defines the shapes.

/// The resolution phase an action belongs to. Every entity's events at a tic resolve
/// **inbound → data → outbound** (the generalization of the receive-first / ack-last saga
/// ordering); within a phase, by `event_reference`. Phase is a function of the action's
/// `action_id` band, so it is pipeline-agnostic — see [`action_phase`] and
/// `docs/pipeline-generalization.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Transferred-in entities/quantities — applied first, so the entity is complete
    /// before any data action reads it.
    Inbound,
    /// Normal mutations (move, damage, …).
    Data,
    /// Transfers out — applied last, so the entity finishes its data before it leaves.
    Outbound,
}

// Action-id bands → phase. The action carried in an event is an `action_reference`
// (`data_type:6 | action_id:10`); phase keys off the low-10-bit `action_id` so the same
// band means the same phase in every pipeline. Inbound low, data mid, outbound high.
const ACTION_ID_MASK: u16 = 0x03FF;
const PHASE_INBOUND_MAX: u16 = 0x00FF; // action_id 0x000..=0x0FF → inbound
const PHASE_DATA_MAX: u16 = 0x02FF; //    0x100..=0x2FF → data; above → outbound

/// The [`Phase`] of an action, from its `action_id` band (the low 10 bits of the
/// `action_reference`; the `data_type` in the high bits doesn't affect phase).
pub fn action_phase(action: u16) -> Phase {
    match action & ACTION_ID_MASK {
        id if id <= PHASE_INBOUND_MAX => Phase::Inbound,
        id if id <= PHASE_DATA_MAX => Phase::Data,
        _ => Phase::Outbound,
    }
}

// ── actions, placed in their phase band ──

/// **Migration receive** (inbound): this (usually cross-shard) entity takes on the actor's
/// payload — the "attach" half of a transfer. Reads the actor (the source, on another
/// shard). Position stays this entity's (a zone cell's key-derived location).
pub const ACTION_RECEIVE: u16 = 0x001;

/// **Spawn** (inbound): materialize an entity from nothing — set its `kind` + placement on a
/// fresh (default) base. Inbound so a same-tic spawn lands before any data action reads the
/// entity; reads no actor. This is how a pawn is minted through the event pipeline (an
/// automated player's `spawn` intent) rather than a direct `seed_entity` write — the
/// work-gen creates the pending row for the new target, and this action fills it in.
/// `data[0]` = `zone_id:32 << 8 | location:8`; `data[1]` = `kind:16 << 16 | rotation:8 <<
/// 8 | offset:8`.
pub const ACTION_SPAWN: u16 = 0x002;

/// Move to a new position (data). `data[0]` = `zone_id:32 << 8 | location:8`; `data[1]` =
/// `rotation:8 | offset:8` (low bits).
pub const ACTION_MOVE: u16 = 0x100;

/// Deal damage to the target (data). `data[0]` = amount. Reads the actor (to void if the
/// actor died this tic).
pub const ACTION_DAMAGE: u16 = 0x101;

/// **Migration transfer** (outbound): mark the source in transfer-state and hand it off —
/// the worker resolving this emits the cross-shard `receive` and the next-tic `ack`.
/// Applied last so the entity finishes its data before it leaves.
pub const ACTION_TRANSFER: u16 = 0x300;

/// **Migration ack** (outbound): the source tombstones itself once the receive landed (the
/// actor = the received copy exists), so the entity isn't duplicated. Issued a tic AFTER
/// the receive so it reads the *completed* copy — a same-tic receive+ack is a cross-shard
/// cycle whose back-edge would read a stale state and duplicate. Reads the actor.
pub const ACTION_ACK: u16 = 0x301;

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

/// Encode a `spawn`'s event data: `data[0] = zone_id<<8 | location` (as `move`), `data[1] =
/// kind<<16 | rotation<<8 | offset`. Spawn carries `kind` (which move does not) so a
/// materialized entity is non-tombstone (`kind != 0`) and renders. The one place the layout
/// is defined, so the edge injector and the worker agree.
pub fn pack_spawn(kind: u16, zone_id: u32, location: u8, rotation: u8, offset: u8) -> [u64; 2] {
    [
        ((zone_id as u64) << 8) | location as u64,
        ((kind as u64) << 16) | ((rotation as u64) << 8) | offset as u64,
    ]
}

/// Encode a `damage` event: `data[0] = amount`.
pub fn pack_damage(amount: u64) -> [u64; 2] {
    [amount, 0]
}

/// Facing values carried in [`EntityState::rotation`]. The client maps each to a directional
/// sprite; **west is the east master mirrored** (no west art ships). `0=south, 1=east,
/// 2=north, 3=west`.
pub const FACE_SOUTH: u8 = 0;
pub const FACE_EAST: u8 = 1;
pub const FACE_NORTH: u8 = 2;
pub const FACE_WEST: u8 = 3;

/// The facing implied by a move `old_loc → new_loc` within one zone, or `None` if the cell
/// didn't change (so the pawn keeps its prior facing). Cells are `y<<4 | x` (`+x` east, `+y`
/// south — screen-down); the dominant axis wins, a diagonal tie favouring the horizontal
/// (east/west) facing.
fn facing_from_delta(old_loc: u8, new_loc: u8) -> Option<u8> {
    let (ox, oy) = ((old_loc & 0x0F) as i32, (old_loc >> 4) as i32);
    let (nx, ny) = ((new_loc & 0x0F) as i32, (new_loc >> 4) as i32);
    let (dx, dy) = (nx - ox, ny - oy);
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx > 0 {
            FACE_EAST
        } else {
            FACE_WEST
        }
    } else if dy > 0 {
        FACE_SOUTH
    } else {
        FACE_NORTH
    })
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
        ACTION_SPAWN => {
            // Materialize: set kind + placement on the (default) base. Unlike MOVE, this
            // also sets `kind`, so the resolved entity is non-tombstone and renders.
            state.kind = ((ev.data[1] >> 16) & 0xFFFF) as u16;
            state.zone_id = (ev.data[0] >> 8) as u32;
            state.location = (ev.data[0] & 0xFF) as u8;
            state.rotation = ((ev.data[1] >> 8) & 0xFF) as u8;
            state.offset = (ev.data[1] & 0xFF) as u8;
        }
        ACTION_MOVE => {
            let new_zone = (ev.data[0] >> 8) as u32;
            let new_loc = (ev.data[0] & 0xFF) as u8;
            // Facing follows the direction of travel: the client picks the pawn's s/e/n sprite
            // (and the west→mirrored-east flip) from this `rotation`. Derived within a zone from
            // the old→new cell; a same-cell move or a cross-zone hop keeps the prior facing
            // (cross-zone facing is a later concern — movers are single-zone today).
            if new_zone == state.zone_id {
                if let Some(rot) = facing_from_delta(state.location, new_loc) {
                    state.rotation = rot;
                }
            }
            state.zone_id = new_zone;
            state.location = new_loc;
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

/// Resolve an entity's new state: fold its events onto its prior state in **phase order**
/// (inbound → data → outbound), each paired with its actor's resolved state (`None` where
/// the action doesn't read an actor). Within a phase the input order is preserved, and the
/// worker supplies events already ordered by `event_reference`, so the effective order is
/// (phase, reference). Generic over the pipeline's [`Domain`].
pub fn resolve_events<D: Domain>(
    base: D::Payload,
    events: &[(Event, Option<D::Payload>)],
) -> D::Payload {
    let mut state = base;
    for phase in [Phase::Inbound, Phase::Data, Phase::Outbound] {
        for (e, actor) in events.iter().filter(|(e, _)| action_phase(e.action) == phase) {
            state = D::apply_event(state, e, actor.as_ref());
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_hp(h: u64) -> EntityState {
        EntityState { data: [h, 0], ..Default::default() }
    }

    #[test]
    fn move_sets_position_from_event() {
        // Position + offset come from the event; rotation is DERIVED from the movement (not the
        // event's rotation field). Base faces east, then moves (10,0)→(5,12) within zone 7 —
        // dominant vertical, down → south.
        let base = EntityState { kind: 1, zone_id: 7, location: loc(10, 0), rotation: FACE_EAST, ..Default::default() };
        let ev = Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(7, loc(5, 12), 0, 5) };
        let out = apply_event(base, &ev, None); // move ignores the actor
        assert_eq!((out.zone_id, out.location, out.offset), (7, loc(5, 12), 5));
        assert_eq!(out.rotation, FACE_SOUTH, "rotation derived from movement, not the event");
        assert_eq!(out.kind, 1, "move leaves kind untouched");
    }

    #[test]
    fn spawn_materializes_kind_and_placement_from_default_base() {
        // A fresh (default, kind 0 = tombstone) base + a spawn → a live entity.
        let ev = Event { action: ACTION_SPAWN, actor_key: 0, data: pack_spawn(7, 0, 40, 1, 0x88) };
        let out = apply_event(EntityState::default(), &ev, None);
        assert_eq!(out.kind, 7, "spawn sets kind so the entity is non-tombstone");
        assert_eq!((out.zone_id, out.location, out.rotation, out.offset), (0, 40, 1, 0x88));
    }

    #[test]
    fn spawn_then_move_resolves_in_phase_order() {
        // Same-tic spawn (inbound) then move (data): the entity materializes, then relocates.
        let events = [
            (Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, 99, 0, 0) }, None),
            (Event { action: ACTION_SPAWN, actor_key: 0, data: pack_spawn(7, 0, 5, 0, 0) }, None),
        ];
        let out = resolve_events::<Spatial>(EntityState::default(), &events);
        assert_eq!(out.kind, 7, "spawn (inbound) applied");
        assert_eq!(out.location, 99, "move (data) applied after spawn, so it wins position");
    }

    /// Cell index `y<<4 | x` — mirror of `codec::packed::cell`, inlined so the pure-tick
    /// tests carry no codec dep.
    fn loc(x: u8, y: u8) -> u8 {
        (y << 4) | x
    }

    #[test]
    fn facing_follows_direction_of_travel() {
        assert_eq!(facing_from_delta(loc(5, 5), loc(8, 5)), Some(FACE_EAST)); // +x
        assert_eq!(facing_from_delta(loc(8, 5), loc(5, 5)), Some(FACE_WEST)); // -x
        assert_eq!(facing_from_delta(loc(5, 5), loc(5, 9)), Some(FACE_SOUTH)); // +y (down)
        assert_eq!(facing_from_delta(loc(5, 9), loc(5, 5)), Some(FACE_NORTH)); // -y (up)
        // Diagonal tie favours horizontal; a pure vertical-dominant move faces N/S.
        assert_eq!(facing_from_delta(loc(5, 5), loc(7, 7)), Some(FACE_EAST));
        assert_eq!(facing_from_delta(loc(5, 5), loc(6, 9)), Some(FACE_SOUTH));
        // No move → no facing change.
        assert_eq!(facing_from_delta(loc(5, 5), loc(5, 5)), None);
    }

    #[test]
    fn move_sets_rotation_from_movement_and_keeps_it_when_still() {
        let base = EntityState { kind: 7, zone_id: 0, location: loc(5, 5), rotation: FACE_SOUTH, ..Default::default() };
        // Move west → rotation becomes west (so the client mirrors the east sprite).
        let west = apply_event(base, &Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, loc(2, 5), 0, 0) }, None);
        assert_eq!(west.rotation, FACE_WEST);
        assert_eq!(west.location, loc(2, 5));
        // A same-cell move keeps the prior facing (no spurious re-facing).
        let still = apply_event(EntityState { rotation: FACE_NORTH, location: loc(2, 5), ..Default::default() },
            &Event { action: ACTION_MOVE, actor_key: 0, data: pack_move(0, loc(2, 5), 0, 0) }, None);
        assert_eq!(still.rotation, FACE_NORTH);
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

    #[test]
    fn action_phase_bands() {
        assert_eq!(action_phase(ACTION_RECEIVE), Phase::Inbound);
        assert_eq!(action_phase(ACTION_SPAWN), Phase::Inbound);
        assert_eq!(action_phase(ACTION_MOVE), Phase::Data);
        assert_eq!(action_phase(ACTION_DAMAGE), Phase::Data);
        assert_eq!(action_phase(ACTION_TRANSFER), Phase::Outbound);
        assert_eq!(action_phase(ACTION_ACK), Phase::Outbound);
        // Phase keys off the action_id (low 10 bits); a data_type in the high bits doesn't
        // change it (same action_id → same phase in every pipeline).
        assert_eq!(action_phase(ACTION_MOVE | (0x2A << 10)), Phase::Data);
    }

    /// A domain whose payload records the exact apply order: each event shifts its action
    /// into a base-0x1000 accumulator. Lets a test read back the sequence of applications.
    struct OrderDom;
    impl Domain for OrderDom {
        type Payload = u64;
        fn apply_event(state: u64, ev: &Event, _actor: Option<&u64>) -> u64 {
            state.wrapping_mul(0x1000).wrapping_add(ev.action as u64)
        }
        fn action_reads_actor(_action: u16) -> bool {
            false
        }
    }

    #[test]
    fn resolve_events_applies_in_phase_order() {
        // Feed events OUT of phase order — they must apply inbound → data → outbound.
        let events = [
            (Event { action: ACTION_TRANSFER, actor_key: 0, data: [0, 0] }, None), // outbound
            (Event { action: ACTION_MOVE, actor_key: 0, data: [0, 0] }, None),     // data
            (Event { action: ACTION_RECEIVE, actor_key: 0, data: [0, 0] }, None),  // inbound
        ];
        // Applied order RECEIVE, MOVE, TRANSFER → 0x001, then 0x100, then 0x300.
        assert_eq!(resolve_events::<OrderDom>(0, &events), 0x001_100_300);
    }

    #[test]
    fn within_a_phase_reference_order_is_preserved() {
        // Two data events keep their input (event_reference) order: DAMAGE then MOVE.
        let events = [
            (Event { action: ACTION_DAMAGE, actor_key: 0, data: [0, 0] }, None),
            (Event { action: ACTION_MOVE, actor_key: 0, data: [0, 0] }, None),
        ];
        assert_eq!(resolve_events::<OrderDom>(0, &events), 0x101_100);
    }
}
