# sim-self-heal — todo

_Plan for the stream (see [README.md](README.md)). Items are checkboxes; the box is the move.
Opened 2026-07-27._

## P1 — the shared `uplink` helper

- [ ] Create `server/uplink` (`resonantdust-uplink`): `Uplink<C>` = build closure + `alive: Arc<AtomicBool>` + optional async subscribe closure; `get()` returns the live conn or rebuilds ([F1](forks.md#f1)/[F3](forks.md#f3)). Acceptance: `bin/rd check` green.
- [ ] Clear `alive` from `on_disconnect`, `on_connect_error`, AND the subscription `on_error` (the gateway pattern misses the third). Acceptance: unit test — while the build closure fails, `get()` errs; once it succeeds, the next `get()` recovers.
- [ ] Gate every rebuild on subscription re-applied (subscribe-and-wait, 5s timeout) so `get()` never returns a conn with an un-applied cache. Acceptance: unit test — `get()` resolves only after the fake subscribe closure completes.
- [ ] Add capped backoff between failed rebuilds (0.5s → 8s) with attempt-count warn logs so a down shard doesn't hot-loop. Acceptance: unit test — N rapid `get()`s during failure invoke the build closure only per the backoff schedule.

## P2 — master

- [ ] Replace master's 4 `.expect("build … connection")` + `rx_i.recv_timeout(…).expect` with `Uplink`s (index carries the `master_clock` sub; the 4 shard conns are sub-less). Acceptance: `bin/rd check` green; all-shards-up behavior unchanged (wolves move).
- [ ] Per pass: a dead shard uplink skips only ITS bump/sweep (warn once per transition, not per tic); a dead index skips the pass. Acceptance: stop one shard DB → master keeps ticking the rest, no panic; restart it → its bumps resume.
- [ ] Cold-start check: start master with SpacetimeDB down, bring SpacetimeDB up after. Acceptance: retry warns, then "metronome starting", and `index.master_clock.tic` advances — with no restart of master.

## P3 — worker + orchestrator

- [ ] Convert worker's 5 connections + its recv/panic ladder to `Uplink`s; a pass needing a dead uplink defers the tic (writes are absolute + replay-safe, so re-composition after a resub is correct). Acceptance: `bin/rd check` green; wolves move.
- [ ] Convert orchestrator's 4 connections + 2 recv `.expect`s likewise; keep `assigned` as-is (assign/claim are idempotent; the prune already tolerates an empty cache). Acceptance: check green; grouping resumes after an event-shard restart.
- [ ] Chaos check: restart the SpacetimeDB container mid-run. Acceptance: master, worker, and orchestrator all reconnect WITHOUT process restarts and wolves resume moving in the browser.

## P4 — edge lock poison

- [ ] Replace every RwLock `.read().unwrap()` / `.write().unwrap()` in `server/edge` (`content.rs`, `connections.rs`, `tex_manifest.rs`, …) with poison-recovering access via one helper (`unwrap_or_else(|e| e.into_inner())`). Acceptance: grep finds 0 sites; `bin/rd check` green.

## P5 — wrap

- [ ] Record evidence in this stream's `completed.md` (commands + observed log lines for the P2/P3 chaos checks) and note the gateway→`uplink` migration as an out-of-scope follow-up. Acceptance: `bin/rd docs-check` green.
