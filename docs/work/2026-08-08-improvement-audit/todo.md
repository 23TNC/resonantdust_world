# Plan — improvement audit

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md),
decisions in [`forks.md`](forks.md) (F#), the anticipated-issue inventory in
[`issues.md`](issues.md) (I#)._

## P0 — the scaffold

- [x] `findings.md` scaffold: the dated-snapshot header (I5), the rubric
      (F4), the six lanes as sections (F2), the "deliberate postures"
      section (I1). Acceptance: docs-check green with the new file linked.

## P1 — the sweep

- [x] Lane 1 — work-stream debt: every OPEN issues.md entry + named
      successor across docs/work/, as candidates with their origin cited.
      Acceptance: every open stream folder appears in the lane's coverage
      list, even if it contributed nothing.
- [x] Lane 2 — docs drift: the docs-check warnings (stale current/ stamps,
      oversized plan items) + any authoritative-doc claim the code has
      outgrown that the sweep trips over. Acceptance: each warning becomes a
      candidate or a stated non-issue.
- [x] Lane 3 — perf headroom: the worker ceiling, the 256-event bursts, the
      eviction/density lever, the parked-event delay — each with its
      evidence pointer into the perf streams. Acceptance: the four named
      questions are entries.
- [x] Lane 4 — code sweeps: TODO/FIXME/HACK greps, unwrap/expect in live
      server paths, compiler-flagged dead code (I4's bound). Acceptance:
      grep commands recorded beside their candidate counts.
- [x] Lane 5 — architecture gaps: the named-but-unscheduled futures
      (ownership, retention, second worker, pixijs retirement, palette,
      runtime thing-traits, fall_off consumer, item-as-entity, …) from
      design/intent/memory. Acceptance: each entry cites where it was named.
- [x] Lane 6 — DX friction: the session-observed toil list (mtime miss,
      ANSI logs, live-check gap, SQL decode ceremony, …). Acceptance:
      entries carry the concrete incident they cost time in.

## P2 — triage and rank

- [x] Statuses per candidate (F3): VERIFIED-OPEN with evidence / STALE
      tombstoned / UNKNOWN with the cheap read-only probe run where one
      exists. Acceptance: no candidate is status-less; probes were
      read-only.
- [x] The RANKED table (F4) at the top of findings.md: impact class, cost,
      recommended successor shape, "unblocks" ties noted. Acceptance: the
      document stands alone; docs+memory truth pass; **the user's eyes
      close the stream** and choose what becomes work.
