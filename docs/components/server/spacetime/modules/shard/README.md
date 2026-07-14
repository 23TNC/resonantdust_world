# Component — `shard` (SpacetimeDB module)

_Path: `server/spacetime/server/modules/shard`. Deploys as: the game's live+cold data module.
Last updated: 2026-07-14._

The unified tick pipeline: `event_log` / `state` / `state_log` / `holder` / `cold` / `tic_meta` /
… with the two-phase recoverable lifecycle. Its `cold` table (the object model) supersedes the
retired `cold_tiles` / `cold_things` modules. Built on the shared
[`pipeline`](../../pipeline/) macro; addresses entities via the shared
[`codec`](../../../../shared/) refs; runs the event-DSL interpreter from
[`shared/tick`](../../../../shared/).

Docs follow [`docs/CONVENTIONS.md`](../../../../../CONVENTIONS.md):

- **[`design/`](design/)** — the *shape* it should be: `tables.md`, `event-dsl.md`, the design
  `README`. (The entity/zone/object **reference layouts** live in
  [`docs/references/`](../../../../../references/) — they're really `shared/codec`'s design; the
  shard consumes them. To relocate when codec is migrated.)
- **[`intent/`](intent/)** — the *why / how used*: `events.md`, `lifecycle.md`, `hot-cold.md`,
  `risks.md`, `event-reference.md`.
- **[`current/`](current/)** — where we are: `README.md` (implemented vs not) + `divergences.md`
  (the gap list). Authoritative done-history is
  [`work/spacetime-rewrite/completed.md`](../../../../../work/spacetime-rewrite/completed.md).
- **[`plan/`](plan/)** — how we close current → design: the staged plan (`stages/`, S0–S7, mostly
  done) + `cleanup.md` (retire the dead modules) + forward work.

Active work-stream: [`docs/work/spacetime-rewrite/`](../../../../../work/spacetime-rewrite/).
