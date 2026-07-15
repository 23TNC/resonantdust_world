// lib.rs
pub mod gate_api;
pub mod players;

use spacetimedb::{reducer, ReducerContext};

/// The data-shard partition id this auth database owns. Stamped onto
/// the `data_shard` column of rows this module writes (`player_profiles`).
/// `0` today.
///
/// This is the `players` **auth** database: it owns accounts, login, and
/// the identity↔player_id↔(card-shard, soul_id) routing. It does NOT hold
/// cards or souls — those live in the per-shard `cards` databases, and each
/// `Player` row carries the `data_shard` of the card shard it's assigned to.
/// Low-write, so a single auth DB can serve all players; the `cards` shards
/// are what scale out.
pub const DATA_SHARD: u16 = 0;

/// Module init — runs once on a fresh publish. Seeds the server-side system accounts
/// (the developer account at a reserved id with content-author) so they exist before
/// any human logs in. Idempotent: the seed skips accounts that already exist.
///
/// It used to also seed a recurring GC schedule, which reaped the `players` table's
/// prior version rows. Flattening the table to one row per player left that sweep with
/// nothing to do, so it — and the `gc` module hosting it — are gone.
#[reducer(init)]
pub fn init(ctx: &ReducerContext) {
    crate::players::seed_system_players(ctx);
}
