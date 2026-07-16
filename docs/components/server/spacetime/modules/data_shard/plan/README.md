# Plan — `data_shard` (nothing → built)

_Last updated: 2026-07-15. Nothing is built._

## Do this first

**Verify SpacetimeDB's subscription grammar accepts the disjunction:**

```sql
SELECT * FROM state_log WHERE worker_a = 1 OR worker_b = 1 OR worker_c = 1 OR worker_d = 1
```

Subscription SQL is a string and a subset — this fails at **runtime**, against a live DB, and no
build gate catches it. If it's rejected, use **four subscriptions**, one per column: same rows, same
cost, no redesign. Either way, know before phase 2.

## Phases

1. **Tables.** `state_log` + `state_events` + `state` per
   [`TABLES.md`](../../../../../../TABLES.md). `state_uid` is composite
   (`reserved | entity_reference | tic`) — entity-major, and it *is* the key, so `entity_reference`
   and `tic` are duplicated out as columns because a subscription filters on columns and a reducer
   needs values.
2. **`declare_pending` + `request_state`.** Slot creation + `dirty`; the reverse index in
   `state_events`; claiming a `worker_*` slot **on every row for the entity** (that's what gives the
   worker the history) and stamping the matching `lease_*`. Then confirm the subscription from §Do
   this first actually delivers.
3. **`apply`.** Compose, decrement `dirty`, re-check the block (an earlier tic for this entity still
   dirty → reject), release slots, and `state.upsert` if `PROMOTE` and settled.
4. **`reap` + `gc`.** Free slots whose `lease_*` expired; drop old settled rows. The GC horizon must
   stay far below `TIC_WINDOW` (32767) or tic comparison silently inverts.

## Watch

- **Every tic comparison is `tic::` serial arithmetic, never `<`.** `tic` is a wrapping u16 ring.
  `<` inverts across the wrap: the causality guard stops firing and `gc` misses its *oldest* rows.
- **`state.macro_position_reference` is a projection of the payload** and nothing enforces it. A row
  where the two disagree is invisible in the zone it's in and visible in one it isn't. The move verb
  is where that breaks.

## Ties into

`event_shard`'s plan — its phase 3 calls into phases 2–3 here.
