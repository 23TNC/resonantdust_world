# The table taxonomy

A shard module is one `decl_tick_pipeline!` invocation. It carries a **hot side** (the
ticking working set + the client-visible latest), a **cold side** (the settled store), and
a few **meta** rows (metronome, shard identity, mint counter). This doc is the intended
shape of each; the tick mechanics live in [events.md](../intent/events.md), the resolution *lifecycle*
in [lifecycle.md](../intent/lifecycle.md), the hot/cold bridge in [hot-cold.md](../intent/hot-cold.md).

> **Event shards vs data shards** ([lifecycle.md](../intent/lifecycle.md)). The `event_log` (rows +
> their lifecycle `status`) lives on **event shards** that workers *service*; the game data
> (`state_log` / `state` / `cold` + the **holder table**) lives on **data shards**
> that workers *write to*. The tables below are the data-shard tables plus the `event_log`;
> which physical shard a table lands on is a deployment fact, not a code fork.

> ✅ **AGREED — the generic-engine principle.** The engine's scheduling spine never
> inspects the game payload. The macro takes a `payload: { … }` field list and copies
> those fields verbatim through `state_log` → `state`. So the tables below split cleanly
> into a **fixed control spine** (identical for every shard) and a **payload** (per shard
> class). Nothing in the spine branches on what the payload *means*.

---

## Hot side

### `state` — the client-visible latest ✅

One row per live (hot) object: its resolved value at its latest tic. Clients subscribe
here and never see unresolved/lookahead rows.

```
state
  entity_key : u64     PK   ← the hot object's identity (see note)
  tic        : u32          the tic this value is resolved at
  <payload…>               copied verbatim from the resolved state_log row
```

- ✅ **Key (DONE 2026-07-14).** The `u64 entity_key` now holds the reference model's
  `entity_reference = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`; a
  hot object is `REF_HOT | server_reference | hot_reference:32` — the per-server `hot_reference`
  paired with `server_reference` for global uniqueness, as intended. Column kept `u64` (so it stays
  server-qualified — see [`work/…/forks.md`](../../../../../../work/spacetime-rewrite/forks.md)).
- ✅ **`state` must be queryable by location** (realm·region·zone·position·layer·type_id) so
  enqueue's `find-or-mint` can resolve a `cold_reference` to the entity at that location
  ([hot-cold.md](../intent/hot-cold.md)).

### holder table — refcount for dumb GC ✅

Which `event_reference`s currently hold a **pending read or write** on a `state_log` row.
Workers add a holder when they stand up a target (write) or register a source they'll read
(read), and remove it on the row's terminal transition. GC reclaims a `state_log` row only when
it has **no holders** (+ not-latest + old) — so GC makes no correctness decision
([lifecycle.md](../intent/lifecycle.md) §GC). There is **no read watermark**; causality is the pipeline's
strict staging + the master's drop, not a per-entity timestamp.

```
holder            (data shard)
  state_log_id : u64   the held row
  event_reference : u64
  kind : u8            read | write
```

### `state_log` — the tick working set ✅

The sparse change-log **+** work queue **+** per-entity frontier, fused into one table.
This is the worker's whiteboard.

```
state_log
  id          : u64   PK auto_inc   surface key only; identity is (entity_key, tic)
  entity_key  : u64   btree
  tic         : u32
  dirty       : u16                 >0 ⇒ pending work (value = event count); 0 ⇒ resolved
  <payload…>                        the resolved (or base-carried) game fields
  server_id   : u16                 fence token: the worker that owns this (entity, tic)
  created_at  : u32
  assigned_at : u32                 lease clock for claim eviction
  status      : u8
```

- `dirty > 0` is a **work item**: this `(entity, tic)` has events awaiting resolution.
- `dirty == 0` is a **resolved value**: the entity's state at that tic.
- Idle entities write nothing here — the log is sparse.
- The **fence**: `server_id` is the only worker allowed to `resolve` this row; a
  stale/evicted worker's write is rejected. This is what makes multi-worker resolution
  safe without locks.

---

## Cold side

### `cold` — the settled store ✅ (shape) / ⚠️ (role)

Settled, packed, static objects — terrain and at-rest things. **Never ticks** (`tick_gc`
ignores it). One row per `(zone, type_reference)`: the shared type half once, plus a `Vec`
of the per-instance kind halves.

```
cold
  cold_key       : u64  PK      packs (zone, type_reference)
  region_zone    : u16          the subscription key (region_zone_reference)
  type_reference : u32          the shared object_type_reference (type/subtype/layer)
  kinds          : Vec<u32>     the object_kind_references settled under it
  version        : u32          bumped on mutation → drives client re-send
```

- ✅ **`Vec<u32>`, not columnar.** A zone is 256 tiles; keep the Vec (`object-model.md`
  Decided).
- ✅ **Cold uniqueness rule:** one object per `(type, layer, tile)`, subtype-agnostic,
  **reducer-enforced** (reject-if-present), not a PK.
