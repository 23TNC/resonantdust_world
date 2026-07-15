# Divergences — code today vs this design

The honest ledger. Each row is where the implementation does *not* match this folder, with
the file and the fix. Point at a row and say "close it" and it's a well-scoped task.

Source of truth for the *intent* is the rest of this folder; source of truth for the
*current code* is [`server/spacetime/server/pipeline/src/lib.rs`](../../../../../../../server/spacetime/server/pipeline/src/lib.rs)
(the `decl_tick_pipeline!` macro) and [`server/worker/src/main.rs`](../../../../../../../server/worker/src/main.rs).

Legend: 🔴 contradicts the design · 🟡 partial / in-migration · ⚪ naming/cosmetic.

---

## 1. ✅ CLOSED — it was **already done**; this entry was stale (2026-07-14, T-9)

- **Design** ([events.md](../intent/events.md), [event-dsl.md](../design/event-dsl.md)): the event carries
  `actions : Vec<u64>` — a flat postfix (RPN) stream of self-qualified words
  (`op_code:4 | reserved:12 | server_reference:16 | payload:32`), operands (push) + verbs
  (pop+run); the worker interprets it. No `action` enum, no positional slot schema, no actor
  field, no operand table.
- **⚠️ This entry described code that no longer exists.** It claimed a fat struct with
  `actor_key:u64`, `target_key:u64`, `data0/1:u64` and named `*_server_reference` columns.
  **None of those identifiers appear anywhere in the repo.** The word DSL landed with S1/S2 and
  the entry was never retired:
  - `EventLog` is `event_reference:u32 · event_tic:u32 · **actions:Vec<u64>** · targets:Vec<u64> ·
    worker_reference:u16 · status:u8 · failed · tic_state_change` — the design's row.
    (`targets` is designed too — [tables.md](../design/tables.md) lists it and
    [lifecycle.md](../intent/lifecycle.md) calls it the *issuer-designated* write set that
    `bump`/`stand_up` read **without interpreting the program**. Only `event-dsl.md`'s abbreviated
    "row restated" omits it.)
  - The word codec is [`shared/codec/src/event_word.rs`](../../../../../../../shared/codec/src/event_word.rs)
    — `pack_word` / `word_op_code` / `word_server_reference` / `word_payload`, the
    `OP_LITERAL/OBJECT/ACTION/ALIAS` set, and an append-only `ACTION_*` palette; tested for
    roundtrip, field-disjointness, and that the low 48 bits stay a clean qualified reference.
  - The interpreter is [`shared/tick/src/vm.rs`](../../../../../../../shared/tick/src/vm.rs) —
    a **purpose-built** stack machine (its own header says *"This is NOT `shared/dsl`"*, per the
    design), with `SKIP`-encoded forward-only branches, `FAIL`, actor-reading `DAMAGE` via the
    `Reads` trait, and `encode_if`.
  - The worker runs it: `vm::run(body, state, &reads)` per row, folded in `event_reference` order,
    gated by `vm::await_gate`.
- **The lesson is the entry, not the code.** A stale `current/` row sent a whole phase (P2, "the
  main event") after work that was already finished, and its *fix* line additionally told us to
  build the VM on `shared/dsl` — which the design explicitly forbids. `current/` is a convenience
  cache; **`design/` is the truth**. Re-read `design/` at the start of a task, not the ticket.

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

## 3. ✅ CLOSED (2026-07-14, T-7) — the standalone `cold_things` / `cold_tiles` modules are gone

- **Design** ([tables.md](../design/tables.md)): cold is **one `cold` table inside the generic
  module**, beside hot `state`.
- **This entry was itself stale.** It claimed the modules "still physically live" and that
  `2b2fc58` / `d47f152` retired only their *wire paths*. Not so — those commits deleted the
  **sources too**; `git ls-files` has carried only `chat`/`index`/`players`/`shard` since. What
  actually remained on disk was **untracked build detritus** (`target/` + `Cargo.lock`, ~530MB
  across the three), plus `experiment`.
- **Nor were they being built.** `rd_list_modules` requires a `Cargo.toml` to treat a directory as
  a module unit, and none of the three had one — so `rd redeploy` skipped them. They cost disk, not
  deploys.
- **Done:** removed the stale artifact dirs; deleted the dead `cold_tiles) / cold_things)` `fam`
  arms in [`bin/lib/redeploy.sh`](../../../../../../../bin/lib/redeploy.sh) (the only real
  repo-visible leftover). `default_cold_tiles_db()` / `default_cold_things_db()` were already gone.
  Verified: `rd redeploy` plans clean.

> **Update 2026-07-14.** #2, #5 and #10 are **closed** (the re-cut landed + is verified —
> [`work/…/completed.md`](../../../../../../work/spacetime-rewrite/completed.md)). #4 is **not**
> blocked (B-2 was retracted — it wrongly treated the legacy `zone_id`/`surface` as a constraint);
> its remaining half was folded into **#11**, the cold row's missing header — **now also closed**
> (2026-07-14), which closes #4 with it. That settles every **identity/keying** divergence
> (#2, #4, #5, #10, #11), and #3/#9 with them (T-7/T-8/T-9).
>
> **Update 2026-07-14 (T-9) — this ledger was badly stale.** Scouting P2 found that **#1, #6 and
> #7 were already built** by the S0–S7 staged build and had simply never been retired here; each
> entry's "Code" line described identifiers that no longer exist (`actor_key`, `target_key`,
> `data0/1`) or absences that are present (`Holder`, `OpenRows`, `drop_timed_out`, the `STATUS_*`
> machine). They are now closed **against verified evidence**, not assumption.
>
> **Only #8 remains open** — the event/data shard split, which the design defers anyway. The
> "pipeline-shape backlog" was a phantom; see [`plan/README.md`](../plan/README.md).

