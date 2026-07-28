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
