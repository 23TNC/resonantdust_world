# Forks — improvement audit

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — the deliverable is ONE ranked inventory, in the stream folder {#f1}
_2026-08-08 · resolved at plan time_

**Chosen.** `findings.md` here: one entry per candidate —
`status · impact · cost · evidence · recommended successor shape`. The
stream folder is where it lives because the findings are a SNAPSHOT (dated,
superseded by whatever streams they spawn), not standing law.

**Rejected — a docs/notes/ document**: notes/ carries durable reference; a
ranked to-consider list goes stale the day the first successor lands and
would need curation forever. **Rejected — filing each candidate as its own
issue somewhere**: scatters the exact thing this stream exists to gather.

## F2 — six enumerated source lanes {#f2}
_2026-08-08 · resolved at plan time — so coverage is checkable, not vibes_

1. **Work-stream debt**: every OPEN issues.md entry and named successor
   across docs/work/ (archived streams excluded — I2).
2. **Docs drift**: docs-check warnings (stale current/ stamps, the 538
   oversized plan items), plus authoritative-doc claims that code has
   outgrown.
3. **Perf headroom**: the open questions the two perf streams named (the
   real worker ceiling, the 256-event burst mechanism, eviction density,
   the parked-event delay).
4. **Code sweeps**: greps for TODO/FIXME/HACK, unwrap/expect in server
   paths, dead code the compiler flags, commented-out blocks.
5. **Architecture gaps**: things design/intent/memory NAME as future and
   nothing schedules — ownership model, event retention, the second worker,
   pixijs retirement, the palette generator, runtime thing-traits,
   fall_off's consumer, item-as-entity.
6. **DX friction**: session-observed toil — the docker mtime miss, ANSI in
   sim logs, the live-check need for schema changes, the SQL decode
   ceremony.

**Rejected — an open-ended "look around"**: unfalsifiable coverage; six
lanes make "done" checkable.

## F3 — the verification bar {#f3}
_2026-08-08 · resolved at plan time_

Every candidate carries exactly one status:
- **VERIFIED-OPEN** — evidence it still bites (a log line, a grep hit, a
  failing invariant, a doc/code mismatch shown).
- **STALE** — closed or superseded since it was written; kept as a one-line
  tombstone so the sweep is auditable, then ignored.
- **UNKNOWN** — verification would cost real work; probed ONLY when the
  probe is cheap and READ-ONLY (a grep, a log read, an SQL count — never a
  drill that mutates the world). Stated as unknown otherwise — an honest
  unknown outranks a guessed status.

## F4 — the ranking rubric, fixed before ranking {#f4}
_2026-08-08 · resolved at plan time_

Impact classes, in strict order: **A** player-visible correctness (wrong
render, teleport-class) · **B** consistency/data risk (drift, double-writes,
loss) · **C** performance ceilings (known walls ahead of content growth) ·
**D** velocity & DX (what makes every session slower) · **E** polish. Cost
is coarse S/M/L (a session / a stream / several streams). The rank is
impact-then-cost; ties broken by "unblocks other items". The rubric is
authored here so the ranking is reproducible, not taste.
