# Work — shard-tables (the `*_tables!` generalization)

_Opened 2026-07-18. Builds the [`world-storage`](../../intent/world-storage/README.md) **direction
shift** (decided in [`cold-rework/forks.md`](../cold-rework/forks.md) F4→F5→F6): fold every composing
shard into a small family of generic table macros, drive **all** writes through events, and make
`PROMOTE` a smart, atomic prefix. Authoritative shapes: [`TABLES.md`](../../TABLES.md) ·
[`VARIABLES.md`](../../VARIABLES.md) · actions [`ACTIONS.md`](../../ACTIONS.md)._

## What & why

`cold-rework` shipped the cold overlay + `tic` composite on **hand-rolled** shard modules, and it
surfaced a real bug: a directly-written baseline row whose `tic` never bumps can be **missed by the
edge relay and never re-acquired** — black ground under rendered scatter (the `on_applied` relay is a
band-aid). The cure is structural: give every shard the **same** composition machinery the hot pipeline
already trusts, and let a row become visible **only through a promoted event** — no direct writes, no
special cold path, no acquire race.

So: three building-block macros (each a `*_log` truth/history + `*` client projection pair, generic
over an optional payload `<T>`), composed per shard, plus the smart `PROMOTE` prefix.

| macro | pair | form |
|---|---|---|
| `entity_tables!(T)` | `entity_state`/`entity_state_log` | one entity per row (hot movers) |
| `dense_entity_tables!(T)` | ″ | `Vec<DenseItem>`, full `ZONE_DIM²` — `tile_reference` indexes |
| `sparse_entity_tables!(T)` | ″ | `Vec<SparseItem>`, occupied cells — `tile_reference` matches |
| `overlay_tables!(T)` | `overlay`/`overlay_log` | the sparse override tier (cold shards) |

`data_shard = entity_tables!{data:u8}` · `tile = dense_entity_tables!() + overlay_tables!()` ·
`thing = sparse_entity_tables!{data:u8} + overlay_tables!{data:u8}`.

## The invariant (done when)

**Every shard's tables are stamped from the `*_tables!` macros; every write to any of them is an event
a worker composes and a `PROMOTE` prefix projects (no direct `seed`/`set_*`/`fold`); a cold cell is
addressed by `cold_row_reference` + `tile_reference` (no per-cell entity id); the client renders
`entity_state ⊕ overlay` with no acquire race — a browser walk of a fresh world shows fully-covered
terrain, and a scripted override renders, `PACK`s, and promotes atomically with no flash.**

## How this runs — phases

- **P0 · Spike: the payload-generic `#[table]` macro.** Prove a `macro_rules!`/proc-macro can stamp a
  SpacetimeDB `#[table]` parameterized by an optional payload — `entity_tables!{data:u8}` **and**
  `entity_tables!()` both compile + deploy a valid module. **Gates everything** (F5/F6 open item a). If
  `#[table]` can't take a macro-supplied payload cleanly, fall back to a fixed payload enum or per-shard
  hand-roll — decide here.
- **P1 · The macros in `shared/codec`.** Generalize `tick_pipeline!` into `entity_tables!` /
  `dense_entity_tables!` / `sparse_entity_tables!` / `overlay_tables!` (composition columns + `write` /
  `claim` / `gc` reducers + the `DenseItem`/`SparseItem` payloads). **Prove behavior-preserving on
  `data_shard`** — byte-identical bindings (as the `tick_pipeline!` extraction was) + the wolf still
  moves — before any shard renames.
- **P2 · The `PROMOTE` prefix.** `PROMOTE` (arity 0) in codec; the worker's `apply` consumes it and
  marks the next action's targets; the macro's `write` reducer promotes `entity_state` **and** `overlay`
  where `visible.tic != log.tic`, in **one transaction**. Verify a hot promote still lands a mover.
- **P3 · Convert the shards + rename tables.** `data_shard`→`entity_tables!{data:u8}` (`state`→
  `entity_state`); `tile`/`thing` → the dense/sparse baseline (`cold_tile`/`cold_thing`→`entity_state`)
  **+** `overlay_tables!`. Regenerate bindings; update the edge relay + client render to
  `entity_state ⊕ overlay` (drop the `cold_entity_reference` + per-cell `tic` composite). Reset + reseed
  is fine (dev).
- **P4 · Events replace direct writes.** `init_zone` action (worker builds the whole baseline row,
  writes `entity_state_log`, `promote` projects it); retire `seed`/`set_tile`/`set_thing`/`fold`. A
  per-cell override → `overlay_log` (`SET`, reconciled to the row+`tile_reference` addressing). `PACK`
  action (fold settled `overlay` → `entity_state_log`), GC-queued `promote pack …`.
- **P5 · Seed content source.** Resolve F5/F6 open item b — worker-side worldgen vs an event carrying
  the `Vec` — and implement it for `init_zone`. (Leans worker-side: keeps the event small + replay
  deterministic from worldgen.)
- **P6 · Verify + retire the band-aid.** Browser: a fresh world renders fully-covered terrain **with no
  acquire race** (the whole point) even under fast panning; a scripted override renders → `PACK` →
  atomic promote with no flash. Remove the `on_applied` relay band-aid once the event path is proven.

## Follow-on — the *next* streams (deferred until the tables land; some **enabled by** them)

These are the concerns to address **after** this rework nails the tables down (owner's list, 2026-07-18)
— recorded here so they aren't lost; each becomes its own stream when this one completes:

- **Client's interpretation of the data** — deferred *on purpose*; needs the table shapes locked first.
- **Tiles not loading in** — the acquire race; **may be fixed by this rework alone** (P6 verifies — a row
  is invisible until promoted, acquired like any `*` update).
- **~10,000 draw calls** — a rendering-perf problem in the client; find the cause + resolve.
- **Move wolves to a `pawn` table** — off `data_shard`; **enabled** — `entity_tables!{…}` makes a `pawn`
  shard a one-line stamp.
- **Hot *and* cold thing shards** — a thing can be hot (a dropped/movable item) or cold (biome scatter);
  **enabled** — the macros make a hot thing shard trivial.

## Left alone — recorded

- **Naming `cold_row_reference` / `cold_uid`** keep "cold" for now — a later mechanical cleanup (F6).
- **`PROMOTE_EVENT` + movement** are a separate pass (tabled — see [`ACTIONS.md` §Movement](../../ACTIONS.md)).
- **Cross-shard transfers** (`counterpart_reference`) deferred — cold mutation stays in-shard.
- **Multi-region → multi-shard connection pool** (cold-rework's deferred P2) is unchanged by this.

## Files

`README` (this) → `todo` → `completed`. `forks`/`issues`/`blockers`/`deviations`/`remaining` appear
only as they gain content. The design decisions that spawned this stream live in
[`cold-rework/forks.md`](../cold-rework/forks.md) F4–F6.
