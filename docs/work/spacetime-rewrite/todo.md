# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

## The codec re-cut + shard re-keys (now UNBLOCKED — B-1 closed 2026-07-14)

Functional-neutral **representation** reconciliations — the system works fully without them; they
change *shape*, not *behavior*. The object model is now fully settled
([reference-model.md](../../components/shared/codec/design/reference-model.md)), so these are
**implementation against a fixed target**, not decisions. The codec re-cut lands the shapes; the
shard re-keys follow. (Primary tracking: [codec plan](../../components/shared/codec/plan/README.md).)

- **2026-07-14** · **codec re-cut** — `object.rs`/`refs.rs` v1 → the reference model
  (`definition_reference`, `position_reference`/`cold_reference`, cold row + `data:8`,
  `object_reference` union, `entity_reference`, `server_reference = realm:8|server_id:8`).
- **2026-07-14** · **#5 `hot_reference` re-key** — `state`/`state_log` key `u64 entity_key` →
  `object_reference = hot_reference:32` (server-qualified). Big PK change; target now defined.
- **2026-07-14** · **#4 `region_zone` cold keying** — `cold` keys flat `zone_id` →
  `macro_position_reference = region:8|zone:8`.
- **2026-07-14** · **#10 `server_reference`** — → `realm:8|server_id:8` (decided).
- **2026-07-14** · **`event_reference` → `u32`** — the `event_log` PK `u64` → `u32` (composes into
  `action_reference`).
- **2026-07-14** · **`PACK` trigger** — `pack_settle` exists; *who* calls it is unbuilt. The
  pack-criterion is now defined (`data:8` fits ⇒ packable; else stays hot) — a normal todo.

## Not blocked (small, deferrable)

- **2026-07-14** · **Cross-shard foreign Phase-1 hold** — a foreign target gets no pending
  row/holder on its home shard during the in-flight window (read-rule/GC visibility). The
  convergent *write* is done ([completed.md](completed.md) #4); this is the in-flight *hold*.
  Eventually-consistent today; low priority.
