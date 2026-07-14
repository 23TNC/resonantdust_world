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
  and why. `completed.md` is authoritative and `current` may lag it — but we keep `current`
  because its job is to **cut re-derivation time**: a convenience cache of known state so we
  don't re-investigate what we already established.
- **`plan/`** — **how we close current → design**. Proposed phases, how they tie into other
  components' plans, what blocks forward motion. **`plan` (and `design`/`intent`) hold the FULL
  future intent, not just what current work needs.** When executing, build *toward* the
  documented design/intent — never quietly simplify it away for the task in hand. (We keep a
  complex LOD system with three sprites because the game will have many; the plan is where that
  future intent survives session limits, deviations, and re-planning.) If the plan genuinely
  needs to change, change *the plan doc* with input — don't circumvent it in the code.

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
  blocks (needs a human), suggested path. Open→resolved; resolved rows archive with a date. The
  point: surface *what* blocks me and *why* so you can supply what I need — the goal is fewer
  blockers over time, because a well-understood one becomes an `issue` or `fork` I resolve myself.

A work-folder headers the component(s) + plan phase it executes; plans link back to active work.

## Row conventions

- **Timestamp every row** with the date you **write or modify** it (`YYYY-MM-DD`) — the
  *doc-modification* time, not the git date. This is what lets us trace the line of logic later
  ("this landed three days ago, that two days ago → so this is what happened, and here's what
  needs doing"), and it's how we catch code carrying an older design against a newer methodology.
  The commit is recoverable from the date; the date is the anchor. Datetime only if same-day
  order matters.
- **Order:** newest-first for at-a-glance files (`remaining`, `todo`, `blockers`); chronological
  append for history (`completed`, `issues`, `forks`).
- **The lifecycle:** on starting work → write `todo`; on executing → move to `remaining`; on
  finishing → move to `completed`. Issues/forks/blockers captured as they arise.

`memory/` (agent cross-session hints) stays distinct from `docs/work` (project-durable state) —
memory may *point* to work docs, not duplicate them.
