# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first. Most items here are **blocked** — see [blockers.md](blockers.md).

## Blocked on the object-model decisions (see blockers.md · B-1)

These are functional-neutral **representation** reconciliations — the system works fully without
them; they change *shape*, not *behavior*. Each turns on the in-flux object-model taxonomy, so
they can't start until B-1 is resolved.

- **2026-07-14** · **#5 `hot_reference` u32 re-key** — `state`/`state_log` key on `u64
  entity_key`; target is the per-server `hot_reference:u32` (decision D3, deliberately deferred).
  Big PK change. Note: #3 showed actor-reads do NOT need this after all.
- **2026-07-14** · **#4 `region_zone` cold keying** — `cold` keys by flat `zone_id`; target is
  `region_zone:u16`.
- **2026-07-14** · **#10 geographic `server_reference`** — `server_type:6|server_id:10`
  (functional) vs `realm:u8|server_id:u8` (geographic).
- **2026-07-14** · **`event_reference` width** — the `ALIAS` word carries a `u32`;
  `event_reference` is a `u64` PK. Fine while refs are small; reconcile with the #5 re-key.
- **2026-07-14** · **`PACK` trigger** — `pack_settle` exists; *who* calls it (periodic sweep /
  edge) is unbuilt, and the caller needs a hot→cold **type map** (object-model territory).

## Not blocked (small, deferrable)

- **2026-07-14** · **Cross-shard foreign Phase-1 hold** — a foreign target gets no pending
  row/holder on its home shard during the in-flight window (read-rule/GC visibility). The
  convergent *write* is done ([completed.md](completed.md) #4); this is the in-flight *hold*.
  Eventually-consistent today; low priority.
