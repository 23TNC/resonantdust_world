# Continuation hooks — completed

_Done + verified. Items move here from [`todo.md`](todo.md). Append-only; authoritative for what's
done. Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

- **2026-07-25** · **P0 — the checkbox contract.** An item is now `- [ ]`/`- [x]` and nothing else;
  every heuristic that guessed item state from prose is gone (`DONE_HEADER` scanning, bullet-vs-prose,
  the first-line truncation). Migrated **42 items across 5 files** to checkboxes (top-level bullets
  only — nested bullets are detail, which is exactly the rule the detector implements). Auditing
  before flipping the rule found **15 unchecked boxes under DONE headers**: 7 were lying boxes in
  `mrt-bakes`/`caster-lut` ("DONE (moved to completed.md)", never ticked) → ticked; 8 were real
  deferred work under a delivered phase → left open ([`forks.md` F5](forks.md#f5)). `docs-check` gained
  the **`work-items`** invariant, and the progress guard now leads with `(open, done)` box counts, so
  ticking a box is *unambiguous* progress rather than an mtime inference.
  **Verified:** 0 unreadable plan files · 0 phantom open items · counts match a naive recount on all
  35 streams.

- **2026-07-25** · **P1 — failure is visible now.** `.git/rd-work/decisions.jsonl` records every Stop
  verdict (`arm`/`nudge`/`allow`/`release`/`disarm` + why + box counts), and `rd work doctor` prints
  what the parser actually sees per stream with a `PARSE?` flag. The six-day outage was invisible
  because a broken no-op and a healthy "nothing to do" look identical from outside; they no longer do.
  Deviation [D1](deviations.md#d1): the unreadable-plan case landed as an **ERROR**, not the planned
  warning — it is the fail-open hole the stream exists to close, and it costs nothing at 0 occurrences.

- **2026-07-25** · **P2 — arming: the hook is off until you say go** (user's design,
  [`forks.md` F6](forks.md#f6)). `rd work arm|disarm|status`, and the hook nudges **only** when armed.
  This deleted the planned execution-intent gate, which would have guessed at intent from side effects
  — the same mistake as inferring item state from prose. It also removes the entire conversational
  false-positive class: a question asked mid-stream can no longer drag the asker back into stream work.
  Auto-disarms on all three real exits (complete · blocker recorded · `.stop-reason`), and a stall now
  **writes `.stop-reason` itself** and disarms instead of releasing silently.
  **Verified matrix:** unarmed+open work→silent · armed→block · nudge 1,2,3→block · 4th→release +
  `.stop-reason` written + disarmed · post-stall→silent · blocker mid-stream→disarm + silent.

- **2026-07-25** · **P3 — the nudge hands over a brief, not a reminder.** `rd work brief <stream>`
  assembles phase, next items in full, open blockers, what just landed, and what to read.
  **1130 bytes for `primitive-graph`, against ~45KB across 6 files to reconstruct by hand** — that cost
  was being paid on every resume, which is why a nudge produced re-planning rather than execution.

- **2026-07-25** · **P4 — planning assist.** `docs-check` now warns on items over 250 chars once
  **wrapped continuation lines are folded in** (measuring the first line alone found 0 — 81% of items
  wrap). It flags **102 items** tree-wide, warning-only ([F3](forks.md#f3)). Added the `/rd-plan` skill
  (decompose a phase into one-action items each with an acceptance criterion; never arms the hook,
  because planning is a conversation) and `/rd-execute` (resolve → arm → brief → execute to a real exit).

- **2026-07-25** · **P5 — items never move; `[x]` is the move** ([F7](forks.md#f7).1). `CONVENTIONS.md`
  now states the item contract and retires the half-practised `todo → completed` transfer;
  `completed.md` is explicitly the **verification log** (what landed + how checked), not a second copy
  of the list. This matches what the tree already did — 8 of 35 `todo.md` files were marking phases
  `✅ DONE` in place with a pointer rather than moving anything.

- **2026-07-25** · **P6 — a standing guard.** `bin/lib/work_check_selftest.py` asserts the classifier
  against the real corpus *and* pins the exact shapes that fooled earlier versions (prose blocker
  marked OPEN · `✅ RESOLVED` · `None open.` in the body · `[x]` vs `[ ]` · wrapped lines · a box under
  a DONE header · bullets-without-boxes). All 12 checks pass. Its own first run **caught a bad
  assertion in itself** — the test matched a literal `[x]` written inside this stream's item prose —
  which is the lesson restated: assert on parsed state, never on text that documents the parser.
