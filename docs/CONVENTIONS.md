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

- **Groups** (mirrors the repo): `client/`, `server/`, `server/spacetime/modules`,
  `shared/`, and `dev/` (developer + gameplay: the scripts + specs + assets that are the
  foundation of how we build — `rd`, `art`, `content`, the TOML corpus specs, sprite templates).
- **Not components:** an **individual table** (the *module* that holds it is the component); a
  **build output** (`shared/pkg`, `shared/target`); a **piece of a component** (worldgen ⊂ edge).
- **Cross-cutting concepts** are documented where they live, not minted as fake deployables — and
  when they span *every* component, they get a top-level file: `VARIABLES.md` (variables),
  `TABLES.md` (tables), `ACTIONS.md` (the event program).

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

  Every `current/` file carries a **freshness stamp** — `Last updated: YYYY-MM-DD` or
  `verified @ <git-sha>` (the commit it was actually checked against). `rd docs-check` **requires**
  the stamp, and (warning-only) compares it against the component's code path: if the code changed
  *after* the stamp, it flags the cache as possibly stale. That comparison is the mechanical guard
  against the phantom-project trap below — a lagging cache now announces its own age.

  ⚠️ **A cache that may lag is safe to *read* and unsafe to *plan from*.** Learned the hard way on
  2026-07-14: the shard's `divergences.md` had three rows (#1, #6, #7) describing code that had been
  deleted months earlier — `completed.md` said so the whole time — and a whole phase sequence got
  planned against them. Two phases of "work" were already done before they were scheduled. So:

  - **Verify a row before you work it.** `grep` the identifiers it names. If they aren't in the
    code, the row is stale: **close it, don't work it.** This costs a minute and it is not optional
    — `divergences.md` invites "point at a row and say close it", which is precisely how a stale row
    becomes a phantom project.
  - **Close the divergence in the same commit that closes the code.** This is the only rule that
    actually keeps the lag from accumulating. A row retired a week later is a row that never gets
    retired.
  - **Precedence when docs conflict**: `design/` → `intent/` → `completed.md` → `current/` / `plan/`.
    `design/` wins outright: on the same day, `divergences.md` #1's *fix* line told us to build the
    event VM on `shared/dsl` while `design/event-dsl.md` explicitly decided the opposite
    ("purpose-built… don't design around it"). Following the ticket would have violated the design.
    **Re-read `design/` at the start of a task, not the ticket.**
- **`plan/`** — **how we close current → design**. Proposed phases, how they tie into other
  components' plans, what blocks forward motion. **`plan` (and `design`/`intent`) hold the FULL
  future intent, not just what current work needs.** When executing, build *toward* the
  documented design/intent — never quietly simplify it away for the task in hand. (We keep a
  complex LOD system with three sprites because the game will have many; the plan is where that
  future intent survives session limits, deviations, and re-planning.) If the plan genuinely
  needs to change, change *the plan doc* with input — don't circumvent it in the code.

## `docs/intent/<feature>/` — cross-cutting feature intent (staging)

Some features (e.g. **the shard rebuild**) touch many components — implementing them threads through
several pieces. Their intent belongs in the **`intent/` of each component they touch**. But when
we don't yet know which components a feature touches, or don't have enough information to ascribe
it, we **park it here** — a top-level staging area for feature-level intent. As the feature gets
scoped, its intent **distributes out** into the affected components' `intent/` folders (leaving a
pointer here, or retiring the staging entry). This is where a half-formed "we want X to…" lives
before it's a per-component contract.

## `docs/work/<work>/` — the flowing execution state

A work-stream turns component `plan`s into executable items; **work spans components by nature**
(one feature touches many), which is why it's separate from any single component's `plan`.

**Folder name — date-prefix new streams** (`YYYY-MM-DD-<slug>`, the date opened; from 2026-07-21).
So folders sort chronologically and a glance dates the work. Pre-existing un-prefixed folders keep
their names (rename lazily if touched). Files
(create on demand — an empty one isn't required; a one-line change needs no work-folder; full
ceremony is for substantial, multi-phase work):

