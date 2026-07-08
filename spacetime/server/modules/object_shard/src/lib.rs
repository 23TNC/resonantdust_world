// lib.rs
//
// Object-shard module (crate `resonantdust_object_shard`, db
// `resonantdust-<env>-object-0`) — the mobile store for the new game. Holds
// "loose" things: things released out of a zone and now free to move, cross
// region/zone boundaries, and (later) attach to pawns. See docs/object-shard.md.
//
// Why it's separate from `zone_shard`:
//   1. Freedom from regions. Zone shards are partitioned by region, so moving a
//      thing across a region boundary would mean a cross-shard transfer. The
//      object shard is NOT region-partitioned — a loose thing carries its own
//      position, so a boundary crossing is a plain row update. Only the affixed⇄
//      free *conversion* (release/settle) crosses shards; movement never does.
//   2. Attaching things to pawns (a carried sword/chair moves with the pawn) —
//      forward-looking; pawns and the richer identity they need are deferred.
//
// Contrast with `zone_shard`: affixed things there are tile-snapped and packed
// into a per-zone cold blob that folds hot→cold. Loose things here are one row
// each, carry a sub-tile `offset`, and never fold.
//
//   - `free_things` — one row per loose thing: bitemporal `valid_at`, a stable
//     `object_id` instance handle, `zone_id` + `location` + `rotation` + `id` +
//     `offset`. CRUD reducers maintain `presence` transactionally.
//   - `presence`    — per-region loose-thing count; the dynamic routing table
//     gates read to decide which object shards to subscribe for a region.
//   - `gc`          — reaps prior versions + reconciles presence (NO fold).
//
// Shares the bitemporal foundation with `zone_shard` (`time`, `sequence`) and the
// `resonantdust_codec` layouts, so the free ⇄ affixed representations agree.
pub mod debug_mover;
pub mod free_things;
pub mod gc;
pub mod pawns;
pub mod presence;
pub mod sequence;
pub mod time;
pub mod transfer;

/// Default shard id for this deployment. `0` while a single object shard serves
/// everything; horizontal sharding assigns distinct ids per instance. Object
/// placement (which shard a released thing lands on) is the gate's concern — the
/// shard treats `zone_id` / `region_id` as opaque routing keys.
pub const DATA_SHARD: u16 = 0;
