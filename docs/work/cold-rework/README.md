# Work — cold-rework (subtype fix + overlay + region router → the `*_tables!` generalization)

> **Current direction (2026-07-18): the `*_tables!` generalization.** Phases P1–P4 below (subtype fix,
> region router, `state`/`state_log` overlay, event-driven `SET`) are **built + live**. The design then
> shifted (see [`forks.md`](forks.md) **F4→F5→F6**): every shard folds into generic macros —
> `entity_tables!`/`dense_entity_tables!`/`sparse_entity_tables!` (→ `entity_state`/`entity_state_log`)
> + `overlay_tables!` (→ `overlay`/`overlay_log`); **all writes go through events** (retiring the direct
> `seed`/`set_*`/`fold`); and **`PROMOTE`** is a smart, atomic prefix. This closes the seed/acquire race
> by construction and unifies cold with hot. **Implementation plan:
> [`work/shard-tables`](../shard-tables/README.md).** Authoritative shapes:
> [`TABLES.md`](../../TABLES.md) · [`VARIABLES.md`](../../VARIABLES.md) · [`ACTIONS.md`](../../ACTIONS.md)
> · [`world-storage`](../../intent/world-storage/README.md). **Not yet built.**

_Opened 2026-07-17. Original scope: restore the biome `subtype` the baseline dropped, then give cold the
same `state_log`/`state` machinery hot uses so a cold cell mutates through the pipeline — since
generalized (above)._

## What & why

The cold baseline is live but on the **wrong shape**: when `cold_row_reference` was compressed to
`u32`, `type_reference:16` was swapped for `layer_reference:8`, silently dropping `subtype_id:12` —
**the biome**. Content documents biome-as-subtype ([`content/biome/biomes.rd`](../../../content/biome/biomes.rd)),
and the DSL hands worldgen the biome, but `zone_cold` discards it. Nothing renders wrong only because
the client reads `kind_id` alone — so "things change appearance with biome" is currently impossible.

The fix restores `subtype` **inside `u32`**: `type_id` is the shard (the `tile` module *is*
`TYPE_BIOME_TILE`), so drop it from the row → `macro:16 | subtype:12 | layer_id:4`. Then cold gains a
`state`/`state_log` overlay in the **exact hot format**, so a mutation mints a `state_log` row (never
rewriting the big cold row) and rides the live worker/promote machinery; GC folds it back into the
baseline. Routing moves to `index.cold_shards`, keyed by **region**.

## The invariant (done when)

**A cold object carries its full `definition_reference` (biome included) and `position_reference`,
reconstructed from `shard.type_id` + row + entry; a cold cell mutates only via a `state_log` row the
pipeline composes; the client renders baseline ⊕ `state`; and a position resolves to its cold shard
through `index`.**

## How this runs — four phases

- **P1 · subtype fix** — the schema defect. `cold_row_reference` → `macro:16 | subtype:12 | layer_id:4`;
  worldgen groups a zone by `biome_subtype_id` (one row per biome); seed/wire/client reconstruct
  `type_id` from the shard. Self-contained, **browser-verified** (biomes still flow; a tree now knows
  its biome). Read-only cold.
- **P2 · region router** — `index.cold_shards` (`(type, region) → shard`); the edge resolves cold subs
  through it. One default row now; the indirection is the deliverable.
- **P3 · overlay read-path** — add `state`/`state_log` (data_shard shape) to the cold modules; the edge
  subscribes + relays cold `state`; the client composites **baseline ⊕ state** (a `state` row at a
  position overrides that cell). Stands up the *read* side before anything writes it.
- **P4 · mutation** — the large build: mint (`UNPACK`) an `entity_reference` for a touched cell, compose
  its `state_log` via events on a worker, `promote_state`, and GC-fold `state_log` → baseline. Needs the
  blockers resolved (composition reuse, minting/routing). **Land P1–P3 first** (they fix the live defect
  and stand up the overlay); P4 is the follow-on.

## Left alone — recorded so it isn't re-litigated

- **`TYPE_THING` items** (biome-invariant) are out of scope — this rework is `biome-tile`/`biome-thing`.
  Where items live (their own cold module? hot?) is a later decision ([[object-model-redesign]]).
- **The `pawn` shard** rename of `data_shard` is not this work; cold reuses whatever composition
  `data_shard` exposes.
- **Realm stays 0**; the router is region-keyed within realm 0.

## Files

`todo` → `completed`. `forks` holds the resolved decisions; `blockers` is empty (all resolved).
`issues`/`deviations`/`remaining` appear only if they gain content.
