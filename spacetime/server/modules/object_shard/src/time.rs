//! Client/server time-discipline contract.
//!
//! Copied verbatim from the `zone_shard` module (schema-independent and
//! load-bearing for the `valid_at` model), so both shards stamp `valid_at` the
//! same way. Time is **milliseconds** throughout: `valid_at` rows pack u48 ms,
//! durations are ms.

use spacetimedb::ReducerContext;

/// Wall-clock now in unix milliseconds. Convert from the microsecond timestamp
/// the SpacetimeDB scheduler provides.
pub fn now_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000) as u64
}

/// Client-server forward time-drift tolerance.
///
/// The client runs its `serverNowMs()` estimate behind the captured server
/// timestamp, so a `client_time_ms` ahead of the server's clock is never
/// legitimate jitter — it's genuine skew, and accepting it would stamp the row
/// in the server's future where a correctly-clocked observer can't see it yet.
/// Set to 0: there is no forward window. (Back-grace is the separate
/// `BACKWARD_GRACE_MS`.) Keep this 0 unless the client's behind-clock invariant
/// changes.
#[allow(dead_code)]
pub const TIME_DRIFT_BUFFER_MS: u64 = 0;

/// Maximum round-trip network latency the contract tolerates. Documented here as
/// the cheat budget under back-grace; the actual back-window is
/// `BACKWARD_GRACE_MS` below. 3000ms covers high-latency dev setups.
#[allow(dead_code)]
pub const MAX_RTT_MS: u64 = 3_000;

/// Static backward-grace window in [`effective_now_ms`]. The server accepts
/// `client_time_ms` up to this many ms behind its own clock, rejecting anything
/// older as `time_drift:client_behind_by`. Sized at 2× the client's maximum
/// self-imposed `clientDelay` (5s), so there's comfortable headroom.
#[allow(dead_code)]
pub const BACKWARD_GRACE_MS: u64 = 10_000;

/// Resolve the time to use for game-logic calculations in a client-invoked
/// reducer.
///
/// Policy:
///  - Reject if `client_time_ms` is more than [`BACKWARD_GRACE_MS`] behind the
///    server's `ctx.timestamp`.
///  - Reject if `client_time_ms` is more than [`TIME_DRIFT_BUFFER_MS`] (= 0)
///    ahead of the server's `ctx.timestamp` — anti-cheat against a client
///    claiming the future to pull rows whose `valid_at` hasn't elapsed.
///  - Otherwise stamp at the client's (behind) time.
///
/// Errors use the `time_drift:` prefix so the client can parse the rejection and
/// schedule a retry once the gap closes:
/// `time_drift:client_behind_by=<N>` or `time_drift:client_ahead_by=<N>`.
#[allow(dead_code)]
pub fn effective_now_ms(ctx: &ReducerContext, client_time_ms: u64) -> Result<u64, String> {
    let server = now_ms(ctx);
    // Client too far in the past → reject. `saturating_sub` guards overflow.
    let behind = server.saturating_sub(client_time_ms);
    if behind > BACKWARD_GRACE_MS {
        return Err(format!(
            "time_drift:client_behind_by={behind} (server={server}, client={client_time_ms})"
        ));
    }
    // Client too far in the future → reject. Forward grace is static (anti-cheat,
    // not lag absorption) — the client runs behind, so any meaningful "ahead" is
    // skew, surfaced rather than clamped.
    let ahead = client_time_ms.saturating_sub(server);
    if ahead > TIME_DRIFT_BUFFER_MS {
        return Err(format!(
            "time_drift:client_ahead_by={ahead} (server={server}, client={client_time_ms})"
        ));
    }
    // Stamp at the client's (behind) time — no `min(client, server)` clamp.
    Ok(client_time_ms)
}
