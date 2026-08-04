# Plan — definition registry

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), measurements in [`issues.md`](issues.md)._

**P1 and P2 are gated on [`blockers.md`](blockers.md)** — B1/B2 fix the schema, B4 fixes where the
registry lives. P0 can start now; nothing below P0 should begin before those are answered, because
each writes a layout that stored data then depends on.

## P0 — freeze what a def id means today

- [ ] Inventory every site that PACKS or UNPACKS a `definition_reference`, with what it reads and
      whether a table lookup is reachable there. Acceptance: `issues.md` names file:line for each;
      any site not already in [I1](issues.md#i1)/[I2](issues.md#i2) is investigated before P1.
- [ ] A golden dump of today's `(name → packed def)` for every corpus def, committed as a fixture.
      Acceptance: two runs byte-identical; the fixture is what P4 replays to prove resolution
      through the registry yields the same ids for unchanged content.
- [ ] Grep the corpus + code for every place a def id is STORED (zone kinds, pawn defs, payload
      `PART` words, `cold_row`). Acceptance: the list names each table/field and its width, so B2's
      layout choice can be costed against real stored data.

## P1 — the schema, documented before parsed _(needs B1, B2)_

- [ ] Write the taxonomy + applicability arrays into `VARIABLES.md` § TOML content schema
      ([F2](forks.md#f2)): `type`/`kind` scalars, `subType`/`variant` arrays, B1's answers for
      `white` and the 4-segment forms. Acceptance: docs-check green; every current def has a
      spelling.
- [ ] Write the registry ROW into `TABLES.md`: `u32 id, u32 version, string type/subType/kind/
      variant`, its keys, and its readers/writers. Acceptance: the doc names the unique key
      (4 strings + version) and the lookup index (4 strings → max version).
- [ ] Record B2's layout answer in `VARIABLES.md` + `shared/codec` docs before any code moves.
      Acceptance: the packed `definition_reference` layout in the doc matches the one the codec will
      carry, and the change (if any) names what stored data must be re-stamped.

## P2 — the registry table + allocator _(needs B4)_

- [ ] Create the registry module/table per B4 with an idempotent `ensure(type, subType, kind,
      variant, version) → id` reducer. Acceptance: two concurrent calls for one tuple return the
      SAME id; a module test proves it.
- [ ] Allocate ids by burning `kind_id` per version ([F5](forks.md#f5)); `subType` ids stay stable
      across versions. Acceptance: a test bumps one kind's version and asserts the sibling kinds in
      that subType keep their ids.
- [ ] Refuse allocation past a field's width (u12 kind, u4 variant unless B2 widens). Acceptance:
      the reducer errors loudly at the boundary; a test asserts the 17th variant of one kind fails
      rather than wrapping.

## P3 — the loader: taxonomy in, cross-product out

- [ ] Parse `type`/`kind`/`subType[]`/`variant[]` in `shared/content`; DELETE `id = N` and the
      explicit-id law it enforced ([F1](forks.md#f1)). Acceptance: crate tests — a def with arrays
      expands to the right tuple set; an `id` key is now an unknown-field load error.
- [ ] Derive the texture stem from the taxonomy instead of authoring it. Acceptance: the corpus has
      no `texture` field except B1's `white` case; the resolver receives the same stems it does
      today for every existing def.
- [ ] Expand the cross-product at load and call `ensure` per tuple. Acceptance: loading today's
      corpus mints exactly the tuple count the arrays imply, and re-loading mints nothing new.

## P4 — resolution moves off the corpus

- [ ] Serve the registry to clients (initial table + updates on change) alongside `/content`.
      Acceptance: a client that boots with an empty cache receives the full table and can resolve
      `("biome-thing","default","conifer","4") → id`.
- [ ] Re-point `Bundle`'s `tile_def_id` / `thing_object_id` at the registry, keeping the accessor
      signatures. Acceptance: P0's golden fixture replays — every unchanged def resolves to the same
      packed id it had before the stream.
- [ ] Order the two updates so a client never holds content referencing ids it lacks. Acceptance: a
      drill that hot-swaps content and the registry together shows no unresolved id in the client
      console.

## P5 — the consumers swap, and the workarounds die

- [ ] Delete the stem-parsing in `client/npc` — species comes from the authored taxonomy.
      Acceptance: `rd-npc` resolves `def 0x…` with the same value it logs today; no `split('/')` on
      a texture stem remains in the repo.
- [ ] Delete the code-owned species palette in `shared/codec/src/object.rs`. Acceptance:
      `pawn_species_subtype_id` is gone; adding a species is a corpus edit with no code change,
      proven by adding one in a test corpus.
- [ ] Delete `is_append_compatible_with` and its call site. Acceptance: a corpus REORDER hot-swaps
      cleanly and stored zones still render the same tiles — the guard's whole premise is gone.

## P6 — versioning live _(needs B3)_

- [ ] Implement `version` bumping per B3's boundary, minting a new row + id and leaving the old row.
      Acceptance: editing a data field mints v1; editing art alone does not; both asserted by test.
- [ ] Resolve by `max(version)` for new placements ([F6](forks.md#f6)). Acceptance: after a bump,
      a newly placed object carries the v1 id while an existing entity still carries v0 and still
      renders and behaves as v0.
- [ ] Prove the apple case end to end on a real field: change one def's data, place a new one beside
      an old one. Acceptance: the two coexist with different behaviour, in the client, with no
      migration step run.

## P7 — reclaim: designed, NOT built ([F7](forks.md#f7))

- [ ] Write the reclaim design into the registry component's `intent/`: the sweep, what "gone"
      must mean, and why `entity_state_log` makes it dangerous. Acceptance: the doc names every
      place an id can survive (shards, event log, backups, stale client tables).
- [ ] Add a `droppedCoordinates`-style counter or query showing how much `kind_id` space versioning
      has consumed. Acceptance: a number is reportable, so the decision to build reclaim is made on
      pressure rather than on principle.
