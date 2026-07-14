# Docs conventions

How we externalize state so neither of us has to hold it in our heads. Two trees:

## `docs/components/<component>/` — the durable truth about a component

Four folders. Together they answer "what is this, why, where are we, and how do we get there":

- **`design/`** — the **shape**. What the component *is / should be* — the final-state
  specification (tables, types, bit-layouts, protocols). Describes structure, not usage.
- **`intent/`** — the **what + why**. How the component *is / should be used*: entry & exit
  points, what we store into the shape and why, what we read where and why. The rationale
  behind the design.
- **`current/`** — **where we are now**. A short prose snapshot of what's implemented vs not,
  and why. Regenerated from `work/.../completed.md` — a *summary*, never a parallel ledger.
- **`plan/`** — **how we close current → design**. Proposed phases, how they tie into other
  components' plans, what blocks forward motion.

Each file opens with a one-line header naming which of the four it is (keeps `intent` vs
`design` from blurring).

## `docs/work/<work>/` — the flowing execution state

A work-stream turns component `plan`s into executable items. Files (create on demand — an
empty one isn't required; a one-line change needs no work-folder):

- **`todo.md`** — planned, not started. Written from the component plan/design/current when we
  take on a work-stream.
- **`remaining.md`** — actively executing now. Items move here from `todo` when we begin.
- **`completed.md`** — done + verified. Items move here from `remaining`. Append-only history.
- **`issues.md`** — problems hit + candidate solutions + which we chose + why.
- **`forks.md`** — decision points + options + which we chose + why.
- **`blockers.md`** — things needing human input: description, solution analysis, *why* it
  blocks (needs a human), suggested path. Open→resolved; resolved rows archive with a date.

A work-folder headers the component(s) + plan phase it executes; plans link back to active work.

## Row conventions

- **Timestamp every row** `YYYY-MM-DD` (from git commit date where one exists — accurate +
  recoverable; use datetime only when same-day order matters).
- **Order:** newest-first for at-a-glance files (`remaining`, `todo`, `blockers`); chronological
  append for history (`completed`, `issues`, `forks`).
- **The lifecycle:** on starting work → write `todo`; on executing → move to `remaining`; on
  finishing → move to `completed`. Issues/forks/blockers captured as they arise.

`memory/` (agent cross-session hints) stays distinct from `docs/work` (project-durable state) —
memory may *point* to work docs, not duplicate them.
