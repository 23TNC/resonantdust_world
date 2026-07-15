# Divergences — code today vs this design

The honest ledger. Each row is where the implementation does *not* match this folder, with
the file and the fix. Point at a row and say "close it" and it's a well-scoped task.

Source of truth for the *intent* is the rest of this folder; source of truth for the
*current code* is [`server/spacetime/server/pipeline/src/lib.rs`](../../../../../../../server/spacetime/server/pipeline/src/lib.rs)
(the `decl_tick_pipeline!` macro) and [`server/worker/src/main.rs`](../../../../../../../server/worker/src/main.rs).

Legend: 🔴 contradicts the design · 🟡 partial / in-migration · ⚪ naming/cosmetic.

---

## 1. 🔴 `event_log` is wide scalar columns, not `actions : Vec<u64>` DSL

- **Design** ([events.md](../intent/events.md), [event-dsl.md](../design/event-dsl.md)): the event carries
  `actions : Vec<u64>` — a flat postfix (RPN) stream of self-qualified words
  (`server_reference:16 | reserved:16 | ref:32`), operands (push) + verbs (pop+run); the
  worker interprets it. No `action` enum, no positional slot schema, no actor field, no
  operand table.
- **Code**: a fat struct with `actor_key:u64`, `target_key:u64`, `data0/1:u64`, and named
  `source/actor/requesting/worker/trigger _server_reference` columns
  ([pipeline/src/lib.rs](../../../../../../../server/spacetime/server/pipeline/src/lib.rs) `EventLog`).
- **Fix**: replace the scalar target/actor/data columns with `actions:Vec<u64>`; keep
  `event_reference`, `tic`, `worker_reference`, `status`. Stand up the RPN interpreter in the
  worker (reuse `shared/dsl`'s value-stack VM behind a `Vec<u64>` word decoder).
- **Blast radius**: the worker's `resolve_one` (currently reads `actor_key`/`target_key`) →
  becomes the interpreter; `append_event` signature; edge/npc callers; a compile step
  (surface DSL / plan → `Vec<u64>`).

## 2. ✅ CLOSED (2026-07-14) — mint is enqueue-absorbed; `PACK` is an enqueued execute op

- **Design** ([hot-cold.md](../intent/hot-cold.md), [lifecycle.md](../intent/lifecycle.md)): cold→hot of a *target*
  is **absorbed into the enqueue phase** (`find-or-mint` by location when standing up a
  `cold_reference` target — no `MINT`/`GET` verb); `PACK` (settle hot→cold) is an execute op.
- **✅ Mint.** `find-or-mint` runs in the worker's **enqueue** phase (`stand_up` time), by location,
  idempotently; there is no `MINT`/`GET` verb and the edge only names the cold target by its
  `cold_reference`. Verified live (Interact → promote).
- **✅ PACK.** Now a real **execute op**: a periodic `worker::enqueue_pack_sweep` (the trigger owner
  hot-cold.md left open) **appends a `[OBJECT(zone), ACTION(PACK)]` event** against each zone
  holding at-rest packable objects (one in flight per zone); the worker recognises the program at
  **execute**, settles the zone, and completes the row through the normal fenced `resolve`. It
  targets the **zone/cold row** — an object-targeted PACK would hold the very object it means to
  pack. Refcount-gated + atomic inside `pack_settle`; worker-owned, not GC.
- **Verified:** `find-or-mint` → `enqueued PACK zone=0` → `hot → cold (PACK settle)` →
  `PACK resolved ev=… packed=1`, with the object restored to cold verbatim (provenance stashed in
  `data0` at mint) and no enqueue churn.
- **The bespoke side-door reducers are gone from the call path** — this was the specific "you wrote
  cold tables anyway" grievance.

## 3. 🟡 Standalone `cold_things` / `cold_tiles` modules still exist

- **Design** ([tables.md](../design/tables.md)): cold is **one `cold` table inside the generic
  module**, beside hot `state`.
- **Code**: the in-macro `Cold` table exists ✅, but the separate `cold_things` and
  `cold_tiles` modules still physically live under
  [`server/spacetime/server/modules/`](../../../../../../../server/spacetime/server/modules). The last commit
  retired their *wire paths*, not the modules.
- **Fix**: delete the two modules once the `cold` table fully carries their data end-to-end
  (edge seeds it, client decodes it).

> **Update 2026-07-14.** #2, #5 and #10 are **closed** (the re-cut landed + is verified —
> [`work/…/completed.md`](../../../../../../work/spacetime-rewrite/completed.md)). #4 is **not**
> blocked (B-2 was retracted — it wrongly treated the legacy `zone_id`/`surface` as a constraint);
> its remaining half is folded into the new **#11**, the cold row's missing header.

