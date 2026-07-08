//! Server time source for the `valid_at` model.
//!
//! Time is **milliseconds** throughout: `valid_at` rows pack u48 ms, durations
//! are ms. The old client/server drift-grace contract (`effective_now_ms` + its
//! tolerance constants) was retired with the move to the event-log sync model
//! (`docs/sync.md`): reducers stamp at server time and the render delay `D` is
//! applied entirely client-side, so there is no per-call client-time validation.

use spacetimedb::ReducerContext;

/// Wall-clock now in unix milliseconds. Convert from the microsecond timestamp
/// the SpacetimeDB scheduler provides.
pub fn now_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000) as u64
}
