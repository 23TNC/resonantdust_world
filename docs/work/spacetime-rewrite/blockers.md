# Blockers — spacetime rewrite (need human input)

Open items I can't resolve alone, with enough analysis to bring you up to speed without digging.
Open→resolved; a resolved blocker gets a resolution + date, then archives to the bottom.

---

## Open

_(none)_

---

## Resolved

### B-2 · ~~Cold geometry conflicts with world-global `zone_id`~~ — RETRACTED (not a blocker)
**Raised 2026-07-14 · retracted 2026-07-14 (same day, by the user).**

I raised this treating `packed.rs`'s `zone_id` (`region_x:8 | region_y:8 | surface:8 | …`) as the
authoritative world geometry. **It is legacy from the old game** — `surface` doesn't exist in the
go-forward model, and `zone_id` is explicitly "the legacy routing key it will reconcile against"
([spatial-references.md](../../components/shared/codec/design/references/spatial-references.md)).
The go-forward geometry is **realm · region · zone · tile · layer** (each a `u8` of two `u4`
nibbles; realm rides `server_reference`), which the reference model already encodes. So the cold
side is *not* blocked — `cold_reference : u32 = region:8 | zone:8 | tile:8 | layer:8` is geographic
and fits `REF_COLD | server_reference | cold_reference:32`. Lesson: don't preserve legacy code as a
constraint; conform it to the design.

The cold-side re-cut then landed (drop `surface`, retire the flat `zone_id`) — see
[completed.md](completed.md). What B-2 did *not* excuse: the cold **row** still lost two of its
three header fields in that re-cut (`macro_position` + `layer_id`) — tracked as divergence **#11**
in [todo.md](todo.md).

### B-1 · The object-model taxonomy was in-flux — blocked the representation re-keys
**Raised 2026-07-14 · fully resolved 2026-07-14.**

The re-keys (#4 `region_zone`, #5 `hot_reference`, #10 `server_reference`, `event_reference` width,
the object taxonomy/naming) all had to hit an object-model shape the docs marked *in-flux* (D3
"deliberately deferred", "4 open decisions") — so they were decisions, not implementation, and
reserved for the user. Over 2026-07-14 the user co-designed and settled the whole thing:
**[`components/shared/codec/design/reference-model.md`](../../components/shared/codec/design/reference-model.md)**
(+ `intent/`).

- **taxonomy / naming** — `definition / position / data` split; `object_reference` → the object
  *handle*; the u64 blueprint → `definition_reference:u32` (type+kind, `subkind` dropped).
- **#4 `region_zone`** — explicit `macro_position_reference = region:8 | zone:8`.
- **#5 `hot_reference` re-key** — `entity_reference = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`;
  a hot object is `object_reference = hot_reference:32`.
- **#10 `server_reference`** — `realm_id:8 | server_id:8` (geographic); realm a functional unit — realm-uniqueness rides on
  `server_reference`, so no dedicated realm field in `entity_reference`.
- **`event_reference` width** — `u32` (composes into `action_reference`).

**Now unblocked:** implementing all of the above is a **codec re-cut** against a settled target
([codec plan](../../components/shared/codec/plan/README.md)) — no decisions left. The one thing
still *deferred* (not blocked): the `PACK` trigger's hot→cold path, which the pack-criterion
(`data:8` fits ⇒ packable) now defines but nobody calls yet — a normal todo, not a blocker.
