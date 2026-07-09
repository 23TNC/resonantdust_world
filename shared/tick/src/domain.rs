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