## 11. ✅ CLOSED — the `cold` row omitted `macro_position` + `layer_id`; its key couldn't select a row

**Raised 2026-07-14 (regression I introduced in the re-cut; caught by the user). Closed 2026-07-14 —
conformed + live-verified;** see [deviations.md](../../../../../../work/spacetime-rewrite/deviations.md)
**D-3**, the deviation that produced it.

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
- **Fixed as designed.** `cold` = PK `cold_row_reference:u64` (`reserved:28 | macro_position:16 |
  type_reference:16 | layer_id:4`) + `macro_position:u16` (btree, the subscription key),
  `type_reference:u16`, `layer_id:u8`; `cold_removed` **1:1** on the same key with `Vec<u8>`
  `tile_reference` tombstones. `find_or_mint` + the edge's interact scan select
  `(macro_position, type_id, layer_id)` — via the shared `cold_row_selects()` rule in the codec, so
  the two call sites can't drift — then match `tile_reference`. Worldgen emits `layer_id` per row.
- **Verified live** (shard republished + reseeded): two targets at the *same* tile (7,7) differing
  only in `layer_reference` each minted exactly the object they named — `type_id=2` → the tree,
  `type_id=1` → the ground. Previously iteration-order luck. Both carry `kind=1` (tree = thing-id 1,
  grass = tile-def 1), which is precisely why the old logs read `kind=1` either way and the bug hid.
- **Pinned by test**: `object::tests::a_target_mints_the_object_it_names_not_the_ground_under_it`
  (plus `cold_row_reference_is_the_header_composite` / `cold_row_of_selects_by_macro_type_and_layer`).

## 4. ✅ CLOSED — geometry is geographic; the cold row's `macro_position` landed with #11

`zone_id` is now the **geographic** `realm:8 | region:8 | zone:8 | reserved:8` (G1 — surface
retired), and the cold `entity_reference` is a geographic `cold_reference` (G2). The `cold` table
now keys by the header's `macro_position_reference:u16` (`zone_id` dropped), carrying `layer_id`
alongside. This was **not** a mere key-width compaction (an earlier note here wrongly said so): the
row header is what a reader reconstructs a `position_reference` from. Closed with **#11**; the edge
reconstructs the client-facing `zone_id` from its shard's realm + `macro_position` when relaying.
The original divergence (flat legacy `zone_id`):

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

## 6. ✅ CLOSED — already built by S2/S3; this entry was stale (verified 2026-07-14, T-9)

- **Design** ([lifecycle.md](../intent/lifecycle.md)): a row carries `status`
  (`enqueue → queueing → in_queue → running → complete` / `queue_failed`) + `failed` +
  `tic_state_change`; **enqueue** (stand up target `state_log` rows) and **execute** are
  separate, each idempotent + crash-recoverable via re-drive; an open-rows table (keyed by
  `event_reference`) tracks held state_log rows; timeout eviction on `tic_state_change`.
- **⚠️ Every claim in this entry's "Code" line is false today.** It read: *"`bump` opens pending
  rows inline + `claim`/`resolve` per `(entity, tic)`; no enqueue phase, no lifecycle status, no
  open-rows table, no eviction."* Checked one by one against
  [pipeline/src/lib.rs](../../../../../../../server/spacetime/server/pipeline/src/lib.rs):
  - **lifecycle status** — `STATUS_ENQUEUE` / `QUEUEING` / `IN_QUEUE` / `RUNNING` / `COMPLETE` /
    `QUEUE_FAILED`, exactly the design's machine, plus `failed` and `tic_state_change` on the row.
  - **enqueue phase** — `claim` → `stand_up` (per target) → `ready`, separate from execute
    (`resolve`); `find-or-mint` runs *at `stand_up`*, which is how divergence #2 closed.
  - **open-rows table** — `OpenRows`, keyed by `event_reference` (+ `source_shard`,
    `state_log_id`), so enqueue can back out.
  - **eviction** — `drop_timed_out` evicts `ENQUEUE`/`QUEUEING` rows past `ENQUEUE_WINDOW_TICS`
    off `tic_state_change`, releasing their holds.