## 11. 🔴 The `cold` row omits `macro_position` + `layer_id` — its key can't select a row

**Raised 2026-07-14 (regression I introduced in the re-cut; caught by the user).**

- **Design** ([reference-model.md](../../../../../shared/codec/design/reference-model.md) §Cold row,
  [tables.md](../design/tables.md)): the row header is **`macro_position_reference:16` +
  `type_reference:16` + `layer_id:4`**, and the PK is their composite `cold_row_reference:u64`.
  `cold_removed` is **1:1** on the same key, its tombstones bare `tile_reference:u8`.
- **Code**: `cold_key : u64 = (zone_id:32 << 32) | type_reference:32`, columns `zone_id:u32` +
  `type_reference:u32`, **no `layer_id`**; `cold_removed` keyed per-zone (`zone_key:u32`) with
  `Vec<u16>` tombstones (`x|y|layer|type_id`).
- **How it broke.** v1's `type_reference:u32` was `type_id:4|subtype_id:12|layer:4|reserved:12` —
  **layer lived inside it**, so the old key *did* discriminate layer. The re-cut correctly moved
  `layer` out to `layer_reference` (it's a tile-slot, not a type property) but **never re-homed it
  in the row/key**, and kept `zone_id:u32` in place of `macro_position:u16`. Two of the three header
  fields were dropped.
- **Consequences (one live, one latent):**
  - 🔴 **Live:** `find_or_mint` selects on `(zone_id, x, y)` and **ignores the target's
    `layer_reference`** entirely, so it takes the *first* row with any entry at that tile. The ground
    layer is dense, so **every occupied cell matches ≥2 rows** (a `BIOME_TILE` row *and* a
    `BIOME_THING` row) — which one wins is iteration-order luck. An Interact naming the tree can mint
    the grass beneath it.
  - 🟡 **Latent:** rows differing only by `layer` collide on one `cold_key`; `seed_cold_row` is
    insert-if-absent, so the second is silently dropped. Masked today only because worldgen emits
    layer 0 exclusively.
- **Fix**: re-cut the row to the design header + composite key; make `find_or_mint` (and the edge's
  interact scan) filter `(macro_position, type_id, layer_id)` then match `tile_reference`; worldgen
  emits `layer_id` per row; `cold_removed` → 1:1 same key, `Vec<u8>` tombstones. Shard republish.

## 4. 🟡 Geometry is geographic; the cold row's `macro_position` is **not** done — see #11

`zone_id` is now the **geographic** `realm:8 | region:8 | zone:8 | reserved:8` (G1 — surface
retired), and the cold `entity_reference` is a geographic `cold_reference` (G2). The `cold` table
still keys by `zone_id:u32` rather than the header's `macro_position_reference:u16`. That is **not**
a mere key-width compaction (an earlier note here wrongly said so): the row header is what a reader
reconstructs a `position_reference` from, and it must also carry `layer_id`. Folded into **#11**,
tracked in [todo](../../../../../../work/spacetime-rewrite/todo.md). The original divergence (flat
legacy `zone_id`):

- **Design**: `cold` keyed by `macro_position_reference:u16`; realm implied by the shard.
- **Code**: `Cold.zone_id : u32` flat, used as the routing column; `cold_key` packs
  `(zone_id, type_reference)`.
- **Fix**: move to `region_zone` as the subscription/routing key as the object-model
  geography lands; reconcile against the legacy flat `zone_id`
  ([../references/spatial-references.md](../../../../../shared/codec/design/references/spatial-references.md)).

## 5. ✅ CLOSED (2026-07-14) — Hot identity re-keyed to the reference model's `entity_reference`

- **Design** ([hot-cold.md](../intent/hot-cold.md)): a hot object is a per-server `hot_reference:u32`
  (+ `server_reference` for global uniqueness).
- **Code**: `state`/`state_log` key on `u64 entity_key`; the 32-bit `object_id` is only a
  *slice* of the `u64` minted `entity_reference`. No `hot_reference` type exists
  ([../references/hot-cold-references.md](../../../../../shared/codec/design/references/hot-cold-references.md)).
- **Fix**: define `hot_reference` in `object.rs`; decide whether hot identity is the compact
  `u32` or stays the `u64`. **Open reconciliation.**

## 6. 🔴 No two-phase recoverable lifecycle / status state machine

- **Design** ([lifecycle.md](../intent/lifecycle.md)): a row carries `status`
  (`enqueue → queueing → in_queue → running → complete` / `queue_failed`) + `failed` +
  `tic_state_change`; **enqueue** (stand up target `state_log` rows) and **execute** are
  separate, each idempotent + crash-recoverable via re-drive; an open-rows table (keyed by
  `event_reference`) tracks held state_log rows; timeout eviction on `tic_state_change`.
- **Code**: `bump` opens pending rows inline + `claim`/`resolve` per `(entity, tic)`; no
  enqueue phase, no lifecycle status, no open-rows table, no eviction.
- **Fix**: build the lifecycle (S2/S3). This is the biggest structural change.

## 7. 🔴 No master drop barrier / refcounted GC (correctness scaffolding)

- **Design** ([lifecycle.md](../intent/lifecycle.md)): causality is **strict staging + a master
  drop-then-bump barrier** (no watermark). GC is **dumb**: a **holder table** refcounts pending
  reads/writes per `state_log` row, and GC reclaims only zero-holder, non-latest, old rows.
- **Code**: `tick_gc` uses a horizon heuristic; no holder table; no master drop reducer; `bump`
  doesn't sweep timed-out enqueued work.
- **Fix**: add the holder table + hold acquire/release (S2/S3); the master's `drop_timed_out`
  reducer before `bump` (S3); make GC the trivial zero-holder rule.

## 8. 🟡 Event log not split from data (event shard vs data shard)

- **Design** ([lifecycle.md](../intent/lifecycle.md)): `event_log` on **event shards**; `state`/`state_log`/
  `cold` + the holder table on **data shards**; workers service the former, write the latter.
- **Code**: one module holds both; not separated.
- **Fix**: deployment split (same generic module), later — not a blocker for the DSL landing.

## 9. ⚪ Ordering model: keep `read_rule`, drop `priority`

- **Design** ([lifecycle.md](../intent/lifecycle.md)): dependencies are explicit `await`s + the read rule
  (`resolved_through`), with cross-entity reads at **≤ T−1** → DAG by tic → deadlock-free. **No
  `Phase`, no priority-DAG.**
- **Code**: `shared/tick` has `Phase` (`domain.rs`) and `priority` (`actor_read_tic`).
- **Fix**: keep `read_rule`; **delete `Phase` and `priority`** as the DSL lands (S3/S4).

## 10. ✅ CLOSED (2026-07-14) — `server_reference` is geographic `realm_id:8 | server_id:8`

Landed. The functional `server_type` question is **moot**: it had 0 live consumers (the worker
maps `server_reference → DB` via its `SHARDS` env list, not a type tag), so the whole functional
machinery (`server_type`/`action_reference`/`zone_reference`) was deleted, not relocated.

<details><summary>original divergence</summary>

- **Design** ([../references/reference-vs-id.md](../../../../../shared/codec/design/references/reference-vs-id.md),
  `object-model.md` §5): `server_reference:u16 = realm_reference:u8 + server_id:u8`.
- **Code**: `refs.rs` has `server_reference:u16 = server_type:6 | server_id:10` (functional,
  not geographic). No home for `server_type` under the geographic layout.
- **Fix**: decide where `server_type` (master/worker/edge/db-class routing) lives once
  `server_reference` goes geographic. **Open** (the fork raised earlier: fold type into the
  `u8 server_id`, keep functional, or look role up from a registry).

</details>

---

## Not divergent (working as designed) ✅

- The **generic engine** (`decl_tick_pipeline!` over a payload field list; spine never
  inspects payload) — proven across divergent payload modules.
- The **fence** (`server_id`, `resolve` rejects non-owners) — reused, now fencing rows.
- The **metronome + `+N` tic gap** — kept; the gap widens **+2 → +3** for the enqueue phase
  ([lifecycle.md](../intent/lifecycle.md)).
- The **`entity_reference` / `server_reference` layouts** in `shared/codec/src/refs.rs` —
  re-cut to the reference model (2026-07-14, #5/#10 above).
- The **`object.rs` definition/cold layouts** — re-cut to the reference model (2026-07-14):
  `definition_reference`, cold entry `kind_reference:16|tile:8|data:8`, `data:8`, geographic
  `cold_reference`. Browser-verified. (Remaining: the `cold` *table* still keys by the legacy
  `zone_id:u32` — the geometry cleanup in [todo](../../../../../../work/spacetime-rewrite/todo.md), not blocked.)

## Resolved (no longer open) ✅

- **Cross-shard multi-target atomicity** — was flagged open; resolved by **convergence**
  (idempotent per-shard writes keyed by `event_reference` + re-drive), not distributed
  transactions ([lifecycle.md](../intent/lifecycle.md)). Eventually-consistent over a tic or two, safe.
- **`read_rule` vs `priority` survival** — decided: keep `read_rule`, drop `priority`/`Phase`
  (#9).
