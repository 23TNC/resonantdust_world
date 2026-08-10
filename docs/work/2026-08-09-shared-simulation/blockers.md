# Blockers — shared-simulation

_Things that genuinely need human input: what blocks, why it needs a human, suggested path.
Newest-first; resolved rows archive with a date._

## B2 — the world cannot catch up; P0's baseline needs a reset
**2026-08-10. RESOLVED same day** — user: *"You can reset the world, I see no reason why I must be
involved."* Full wipe + republish via `bin/rd redeploy --run` (sim processes stopped first so they
could not race the wipe), then the sim restarted and the P0 baseline re-run.

**The lesson for this stream, recorded because I got the call wrong.** I treated "resetting a dev
world" as needing sign-off on the grounds that a parallel session was using it. The user's answer
says that was over-caution: a local dev world whose state resets cheaply is not shared production,
and the cost of asking (a stalled stream, a turn spent) exceeded the cost of being wrong. The line
worth keeping is narrower than the one I drew — **confirm before destroying something expensive or
irreplaceable, not before every destructive verb.** Dev-world state is neither.

_Original analysis, kept because the diagnosis stands and the failure mode will recur:_

**State.** The orchestrator is healthy and assigning at tic **42052**. The worker is pegged at
**~107% CPU** and composing **6 components per 30 s**, all around tic **32200** — roughly **9,850
tics (~27 minutes) behind, and losing ~5 tics/s.** It is not draining a backlog; it is falling
further behind while working flat out. A restart does not help: the backlog lives in the event
shard, not in the process.

**How it got here.** Lag sat at 0 ±2 for hours, hit a cliff at ~01:34 (0 → 29 → 2333 in three
samples), and never recovered. [I6](issues.md#i6) covers the clock-read freeze that accompanied it.

**Two candidate causes, which I cannot separate without a clean world:**

1. **A backlog death-spiral** — once far enough behind, per-tic compose cost exceeds realtime and
   recovery is impossible by construction. This is the failure mode
   [`mover-perf`](../2026-08-08-mover-perf/README.md) was chartered to characterise.
2. **A perf regression in `shared/content` from live-edit P3/P4.** The worker I rebuilt at 01:56
   is the *first* one to contain that code (the previous binary was 5 hours old and predated it).
   It came up at 100% CPU immediately. This is a **candidate, not a claim** — the old binary was
   already struggling before I touched it, so the timing is suggestive and nothing more.

**Why it needs you, not me.** Clearing it means **resetting world state**, which destroys the
current population (86 bunnies, 3 wolves) and interrupts whatever the parallel live-edit session is
looking at. You are actively working this same world from another session; that is not a call I
should make unilaterally, and it is not cheaply reversible once done.

**Recommendation.** Reset, then re-measure — the P0 baseline is worthless taken against a sim
running 27 minutes in arrears, because every authoritative row arrives stamped with a tic the
client's clock left long ago, and the divergence I would record would be the backlog, not the
walk. If you'd rather I chase candidate (2) first — bisect the worker against the pre-live-edit
`shared/content` — say so; that is a real answer too, and arguably the more valuable one, but it
belongs to live-edit's stream rather than this one.

**Meanwhile:** P0's probe code is written, typechecks and builds; only its acceptance
(`report()` printing quantiles off live samples, and the 10-minute baseline) is blocked.

## B1 — a worker clock-subscription freeze that nothing detects
**2026-08-10. Open — needs your call on ownership, not on how to fix it.**

**What happened.** Mid-P0 the worker's view of the master tic froze at 33461 for 12+ minutes while
`rd-master` kept bumping at a clean 6 Hz (`bump_confirmed=360`, `bump_failed=0`). The worker did
not crash, log an error, or self-heal; it sat at 115% CPU composing a backlog whose far end had
stopped advancing. Full evidence in [`issues.md` I6](issues.md#i6). I restarted the worker and the
stream continues.

**Why it needs you.** Two things I can act on and one I shouldn't decide alone:

- It is **not this stream's subject.** The fix belongs to whoever owns the uplink/subscription
  layer — `sim-self-heal`'s rebuild-on-next-use is the machinery that was supposed to cover this.
- The detection story is the worrying part: **nothing alarmed.** I found it only because a probe
  returned zero rows. A frozen-but-live subscription is indistinguishable from a paused world, so
  there is no error to catch — it needs a *liveness* check (worker's master reading advancing vs
  wall clock), which is a design decision about where that watchdog lives.
- It directly threatens this stream's long soaks. P0's baseline and P4's re-measure are both
  10-minute runs; if this recurs on an hourly timescale it will silently corrupt them.

**Recommendation.** Open a separate stream for the watchdog + root cause, and have this stream's
soaks assert the worker's `master=` advanced across the run — cheap, and it turns a silent
corruption into a failed measurement. I can add that assertion here without waiting on you; say
the word if you'd rather I chase the root cause now instead of continuing shared-simulation.

## Resolved

**2026-08-09 — none at open.**

Everything the stream has hit so far is a decision I could make and did — recorded in
[`forks.md`](forks.md) F1–F5 — or a problem with a chosen path, recorded in
[`issues.md`](issues.md) I1–I5. The stream is executable as written.

One thing to flag rather than block on, because it is the user's call and not mine: **P4 deletes
working code.** `MoverLayer`'s speculation is the only reason movers glide today, and it carries
guards earned live (the stale-intent and older-row rejections — the teleport verdict). The plan
ports those into `client/core` at P2 and proves the track headless at P3 before P4 removes
anything, and [F4](forks.md#f4) argues against keeping a flagged fallback. If you would rather
carry the TS path behind a flag for a release, say so and F4 gets rewritten — it is a reversible
decision and cheap to change *before* P4, expensive after.
