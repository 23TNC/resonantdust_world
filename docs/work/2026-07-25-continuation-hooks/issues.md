# Continuation hooks — issues

_Problems hit, candidate solutions, which we chose and why. Convention:
[`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

## I1 — The detector's input is prose in four dialects (the root cause)

Measured across all 34 streams (2026-07-25): `checkbox` 20 · `prose-only` 9 · `bullet` 4 ·
`bullet,checkbox` 1. A parser over that surface has produced **eight** classification bugs, every one
invisible from the code. Options considered:

- **(a) Keep hardening the regexes** — rejected. Eight bugs in two sessions is the trend line, and each
  new work folder invents a new dialect. It also fails *open*, so the next dialect drift is silent.
- **(b) A structured sidecar per stream** (`state.json` generated from the markdown) — rejected as the
  primary move: it duplicates state, and the duplicate will drift from the prose exactly like
  `current/` drifts from code (the phantom-project trap CONVENTIONS already warns about).
- **(c) One explicit marker the human already writes** — **CHOSEN.** `- [ ]`/`- [x]` is already the
  majority dialect, needs no new file, is readable, and collapses the classifier to a single regex.
  Prose stays prose; only checkboxes are items.

## I2 — Item granularity is a *cause* of premature stopping, not a cosmetic problem

Corpus: median item 142 chars, p90 181, **67 items ≥150 chars**, 81% wrapping onto continuation lines.
Items describe phases, not actions (`primitive-graph` P3's first item is three engineering tasks in one
bullet, with no acceptance criterion). A vague next step is itself a reason to check in — the session
must re-plan before it can act, and planning invites ratification, which is a stop. So P4 (planning
assist) is not a nice-to-have; it addresses a cause that no hook can reach.
