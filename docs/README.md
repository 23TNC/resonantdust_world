# docs/ — the project's external memory

Where we externalize state so neither of us has to hold it in our head, and so hours of design
survive session limits, deviation, and re-planning. **Read this file first each session — then the
`design/` of whatever you're about to touch, _not_ the ticket.**

## Precedence (when two docs disagree)

`design/` → `intent/` → `work/completed.md` → `current/` / `plan/`. `design/` wins outright. The
three cross-component **authorities** below outrank component docs *and* the code — when the code
disagrees, **the code is the bug**.

## The authorities — shapes only (outrank everything, including code)

- [`VARIABLES.md`](VARIABLES.md) — every cross-component variable's name / width / bit layout.
- [`TABLES.md`](TABLES.md) — every cross-component table's columns / keys / readers / writers.
- [`ACTIONS.md`](ACTIONS.md) — the event program: actions, arity, encoding (append-only; arity is wire).
- The *why* behind the shapes lives in [`notes/`](notes/). **Never restate a layout** outside its
  authority file — a component doc that copies a layout is drift waiting to happen.

## The trees

- [`CONVENTIONS.md`](CONVENTIONS.md) — **how** we document. Read once; it governs everything here.
- [`components/README.md`](components/README.md) — the component **map**: every deployable/shared
  chunk, where it lives, alive vs dead. Each earns a `{design,intent,current,plan}` folder lazily.
- [`work/README.md`](work/README.md) — the **work index**: every work-stream and its flowing state
  (`todo`/`completed`/`issues`/`forks`/`deviations`/`blockers`).
- [`intent/README.md`](intent/README.md) — cross-cutting feature intent not yet scoped to a component.
- [`repo-layout.md`](repo-layout.md) — the physical on-disk tree + the old→new rename key.

## Kept honest by

`bin/rd docs-check` — the audit that enforces this system's invariants (no decay markers in
authoritative files, reference + link integrity, freshness stamps, index linkage, map↔folder
bijection). Wired to a **Stop hook + git pre-commit** so drift fails loudly instead of accumulating.
Built + explained in [`work/docs-authority/`](work/docs-authority/README.md).

## Where a thing goes

The quick-reference table is in
[`CONVENTIONS.md`](CONVENTIONS.md#where-a-thing-goes-quick-reference).