- **It was already in our own log.** [`completed.md`](../../../../../../work/spacetime-rewrite/completed.md)'s
  2026-07-13 S0–S7 entry says *"pipeline module (event_log/state/holder/cold/meta + **lifecycle
  reducers**), **worker two-phase loop**, master drop→bump … Live-verified with real binaries."*
  This ledger simply never got retired to match.

## 7. ✅ CLOSED — already built; this entry was stale (verified 2026-07-14, T-9)

- **Design** ([lifecycle.md](../intent/lifecycle.md)): causality is **strict staging + a master
  drop-then-bump barrier** (no watermark). GC is **dumb**: a **holder table** refcounts pending
  reads/writes per `state_log` row, and GC reclaims only zero-holder, non-latest, old rows.
- **⚠️ Stale in the same way.** It read: *"`tick_gc` uses a horizon heuristic; no holder table; no
  master drop reducer; `bump` doesn't sweep timed-out enqueued work."* In fact:
  - **holder table** — `Holder` (btree on `state_log_id`), with `release_holds` on the abort /
    drop / resolve paths.
  - **master drop barrier** — `drop_timed_out` exists **and the master calls it immediately before
    `bump`** ([master/src/main.rs](../../../../../../../server/master/src/main.rs)): `drop_timed_out()`
    then `bump(cur + 1)`. That ordering *is* the causality guard.
  - **GC is the trivial rule, not a heuristic** — `tick_gc`'s own doc says *"Dumb reclamation. Drop
    `state_log` rows with **no holders**, **not the entity's latest resolved**, and old"*, and the
    filter is exactly `dirty == 0 && tic < horizon && not-latest && holder-count == 0`. The
    `horizon` is the design's "old", not a substitute for refcounting.

## 8. 🟡 Event log not split from data (event shard vs data shard)

- **Design** ([lifecycle.md](../intent/lifecycle.md)): `event_log` on **event shards**; `state`/`state_log`/
  `cold` + the holder table on **data shards**; workers service the former, write the latter.
- **Code**: one module holds both; not separated.
- **Fix**: deployment split (same generic module), later — not a blocker for the DSL landing.

## 9. ✅ CLOSED (2026-07-14, T-8 + T-9) — both the priority-DAG and the `Phase` band are gone

- **Design** ([lifecycle.md](../intent/lifecycle.md)): dependencies are explicit `await`s + the read rule
  (`resolved_through`), with cross-entity reads at **≤ T−1** → DAG by tic → deadlock-free. **No
  `Phase`, no priority-DAG.**
- **✅ `priority` is gone** — `shared/tick/src/priority.rs` (`priority`, `rank`, `actor_read_tic`)
  had **no callers anywhere**; only its own tests exercised it. Deleted with its re-export and the
  stale priority-DAG prose in `lib.rs` / `domain.rs`.
- **✅ `Phase` is gone too** (T-9). I deferred it in T-8 on the grounds that `resolve_events`
  composed by phase and was therefore live — **that was wrong**: `resolve_events` itself had no
  callers outside its own tests. The worker runs `vm::run` per row and folds in `event_reference`
  order; it never had a phase in it. `Phase`, `action_phase` and the band constants are deleted;
  `resolve_events` survives as the `Domain`-generic fold **in the caller's order**, which is the
  design's model (tics + `await`; no intra-tic guarantee).
- **One real behaviour change, deliberate.** `Phase` re-ordered a tic's events inbound → data →
  outbound *regardless of arrival order*, so a `MOVE` arriving before its own same-tic `SPAWN` was
  silently rescued. That rescue is gone by design — *"if two rows must order, the later one
  `await`s the earlier"*. Nothing live depends on it: a spawn is appended before the move that
  follows it, so it holds the lower `event_reference` and still lands first. The old test asserted
  the rescue by feeding the events backwards; it now asserts caller-order in both directions.

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
  `definition_reference`, `kind_pos_reference` (`kind_reference:16|tile_reference:8|data:8`),
  `data:8`, geographic `cold_reference`. Browser-verified. The `cold` *table* now keys by
  `cold_row_reference:u64` (`macro_position:16|type_reference:16|layer_id:4`) — the legacy
  `zone_id:u32` key is gone. ⚠️ An earlier note here called that a deferrable "key-width
  compaction"; it wasn't — the missing header **was** the D-3 live bug (#11).

## Resolved (no longer open) ✅

- **Cross-shard multi-target atomicity** — was flagged open; resolved by **convergence**
  (idempotent per-shard writes keyed by `event_reference` + re-drive), not distributed
  transactions ([lifecycle.md](../intent/lifecycle.md)). Eventually-consistent over a tic or two, safe.
- **`read_rule` vs `priority` survival** — decided: keep `read_rule`, drop `priority`/`Phase`
  (#9).
