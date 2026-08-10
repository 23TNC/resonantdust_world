# Plan — sim-liveness

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), problems in [`issues.md`](issues.md) (I#)._

**How acceptance is measured.** `bin/sim logs worker`, `bin/sim logs master`, `docker stats`, and
`spacetime sql resonantdust-dev-index-0`. The reproduction is time-based, so several items are
observations over a soak rather than a command that returns.

**EXECUTION ORDER**

| # | phase | why here |
|---|---|---|
| 1 | **P0** | the alarm, before the diagnosis — it is what makes the diagnosis observable |
| 2 | **P1** | find the freeze |
| 3 | **P2** | decide what a lagging worker does (needs the user) |
| 4 | **P3** | the exit |

## P0 — make it visible

- [ ] Log the worker's master-tic reading and its wall-clock age once per compose pass at DEBUG,
      and at WARN past 2 s of no advance. Acceptance: a WARN appears within 3 s of the master
      being stopped.
- [ ] Add the same check to the master: WARN when a worker's acknowledged tic stops advancing
      while the metronome bumps. Acceptance: killing the worker raises it within 3 s.
- [ ] Expose `worker_lag_tics` (master tic − composed tic) in the worker's periodic report, so a
      soak can read it without parsing compose lines. Acceptance: one grep yields the series.
- [ ] Give `bin/sim` a `liveness <crate>` subcommand printing tic, lag and last-advance age.
      Acceptance: it reports HEALTHY on a live world and STALLED on a stopped master.

## P1 — find the freeze

- [ ] Reproduce it deliberately: run a worker against a master paused mid-soak, resume, and record
      whether the reading recovers. Acceptance: a dated note in `issues.md` either way.
- [ ] Instrument the subscription itself — count rows delivered per second on the index clock
      table. Acceptance: the count is visible in the worker's report during a stall.
- [ ] Establish whether the SDK subscription is dead or the worker stops draining it, by logging
      both callback arrivals and loop iterations. Acceptance: the two counters distinguish the
      cases in a captured stall.
- [ ] Fix or escalate what that finds. Acceptance: either a fix with the stall no longer
      reproducing over a 30-minute soak, or a `blockers.md` row explaining why it is upstream.

## P2 — decide what a lagging worker does

- [ ] Measure whether recovery is possible at all: from a known backlog, record drain rate against
      realtime. Acceptance: a table of backlog vs drain rate in `completed.md`.
- [ ] Put the options to the user — shed, bound, or declare unhealthy — with the measured cost of
      each. Acceptance: a resolved fork in `forks.md`.
- [ ] Implement the chosen policy. Acceptance: an induced 2000-tic backlog reaches the policy's
      stated end state rather than grinding.

## P3 — the exit

- [ ] Add the liveness assertion to the soak harness shared-simulation uses, so a measurement that
      spans a stall fails instead of reporting. Acceptance: a soak over an induced stall reports
      FAILED, not a number.
- [ ] Write the exit record and update `docs/work/README.md`. Acceptance: docs-check green.
