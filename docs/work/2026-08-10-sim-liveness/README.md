# sim-liveness — the world stops and nothing says so

_Split out of [shared-simulation](../2026-08-09-shared-simulation/README.md) on 2026-08-10 (user:
"B1 needs a new work folder written and doesn't block this sequence of work"). That stream hit this
twice while trying to measure something else, worked around it twice, and it is not that stream's
subject._

## The thing

**The worker stops advancing and reports nothing.** Twice on 2026-08-09/10, hours apart:

- **The clock read froze.** `rd-master` was healthy throughout — `achieved_hz` 5.9–6.2,
  `bump_confirmed=360`, `bump_failed=0`, every minute, continuously. The worker's `master=` reading
  sat at **33461 for 12+ minutes**, across which the master bumped ~4,300 tics. Every
  `composed component` line in that window carried the same stale number. The worker did not crash,
  did not log an error, and did not self-heal.
- **It burned CPU while doing it.** ~115%, composing components at tics *behind* where it had
  already been (33098 at 01:34 → 32036 at 01:52) — grinding a backlog whose far end had stopped
  moving toward it.
- **Onset was a cliff, not a ramp.** Lag sat at 0 ±2 for hours, hit 29, then 2333 on the next
  sample, and never recovered.
- **A restart did not fix it**, because the backlog lives in the event shard, not the process. Only
  a full state reset did.
- **It recurred.** ~10 hours after that reset, the worker was ~4,000 tics behind again with the
  same signature.

## Why it is worth its own stream

**Nothing alarmed.** Not once. The only reason it was ever noticed is that an unrelated probe
returned zero rows, twice. A frozen-but-live subscription is indistinguishable from a paused world:
there is no error to catch, no exception to log, no failed call to retry. The tree already has
rebuild-on-next-use uplinks for exactly this class of fault
([sim-self-heal](../2026-07-27-sim-self-heal/README.md)) and they did not fire — correctly, by
their own logic, because nothing *failed*.

That is the actual defect: **the system's health check is "did a call error", and the failure mode
is "no calls happen at all".** Liveness is not the absence of errors.

It also has a compounding shape. Once far enough behind, per-tic compose cost appears to exceed
realtime and recovery becomes impossible by construction — the death-spiral
[mover-perf](../2026-08-08-mover-perf/README.md) was chartered to characterise. So a brief stall
becomes a permanently dead world, and the only remedy anyone has found is a reset.

## The stance

Three things, in order, and the first is not the fix — it is the alarm.

1. **Make it visible.** A liveness check that asserts the worker's *view of the master tic* is
   advancing against wall time, not merely that its calls succeed. This is cheap, it is the thing
   whose absence cost this much, and it turns a silent corruption into a loud one. It also unblocks
   honest measurement everywhere else: shared-simulation's soaks are 10 minutes long and currently
   have no way to know the world went quiet underneath them.
2. **Find the freeze.** Whether the subscription genuinely dies, whether the SDK's re-subscribe
   path has a hole, or whether the worker's own loop stops draining it. Unknown today; the evidence
   above is all there is.
3. **Decide what a lagging worker should DO.** Right now it grinds forever. Options span shedding
   the backlog, refusing to compose past a bound, and declaring itself unhealthy so something else
   can act. This is a design decision, not a bug fix, and it wants the user.

**Out of scope:** making the worker faster. This is not a performance stream — a worker that keeps
up is [mover-perf](../2026-08-08-mover-perf/README.md)'s subject. This one is about a world that
has already stopped and says nothing.

## Exit

The worker cannot go quiet without something saying so within a tic or two; the freeze's cause is
known and fixed or knowingly accepted; and a soak can assert that the world stayed live for its
whole duration.