- **`todo.md`** — **the plan**, for the life of the stream. Written from the component
  plan/design/intent/current when we take on a work-stream.

  **The item contract.** An **item** is a checkbox line — `- [ ]` open, `- [x]` done. That is the
  *only* thing that counts as an item: a plain bullet is detail under an item, and prose is prose.
  `rd docs-check` enforces it (a plan file with bullets but no checkbox is an ERROR, because its work
  is invisible to the continuation hook — which is how that hook once sat dead for six days). Write
  each item as **one action plus its acceptance criterion**, under ~250 chars; `docs-check` warns
  past that, because an item too coarse to execute forces a resuming session to re-plan, and planning
  invites ratification, which is a stop. Use [`/rd-plan`](../.claude/skills/rd-plan/SKILL.md) to
  decompose one.

  **Items never move — `[x]` IS the move.** Tick the box in place; don't cut the line out. (Don't
  strike it through either: `~~` is banned tree-wide, and a ticked box says the same thing where a
  machine can read it.) The stream's full plan stays legible in one file, and `completed.md` stops
  being a second copy of the list.
- **`completed.md`** — the **verification log**: dated entries saying what landed and *how it was
  checked*. Append-only; authoritative for what's done and why we believe it. It records evidence,
  not item text — the items live in `todo.md` with their boxes ticked.
- **`remaining.md`** *(optional)* — an in-flight tier for a **long** stream where several items run
  at once and knowing *which* is mid-execution matters. Most streams skip it — the box state already
  says planned-vs-done. Use it only when the extra tracking earns its keep; don't create it empty.
- **`issues.md`** — problems hit + candidate solutions + which we chose + why.
- **`forks.md`** — decision points + options + which we chose + why.
- **`deviations.md`** — **where the code departs from the plan** (`components/<c>/{design,intent}`).
  Log a row **at the moment you deviate**, not when someone catches it. The plan encodes decisions
  we spent real thought on, so a departure is how bugs get in — it must carry a **strong** reason.
  *"Less churn", "the existing code already did X", and "it's only cosmetic" are **not** reasons* —
  they're the absence of one. If the plan looks wrong, change **the plan** (with input); don't
  quietly diverge in code. Also log a deviation you *find* (someone else's, or your own, later).
  Rows: date · what the plan says · what the code does · why · fix/status. Open→resolved.
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
- **Order:** newest-first for at-a-glance files (`todo`, `blockers`, `remaining` if used);
  chronological append for history (`completed`, `issues`, `forks`).
- **The lifecycle:** on planning work → write `todo`; on finishing an item → move it to `completed`
  (default `todo → completed`). On a long stream, optionally park in-flight items in `remaining`
  between the two. Issues / forks / blockers captured as they arise.

## Where a thing goes (quick reference)

| You have… | It goes in… |
|---|---|
| **a cross-component variable's name / width / bit layout** | **`docs/VARIABLES.md` — authoritative; never re-state a layout in a component doc as if it owned it** |
| **a cross-component table's columns / keys / readers / writers** | **`docs/TABLES.md` — authoritative; module-internal tables stay in the module** |
| **an action, its arity, or the program encoding** | **`docs/ACTIONS.md` — authoritative; append-only, arity is wire** |
| the final shape of a component (tables/types/protocol) | `components/<c>/design/` |
| why the shape is that way, what's stored/read where + entry/exit | `components/<c>/intent/` |
| what's implemented vs not, right now | `components/<c>/current/` |
| phased path from current → design | `components/<c>/plan/` |
| a feature's intent, components unknown/unscoped | `docs/intent/<feature>/` |
| the same feature's intent once scoped to components | each `components/<c>/intent/` |
| an item to do / done | `work/<w>/todo.md` as `- [ ]` / `- [x]` — items never move, the box is the move |
| evidence that an item is really done | `work/<w>/completed.md` (dated: what landed + how verified) |
| a problem + how we resolved it / a decision between options | `work/<w>/{issues,forks}.md` |
| something needing your input | `work/<w>/blockers.md` |
| code that departs from the plan (yours or found) | `work/<w>/deviations.md` |
| a cross-session hint for me (not project state) | `memory/` |
| a superseded / pre-rewrite design, or a completed/reverted work stream | **move it out of the repo to `../archive/`** (git history keeps it; an in-repo `archive/` gets read as current + confuses). Do **not** keep it in-repo. |
