# Forks — cold-rework

_Decision points, options, what we chose, why. Chronological._

---

## F1 · Composition machinery: **shared macro** (was BK1)

**Decision.** Factor `data_shard`'s bespoke composition into a shared `tick_pipeline!` macro that
`data_shard`, `tile`, and `thing` all invoke — not duplicate it three times.

**What moves:** the `clock` / `state_log` / `state` tables and `init` / `bump` / `claim` / `write` /
`gc` reducers (everything but the payload). `data_shard` becomes the payload struct (`TargetState` =
`definition_reference | position_reference | data`) + one macro invocation (~242 → ~15 lines). Cold's
payload is the **same** three-ref shape, so `tile`/`thing` invoke the same macro and add only their
baseline + seed + mint + fold.

**Why.** One source of truth for the composition (subscription SQL is string-typed — three drifting
copies is a live-bug hazard). It's the pipeline-generalization direction already set.

**Care.** This re-touches the **live** `data_shard`. The macro must emit byte-identical table shapes
and reducer signatures. **P3 step 1** = extract the macro and prove `data_shard` is unchanged (bindings
diff clean + the wolf still moves) *before* the cold modules build on it. Macro crate location settled
at build (a lightweight `#[macro_export]` crate the modules path-dep; it emits `spacetimedb::` tokens,
so it needn't depend on spacetimedb itself).

---

## F2 · `server_reference` is master-assigned; the edge routes via `index` (was BK2)

**Decision.** (a) A minting shard's `server_reference` is **master-assigned at standup** (a `server`
table set like `set_orchestrator`), not a compile-time const — the mint reads it. (b) The edge locates
shards through `index.cold_shards` for **both** the baseline subscription and the `state` routing — one
region-keyed router.

**Why.** Consistency with the rest of the standup (orchestrator is already master-assigned); the mint
needs a unique `server_reference` per shard and the master owns that allocation. One router avoids two
mechanisms.

