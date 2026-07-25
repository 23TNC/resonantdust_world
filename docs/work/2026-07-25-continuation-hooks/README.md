# Continuation hooks — make the "keep going" system robust

_Work stream. Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md). Opened 2026-07-25._

Hardens the **continuation system** — the machinery that stops a turn ending while unblocked,
executable work remains. Built as [`docs-authority`](../docs-authority/README.md) P5/P6; this stream
takes it from "works on the corpus we tested" to "can't silently stop working", and closes the
**planning** gap that no amount of hook engineering can reach.

## Why this stream exists

P6 fixed a hook that had been **silently dead for six days** (it selected the active stream by work-index
status `open`, and the stream being driven was indexed `blocked`). Auditing that fix against all 34
streams immediately found two more defects. The tally is now **eight classification bugs** in a detector
whose entire job is classification:

| Where | Defect |
|---|---|
| P5 (D5) | 3 prose false-matches → forced the redesign to marker signals |
| P6 | bullet-only blocker scan · case-insensitive `resolved` · `None open.` in the body |
| P6a | `- [x]` counted as open (13 phantom items) · 81% of bullets truncated mid-sentence |

**One root cause: the detector infers machine state from prose written for humans, in four different
item dialects.** Every defect was invisible from the code and only showed up by measuring the corpus.
And the failure direction is almost always **fail-open and silent** — two currently-`open` streams
write prose-only todos, so the parser sees *zero work* and can never nudge on them.

## The two things this stream changes

1. **Stop inferring — make item state explicit.** A `- [ ]` / `- [x]` checkbox is the *only* thing
   that counts as an item. That is already the majority dialect (20 of 34 streams), so this codifies
   practice rather than imposing on it, and it retires the whole heuristic layer (DONE-header regexes,
   header casing, bullet-vs-prose) at the root.
2. **Make the plan executable, not just present.** Item granularity across the corpus: **median 142
   chars, p90 181, 67 items over 150**. These are paragraphs describing a phase, not actions. This is
   a *cause* of the stopping behaviour, not a cosmetic issue: a vague next step is itself a reason to
   check in. If the next box reads "wire X to Y, verify at zoom 0.25" I execute; if it reads as a
   paragraph describing a subsystem I *plan*, and planning invites ratification, which is a stop.

Everything else here (visibility, briefs, the mechanism fixes) exists to make those two hold up.

## Design stance

- **The hook is a safety net, not the mechanism.** What actually keeps a session going is an
  unambiguous next action. So plan quality (P4) outranks further hook engineering.
- **Fail loud, not open.** A detector that can't read a stream must *say so*. Silence is the failure
  mode that cost six days.
- **Test against the corpus, never against intuition.** All eight bugs were code-invisible. Every
  change here is measured across all streams before it lands.

## Files
- [`todo.md`](todo.md) — the phased plan (P0–P5).
- [`completed.md`](completed.md) — what landed + how it was verified.
- [`issues.md`](issues.md) — problems hit, options, what was chosen.
- [`forks.md`](forks.md) — the load-bearing decisions.
- [`blockers.md`](blockers.md) — anything needing the user.
- [`deviations.md`](deviations.md) — where the code departs from the plan.

## Related
- [`docs-authority`](../docs-authority/README.md) — built this system (P5/P6); owns `docs-check`,
  the Stop hook, and the front-door index. This stream is its continuation-half, split out because
  it is substantial and multi-phase.
- [`../../CONVENTIONS.md`](../../CONVENTIONS.md) — the convention being enforced.
