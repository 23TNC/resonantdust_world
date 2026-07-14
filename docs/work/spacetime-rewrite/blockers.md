# Blockers — spacetime rewrite (need human input)

Open items I can't resolve alone, with enough analysis to bring you up to speed without digging.
Open→resolved; a resolved blocker gets a resolution + date, then archives to the bottom.

---

## Open

_(none)_

---

## Resolved

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
