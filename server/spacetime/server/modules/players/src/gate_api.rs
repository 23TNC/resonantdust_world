//! Gate-facing write reducers for the `players` auth/index DB. Authorization
//! is the gateway's job — these trust their arguments.

use spacetimedb::{reducer, ReducerContext};

use crate::players::{set_faction, set_permissions};

/// Set `player_id`'s faction — the public reducer the `players` module anticipated
/// for the recipe `…aspect.faction.set` effect, now driven by the gateway instead of
/// an in-module action path.
///
/// `time_ms` is accepted for wire-format parity and **ignored**: it used to select and
/// stamp a version row, and the table no longer has versions. Kept in the signature
/// because `/call` keys args on the exact param name — dropping it would break callers
/// for no gain. Same treatment as `client_time_ms` elsewhere in this module.
#[reducer]
pub fn set_player_faction(
    ctx: &ReducerContext,
    player_id: u32,
    time_ms: u64,
    faction: u8,
) -> Result<(), String> {
    let _ = time_ms;
    set_faction(ctx, player_id, faction)
}

/// Set `player_id`'s permissions capability byte. The store is content-agnostic and
/// trusts its args (per the module contract); the gateway authorizes the caller — a
/// player can't grant themselves capabilities. Used to provision content-author /
/// admin accounts (typically in the `0..FIRST_PLAYER_ID` reserved range).
///
/// `time_ms` is accepted and ignored — see [`set_player_faction`].
#[reducer]
pub fn set_player_permissions(
    ctx: &ReducerContext,
    player_id: u32,
    time_ms: u64,
    perms: u8,
) -> Result<(), String> {
    let _ = time_ms;
    set_permissions(ctx, player_id, perms)
}
