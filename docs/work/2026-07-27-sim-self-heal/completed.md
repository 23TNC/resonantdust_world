# Completed — sim-self-heal

## 2026-07-28 · P1 — `server/uplink` (`resonantdust-uplink`)

`Uplink<C, S>`: closure-built (F3 — no SDK types named, unit-testable with a fake `C`),
`get()`-only API returning the live conn or rebuilding it, subscribe-and-wait gating (a conn
that dies DURING subscribe is also rejected), capped doubling backoff 0.5 s → 8 s with
attempt-count warn logs and a "reconnected after N attempts" info. The `alive`-from-three-
places rule (disconnect + connect-error + subscription on_error) is the caller's build-closure
contract, documented at the top of the crate. Verified: 4/4 unit tests green in docker
(recover-after-down, dead-flag-triggers-exactly-one-rebuild, get-waits-for-subscribe,
backoff-schedule-with-cap under tokio's paused clock — which found a real bug: the backoff
originally used `std::time::Instant`, invisible to tokio's virtual clock; now `tokio::time::
Instant`). NOTE: the plan's acceptance said `bin/rd check` — no such command exists; the
equivalent gate used is in-docker `cargo test`/`cargo check` via the sim builder image.

## 2026-07-28 · P2 — master converted + three live drills

All 6 connections are `Uplink`s (`shard_uplink!` macro stamps the build closure with `alive`
wired into connect-error + disconnect; the index uplink carries the `master_clock`
subscribe-and-wait with sub `on_error` clearing the SAME flag). The loop `acquire`s per
upstream and logs only up/down TRANSITIONS (the uplink's backoff paces its own warns);
`set_orchestrator` re-stamps per event-shard connection GENERATION (a republished shard
forgets it — the standup-only call was itself a latent bug). A dead index skips the pass;
a dead shard skips only its own bump/sweep.

Drills, all verified live with zero master restarts:
1. **All-up:** tic 31398→31411 advancing, orchestrator stamped via the generation path.
2. **Partial outage:** `spacetime delete …-pawn-0` mid-run → ONE "uplink down — skipping its
   work" warn, tile clock kept ticking (31566→31579); republish → "uplink up — resuming",
   fresh pawn clock immediately riding the metronome (31762→31775). This is the first-pawns
   I1 void-write, cured for the master.
3. **Cold start:** master started with the DAEMON stopped → 6 paced retry warns, no panic;
   daemon started → all uplinks up within seconds, tic resumed from the durable row
   (31906→31919), container "Up 21 seconds" — never restarted.
