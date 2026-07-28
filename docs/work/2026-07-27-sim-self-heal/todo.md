# sim-self-heal — todo

_Plan for the stream (see [README.md](README.md)). Items are checkboxes; the box is the move.
Opened 2026-07-27._

## P1 — the shared `uplink` helper

- [ ] Create `server/uplink` (`resonantdust-uplink`): `Uplink<C>` = build closure + `alive: Arc<AtomicBool>` + optional async subscribe closure; `get()` returns the live conn or rebuilds ([F1](forks.md#f1)/[F3](forks.md#f3)). Acceptance: `bin/rd check` green.
- [ ] Clear `alive` from `on_disconnect`, `on_connect_error`, AND the subscription `on_error` (the gateway pattern misses the third). Acceptance: unit test — while the build closure fails, `get()` errs; once it succeeds, the next `get()` recovers.
- [ ] Gate every rebuild on subscription re-applied (subscribe-and-wait, 5s timeout) so `get()` never returns a conn with an un-applied cache. Acceptance: unit test — `get()` resolves only after the fake subscribe closure completes.
- [ ] Add capped backoff between failed rebuilds (0.5s → 8s) with attempt-count warn logs so a down shard doesn't hot-loop. Acceptance: unit test — N rapid `get()`s during failure invoke the build closure only per the backoff schedule.

## P2 — master

- [ ] Replace master's 5 `.expect("build … connection")` + `rx_i.recv_timeout(…).expect` with `Uplink`s (index carries the `master_clock` sub; the 5 shard conns — event/data/pawn/tile/thing — are sub-less). Acceptance: `bin/rd check` green; all-shards-up behavior unchanged (the wolf moves).
- [ ] Per pass: a dead shard uplink skips only ITS bump/sweep (warn once per transition, not per tic); a dead index skips the pass. Acceptance: stop one shard DB → master keeps ticking the rest, no panic; restart it → its bumps resume.
- [ ] Cold-start check: start master with SpacetimeDB down, bring SpacetimeDB up after. Acceptance: retry warns, then "metronome starting", and `index.master_clock.tic` advances — with no restart of master.

## P3 — worker + orchestrator

- [ ] Convert worker's 6 connections (incl. pawn) + its recv/panic ladder to `Uplink`s; a pass needing a dead uplink defers the tic (writes are absolute + replay-safe, so re-composition after a resub is correct — and a MOVE_TO continuation lost to a dead event uplink is re-issued by the npc's trip deadline). Acceptance: `bin/rd check` green; the wolf moves.
- [ ] Convert orchestrator's 5 connections (incl. pawn) + 2 recv `.expect`s likewise; keep `assigned` as-is (assign/claim are idempotent; the prune already tolerates an empty cache). Acceptance: check green; grouping resumes after an event-shard restart.
- [ ] Chaos check: restart the SpacetimeDB container mid-run. Acceptance: master, worker, and orchestrator all reconnect WITHOUT process restarts and the wolf resumes moving in the browser.
- [ ] Republish check (the first-pawns I1 void-write): `rd redeploy` a module while the trio runs. Acceptance: the trio's uplinks drop + rebuild against the fresh DB and the next compose LANDS (row visible via spacetime sql) — no trio restart.

## P4 — client-side heal (npc)

- [ ] `client/core` native engine: reconnect on WS death — surface `Disconnected`, then re-run the login flow (gateway → connect → login → re-anchor) with capped backoff; zone subs re-issue from the anchor manager. Acceptance: kill + restart the edge under a running npc → the npc re-logs-in and trips resume, no container restart.
- [ ] Until (or besides) reconnect: `run_brain` EXITS on `Event::Disconnected` so the container stops visibly instead of zombie-issuing into a dead socket (observed live 2026-07-28). Acceptance: kill the edge → `rd-npc` exits within seconds; `bin/sim ps` shows it gone.
- [ ] `bin/sim run npc` gains `--restart=on-failure` (or a `sim run --keep` flag using docker's restart policy) so an exited npc comes back by itself once the edge is up. Acceptance: bounce the edge → the npc container restarts and the wolf resumes without any manual step.

## P4b — edge lock poison

- [ ] Replace every RwLock `.read().unwrap()` / `.write().unwrap()` in `server/edge` (`content.rs`, `connections.rs`, `tex_manifest.rs`, …) with poison-recovering access via one helper (`unwrap_or_else(|e| e.into_inner())`). Acceptance: grep finds 0 sites; `bin/rd check` green.

## P5 — hardening development redeploys (user, 2026-07-28)

- [ ] `bin/lib/redeploy.sh`: EXCLUDE `target/` (and other build artifacts) from module input-hashes — a mere in-tree build must never re-trigger a data-wiping republish (bit the pawn shard twice, first-pawns I1). Acceptance: `rd redeploy` after `rd build spacetime <mod>` reports "changed: (none)".
- [ ] `rd redeploy`: after ANY module publish, bounce the running sim consumers — `rd-master`/`rd-orchestrator`/`rd-worker`/`rd-npc` restarted via `bin/sim run` if their containers exist (idempotent; replay-safe by design), with a log line naming what was bounced and why. Acceptance: republish a shard mid-run → the next compose lands in the fresh DB with zero manual steps.
- [ ] `rd redeploy` edge: also bounce `rd-npc` if running (its session strands until P4 reconnect lands) + print a reminder that browser sessions need a reload. Acceptance: edge redeploy under a running npc → the wolf resumes within ~10 s.
- [ ] `bin/sim`: `cmd_build` failure must FAIL LOUDLY and `cmd_run` must refuse a binary older than the crate's newest source file (override with `FORCE=1`) — a failed build silently running yesterday's binary cost two debugging loops (2026-07-28). Acceptance: touch a src file, skip the build, `sim run` refuses; after `sim build` it runs.
- [ ] `bin/sim build` guards the docker mtime miss: if cargo reports `Finished` with no `Compiling <crate>` line after a source change, warn loudly (suggest `touch`). Acceptance: reproduce the miss (or simulate) → the warning fires.

## P6 — wrap

- [ ] Record evidence in this stream's `completed.md` (commands + observed log lines for the P2/P3 chaos checks + the P5 redeploy drills) and note the gateway→`uplink` migration as an out-of-scope follow-up. Acceptance: `bin/rd docs-check` green.
