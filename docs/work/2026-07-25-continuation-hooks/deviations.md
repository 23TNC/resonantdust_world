# Continuation hooks — deviations

_Where the code departs from the plan ([`todo.md`](todo.md) / the component `design`/`intent`).
Logged **at the moment of deviation**. Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

## D1 — unreadable plan files are an ERROR, not the planned WARNING · 2026-07-25

- **Plan says** (P1): "Wire doctor into `docs-check` as a WARNING … Warning-only — it must never
  block a commit."
- **Code does**: `docs-check`'s `work-items` invariant raises an **ERROR** when a plan file has plain
  bullets but no checkbox. The *granularity* half stayed a warning, as planned.
- **Why**: an unreadable plan file is not a style issue, it is the **fail-open hole this whole stream
  exists to close** — its work is invisible to the hook, which then silently allows a stop. A warning
  for that would be advisory guidance about a correctness hole, and the six-day outage is the proof
  that advisory signals about this get missed. It costs nothing today: the corpus is at **0**
  unreadable files after the P0 migration, so the ERROR can only fire on a *new* regression, which is
  exactly when it should. The plan's "never block a commit" concern is preserved for the thing it was
  really about — the 102 oversized items, which are warnings and would have been intolerable as errors.
- **Status**: intentional, kept. Reversible in one line if it ever proves too strict.
