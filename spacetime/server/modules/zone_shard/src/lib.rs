// lib.rs
//
// Zone-shard module (crate `resonantdust_zone_shard`, db `resonantdust-<env>-zone-0`)
// — the hot/cold zone store for the new game. Holds the location-dependent
// region/zone state: terrain plus the things that have *settled into* the world.
// Mobile ("loose") things and pawns live in a separate `object_shard` — something
// dropped into the world lives in an object shard, and becomes part of a zone
// shard once it settles. (Formerly `region_shard`; renamed because the db name
// must be DNS-like — no underscores — and the shard is keyed by zone.)
//
// SpacetimeDB is the generic data store; the Gateway is the game server. The
// zone shard holds two representations of every 16×16 zone and trusts the
// Gateway to validate and to overlay them:
//
//   - `zones` (cold) — one settled `cold_zone` row per zone: the full 256-tile
//     terrain + the packed thing list. Big, rarely rewritten.
//   - `hot` — two identical layer tables (`hot_tiles` / `hot_things`), one tiny
//     row per *changed* cell. A single edit fans out a ~16-byte row, not the
//     whole cold zone.
//   - `gc` — folds at-rest hot cells back into cold (one row per zone, batched
//     and back-dated), then reaps prior versions.
//
// Built on the bitemporal foundation carried over from the previous iteration:
//   - `time` — the client/server time-discipline contract.
//   - `sequence` — the u16 allocator making `valid_at` unique per same-ms write.
//
// The `valid_at` PK + zone-cell + packed-thing/tile bit helpers live in the
// shared `resonantdust_codec::packed` crate (bind-mounted at /workspace/shared),
// so the region shard and the client encode the same layouts. Pawns are a
// later, separate model in the `object_shard` (they need instance identity the
// positional tables don't).
pub mod exists;
pub mod gc;
pub mod hot;
pub mod sequence;
pub mod time;
pub mod transfer;
pub mod zones;

/// Default shard id for this deployment. `0` while a single shard serves
/// everything; horizontal sharding assigns distinct ids per instance. `zone_id`
/// encoding (region + local zone) and shard routing are the Gateway's concern —
/// the shard treats `zone_id` as opaque.
pub const DATA_SHARD: u16 = 0;
