# Docs conventions — how we document

How we externalize state so neither of us has to hold it in our heads, and so hours of design
survive session limits, deviation, and re-planning. This file is authoritative for *how we
document*; the actual content lives in the trees below.

Three trees: **`docs/components/`** (durable truth per component), **`docs/intent/`** (cross-
cutting feature intent not yet ascribed to components), and **`docs/work/`** (flowing execution
state). Plus **`memory/`** (agent cross-session hints — distinct; may *point* to these, never
duplicate them).

---

## What is a component

A **component** is a self-contained chunk we **deploy** or **share**. Shared components get an
entry too, because *how they change dictates how their consumers operate*.

- **Groups** (mirrors the repo): `client/`, `server/`, `server/spacetime/{modules, pipeline}`,
  `shared/`, and `dev/` (developer + gameplay: the scripts + specs + assets that are the
  foundation of how we build — `rd`, `art`, `dsl`, the `.rd` content specs, sprite templates).
- **Not components:** an **individual table** (the *module* that holds it is the component); a
  **build output** (`shared/pkg`, `shared/target`); a **piece of a component** (worldgen ⊂ edge).
- **Cross-cutting concepts** (e.g. the event-DSL, defined across `shared/codec` + `shared/tick`)
  are documented where they live, not minted as fake deployables.

The full list — every component, its path, what it deploys/shares, and alive-vs-dead — is the
**map: [`docs/components/README.md`](components/README.md)**. Keep it current; it's the antidote
to "I forget which is why we need this."

## `docs/components/<group>/<component>/` — the durable truth about a component

Four folders. Together they answer "what is this, why, where are we, and how do we get there."
Each file opens with a one-line header naming which of the four it is (keeps `intent` vs `design`
from blurring). Folders are created **lazily** — as we actually work a component, not up front.

- **`design/`** — the **shape**. What the component *is / should be* — the final-state
  specification (tables, types, bit-layouts, protocols). Describes structure, not usage.
- **`intent/`** — the **what + why**. How the component *is / should be used*: entry & exit
  points, what we store into the shape and why, what we read where and why. The rationale behind
  the design. (`design` holds the shape; `intent` holds what goes in the shape, and why.)
- **`current/`** — **where we are now**. A short prose snapshot of what's implemented vs not, and
  why. `completed.md` is authoritative and `current` may lag it — but we keep `current` because
  its job is to **cut re-derivation time**: a convenience cache of known state so we don't
  re-investigate what we already established.
- **`plan/`** — **how we close current → design**. Proposed phases, how they tie into other
  components' plans, what blocks forward motion. **`plan` (and `design`/`intent`) hold the FULL
  future intent, not just what current work needs.** When executing, build *toward* the
  documented design/intent — never quietly simplify it away for the task in hand. (We keep a
  complex LOD system with three sprites because the game will have many; the plan is where that
  future intent survives session limits, deviations, and re-planning.) If the plan genuinely
  needs to change, change *the plan doc* with input — don't circumvent it in the code.

## `docs/intent/<feature>/` — cross-cutting feature intent (staging)

Some features (e.g. **pathfinding**) touch many components — implementing them threads through
several pieces. Their intent belongs in the **`intent/` of each component they touch**. But when
we don't yet know which components a feature touches, or don't have enough information to ascribe
it, we **park it here** — a top-level staging area for feature-level intent. As the feature gets
scoped, its intent **distributes out** into the affected components' `intent/` folders (leaving a
pointer here, or retiring the staging entry). This is where a half-formed "we want pathfinding
to…" lives before it's a per-component contract.

## `docs/work/<work>/` — the flowing execution state

A work-stream turns component `plan`s into executable items; **work spans components by nature**
(one feature touches many), which is why it's separate from any single component's `plan`. Files
(create on demand — an empty one isn't required; a one-line change needs no work-folder; full
ceremony is for substantial, multi-phase work):

- **`todo.md`** — planned, not started. Written from the component plan/design/intent/current
  when we take on a work-stream.
- **`remaining.md`** — actively executing now. Items move here from `todo` when we begin.
- **`completed.md`** — done + verified. Items move here from `remaining`. Append-only history;
  authoritative for what's done.
- **`issues.md`** — problems hit + candidate solutions + which we chose + why.
- **`forks.md`** — decision points + options + which we chose + why.
- **`blockers.md`** — things needing human input: description, solution analysis, *why* it blocks
  (needs a human), suggested path. Open→resolved; resolved rows archive with a date. The point:
  surface *what* blocks me and *why* so you can supply what I need — the goal is fewer blockers
  over time, because a well-understood one becomes an `issue` or `fork` I resolve myself.

A work-folder headers the component(s) + plan phase it executes; plans link back to active work.

## Row conventions

- **Move items between state-files — never annotate one file** with strikethrough / "DONE".
  (That partially-crossed-out mess is exactly what these separate files replace.)
- **Timestamp every row** with the date you **write or modify** it (`YYYY-MM-DD`) — the
  *doc-modification* time, not the git date. This is what lets us trace the line of logic later
  ("this landed three days ago, that two days ago → here's what happened and what's next"), and
  it's how we catch code carrying an older design against a newer methodology. The commit is
  recoverable from the date; the date is the anchor. Datetime only if same-day order matters.
- **Order:** newest-first for at-a-glance files (`remaining`, `todo`, `blockers`); chronological
  append for history (`completed`, `issues`, `forks`).
- **The lifecycle:** on starting work → write `todo`; on executing → move to `remaining`; on
  finishing → move to `completed`. Issues / forks / blockers captured as they arise.

## Where a thing goes (quick reference)

| You have… | It goes in… |
|---|---|
| the final shape of a component (tables/types/protocol) | `components/<c>/design/` |
| why the shape is that way, what's stored/read where + entry/exit | `components/<c>/intent/` |
| what's implemented vs not, right now | `components/<c>/current/` |
| phased path from current → design | `components/<c>/plan/` |
| a feature's intent, components unknown/unscoped | `docs/intent/<feature>/` |
| the same feature's intent once scoped to components | each `components/<c>/intent/` |
| an item to do / doing / done | `work/<w>/{todo,remaining,completed}.md` |
| a problem + how we resolved it / a decision between options | `work/<w>/{issues,forks}.md` |
| something needing your input | `work/<w>/blockers.md` |
| a cross-session hint for me (not project state) | `memory/` |