**Consequence.** `event_shard`'s hardcoded `const SERVER_REFERENCE` becomes the outlier — a small
follow-up migrates it to the master-assigned pattern (not in this work's critical path). Only *minting*
shards carry the `server` table (`data_shard` doesn't mint), so it's a mint-side addition, not part of
the composition macro (F1).

---

## F4 · Cold gets its own `cold_log`/`cold` pair; every cold write is an event (2026-07-18)

**Decision.** Give the cold **baseline** the same server-side-`*_log` / client-visible-`*` split every
other subject already has: **`cold_log`** (composed truth + history, keyed by `cold_uid =
cold_row_reference:32 | tic:16`) and **`cold`** (the promoted projection, renamed from
`cold_tile`/`cold_thing`). **Retire the direct `seed`/`set_tile`/`set_thing`/`fold` reducers** — every
change to `cold`/`cold_log`/`state`/`state_log` flows through **events**, composed by a worker. Add two
actions: **`PROMOTE_COLD`** (`cold_log → cold`, the baseline mirror of `PROMOTE_STATE`) and **`PACK`**
(fold settled `state_log` → `cold_log`, GC-queued). A fold emits `PACK`(s) → `PROMOTE_COLD` →
`PROMOTE_STATE`, in that order.

**Why.** The direct writes were the root of the **seed/acquire race** (F-live): a `cold` row written
directly, its `tic` never bumping, so a relay the edge's `on_insert` missed never re-fired → black
ground under rendered scatter (the `on_applied` snapshot relay is a band-aid, not a cure). Routing cold
through the **same worker machinery as everything else** removes the race by construction — a row is
invisible until `PROMOTE_COLD`, acquired like any `*` update — and kills the bespoke cold path that was
"all over the place." It also makes a **multi-tic** zone write atomic to the client (no half-built
baseline) and gives cold a **history** (`cold_log`) to operate on.

**Two overlay tiers stay.** `state`/`state_log` (per-cell, cheap, frequent) is kept *alongside*
`cold`/`cold_log` (whole-row, infrequent) — a wall placed every 3 tics must be a one-cell `state`
override, never a 256-tile `cold` re-broadcast. GC `PACK`s the overrides down into `cold_log` on a
cadence.

**Order is load-bearing.** `PROMOTE_COLD` **before** `PROMOTE_STATE` in a fold: promote the folded
baseline first (client already reads the right pixels, since `cold_new ⊕ state == cold_new`), *then*
clear the redundant overrides — the reverse flashes the pre-edit baseline.

**Open at build (flagged in the design, not blockers):** (a) `cold_log`'s `Vec` payload doesn't fit the
fixed-3-ref `tick_pipeline!` macro — generalize the macro over the payload, or hand-roll `cold_log`
with the shared composition columns; (b) the **source of a seed/regen row's content** in the event
model — a worker-side worldgen call vs an event-carried `Vec` payload; (c) `PACK`/`PROMOTE_COLD` action
values + operand signatures. Supersedes the old P4 "event-driven `UNPACK` via a direct-reducer mint"
framing — the `SET`-to-`state` overlay path (built, [`deviations.md`](deviations.md) D-1) stays; the
*baseline* now rides `cold_log` too.

---

## F6 · `PROMOTE` is a prefix modifier + smart/atomic; tables are `entity_state`/`overlay` (2026-07-18)

**Decision (promote).** Drop the per-table promote actions (`PROMOTE_STATE`/`PROMOTE_COLD`/…). `PROMOTE`
(arity 0) is a **prefix**: executing left-to-right, it sets a promote bit in scratch that the **next**
action consumes, writing the promote flag as part of *its own* result (on both targets). So `promote
place obj dest`, `promote init_zone macro`. The write reducer, given the bit, **smart-promotes** — copies
`entity_state` (from `entity_state_log`) **and** `overlay` (from `overlay_log`) where `visible.tic !=
log.tic`, so it syncs exactly what the action changed, in **one transaction**.

**Why prefix, not postfix.** `add a b promote` would form the result, then have to reverse-engineer what
`promote` targeted; `promote add a b` lets the worker write the bit *during* the add — no post-pass, no
figuring out which shards to call promotes on.

**Consequence — atomicity replaces ordering.** A fold's `promote pack …` projects the folded baseline +
cleared overlay in the same commit, so the client sees them atomically → **no flash, no
`PROMOTE_COLD`-before-`PROMOTE_STATE` rule** (F4/F5's ordering is moot). `PROMOTE_EVENT` is **tabled**
until movement.

**Decision (naming).** The primary pair is **`entity_state`/`entity_state_log`** (not `data`/`cold`) and
the override pair **`overlay`/`overlay_log`**; macros are **`entity_tables!` / `dense_entity_tables!` /
`sparse_entity_tables!` / `overlay_tables!`**. `data_shard = entity_tables!{data:u8}`; `tile =
dense_entity_tables!() + overlay_tables!()`; `thing = sparse_entity_tables!{data:u8} +
overlay_tables!{data:u8}`. Using `entity_state` for the table frees **`data`** to remain a payload field
(no table/field overload). Supersedes F5's `data`/`cold`/`state` table names.

---

## F5 · Generalize the composition tables into three payload-generic macros (2026-07-18)

**Decision.** Extend F4's `*_log`/`*` split into **three reusable macros**, each generic over an
optional payload `<T>`, so actions + the worker machinery are written **once** against them:

Each `*(name, <T>)` emits a `<name>`/`<name>_log` pair; `<T>` is monomorphized at **macro-time**;
`kind_reference` is **always** present (fixed), the payload is the extra.

- **`state!`** — `entity_reference`-addressed. `state` = `entity_reference, macro_position_reference,
  micro_position_reference, tic, definition_reference, <T> payload` (position split into macro-sub-key +
  micro-cell; old flat `data` → generic payload). `*_log` = `state_uid` + `worker_reference,
  observer_reference, status`. **Hot shards only.** (`counterpart_reference` — deferred, it was for
  cross-shard transfers, unneeded while cold stays in-shard.)
- **`dense!`** — `cold_row_reference`-addressed, `Vec<DenseItem{kind_reference:u16, payload:T}>`,
  **full `ZONE_DIM²`** → `tile_reference` **indexes**.
- **`sparse!`** — same header, `Vec<SparseItem{tile_reference:u8, kind_reference:u16, payload:T}>`,
  **occupied cells only** → `tile_reference` **matches**. `*_log` = `cold_uid` + composition columns.

**Resolution — the overlay is always `sparse!`, not the hot `state!`.** An override touches only a few
cells, so a cold shard's client-visible `state` overlay is a *second* `sparse!` shadowing the baseline.
`state!` is only for hot shards (a mover isn't cell-pinned). **Shard-composer** macros wrap a baseline +
overlay pair, named `cold`/`state`:

- **`dense_shard!(T)`** = `dense!(cold, T)` + `sparse!(state, T)` — e.g. `tile` (`T` = none).
- **`sparse_shard!(T)`** = `sparse!(cold, T)` + `sparse!(state, T)` — e.g. `thing` (`T` = `u8 data`).
- hot uses `state!(state, u8)` directly (`state_shard!` optional sugar).

**Why.** One machinery, not hand-rolled shard modules; the minimum to stand up the tables, generic
across every shard. The overlay-is-`sparse!` insight **drops** the `cold_entity_reference` deterministic
id and the per-cell `tic` composite (a cold cell is addressed by `tile_reference` within its biome-row;
"overlay present ⇒ override wins"). "dense vs sparse" (index vs match) is the **only** place the forms
differ, behind the macro. Supersedes F4's single `cold_log`/`cold` pair framing.

**Open at build:** (a) validate a `spacetimedb`-`#[table]` payload-generic macro pattern (`<T>` = `()` /
`u8`); (b) seed/regen row content source — worker-side worldgen vs event-carried `Vec`; (c)
`PROMOTE_COLD`/`PACK` action values + `PACK` signature. Naming: `cold_row_reference`/`cold_uid` keep
"cold" for now (later cleanup).

---

## F3 · No `route_reference` wildcard — explicit region assignment (was BK3)

**Decision.** `cold_shards` holds **explicit `(type_id, region_reference) → shard` rows**; the master
assigns them as regions come online. No wildcard / sentinel region.

**Why.** The wildcard only existed to avoid seeding a row per region for the single-shard default — but
the world uses only a handful of regions today (around the origin), so we just seed those. An unassigned
region has no row until the master allocates it. Simpler and truthful; `route_reference` needs no
special encoding.