- ⚠️ **Keyed by `region_zone`, realm implied by the shard** — see [hot-cold.md](../intent/hot-cold.md)
  §shard classes. Today the code keys cold by a flat `zone_id` and stores it as the routing
  column; the geographic `region_zone` key is the target. (Divergences.)

### `cold_removed` — the cold modification delta ✅

A small companion to `cold`: per zone, the objects **removed** from cold since the last
compaction (each object minted hot leaves one). Instead of mutating the big `cold` `Vec` on
every unpack — which re-transmits the whole zone row to every subscriber — we append a tiny
tombstone here.

```
cold_removed
  zone_key : u32         PK   the zone (region_zone)
  removed  : Vec<u16>         each u16 = x:4 | y:4 | layer:4 | type_id:4
  version  : u32              bumped on append → drives client re-send (tiny row)
```

- ✅ **Each `u16` is the intra-zone part of a `cold_reference`** (the whole ref minus its
  region/zone, which are the row's `zone_key`).
- ✅ **Written at mint (unpack).** Enqueue's `find-or-mint` appends the removed position here
  rather than rewriting the `cold` row. The entry also serves as the **"already unpacked"
  marker** for `find-or-mint` idempotency.
- ✅ **Clients render `cold` minus `cold_removed`.** They subscribe to both by zone and omit any
  cold object whose `(x, y, layer, type_id)` is in `removed`.
- ✅ **Compacted by a `PACK` action, not GC.** A worker-resolved `PACK` applies the tombstoned
  kinds to the `cold` rows and clears `removed` (and settles idle hot objects back in). So the
  `cold` `Vec` mutates (and re-sends) at most once per compaction, not once per unpack — and
  compaction is normal recoverable work, not a GC correctness job ([hot-cold.md](../intent/hot-cold.md)).
- ✏️ **Additions (`pack`) could mirror this** with a `cold_added` delta if we also want to avoid
  re-sending on settle. Not built yet — removals only, per the immediate need.

---

## Event side

### `event_log` — rows + their lifecycle ✅

The plan/actions and the state machine that resolves them (full shape in [events.md](../intent/events.md),
lifecycle in [lifecycle.md](../intent/lifecycle.md)). Beyond `event_reference` / `event_tic` /
`actions : Vec<u64>` / `targets`, it carries the lifecycle columns:

```
event_log (lifecycle columns)
  status            : u8    enqueue | queueing | in_queue | running | complete | queue_failed
  failed            : bool  set alongside queue_failed (back-out driver)
  tic_state_change  : u32   the tic this row entered its current status (timeout/eviction)
```

- **No side table for the state machine** — it's these columns on the row.
- **An open-rows table** (keyed by `event_reference`) records which `state_log` rows an
  enqueue holds open, so a re-drive can finish or back out idempotently
  ([lifecycle.md](../intent/lifecycle.md) Phase 1).

---

## Meta rows (fixed, one row each) ✅

| table           | PK  | holds | purpose |
|-----------------|-----|-------|---------|
| `tic_meta`      | `0` | `master_tic : u32` | the metronome; advanced by the master via `bump` |
| `shard_meta`    | `0` | `shard_id : u16`   | this shard's `server_reference`; set once at deploy |
| `object_counter`| `0` | `next : u32`       | monotonic mint counter → the `entity_id`/`hot_reference` half |

These are pure bookkeeping; no game logic reads them beyond identity/tic.

---

## The shape in one picture

```
   EVENT SHARD                             DATA SHARD(S)
  ┌────────────────────────┐             ┌──────────────────────────────────────────────┐
  │ event_log              │  worker      │  HOT   state_log ──▶ state ◀── clients        │
  │  actions:Vec<u64>      │  enqueue     │        (work+frontier) (latest)               │
  │  targets · status      │ ──stand up──▶│        + holder table (refcount for GC)       │
  │  (enqueue→…→complete)  │  execute     │  COLD  cold (settled Vec<u32>) ◀─mint_hot/pack─┤
  │        ▲               │ ──write────▶ │  META  tic_meta · shard_meta · object_counter │
  │        │ append        │              └──────────────────────────────────────────────┘
  │   (edge/npc/worker)    │   workers service event shards; write results to data shards;
  └────────────────────────┘   every write idempotent (event_reference); re-drive to converge
```

*(Physical split — one generic `decl_tick_pipeline!` module still defines the tables; a
deployment decides whether an instance carries the event side, the data side, or both.)*

- The **spine** (`event_log`, `state_log`, `state`, meta) is identical for every shard.
- The **payload** differs per shard class but the engine never looks inside it.
- cold→hot mint is **absorbed into enqueue** (`find-or-mint` on a cold target); `pack`
  (settle) is an execute op — not side-door reducers ([hot-cold.md](../intent/hot-cold.md),
  [lifecycle.md](../intent/lifecycle.md)).
- `event_log` carries a `Vec<u64>` **program** per event, not fixed action columns; the
  worker VM executes it. Full shape in [events.md](../intent/events.md).
