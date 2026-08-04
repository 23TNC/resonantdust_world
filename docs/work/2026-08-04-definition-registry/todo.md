# Plan — definition registry

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), measurements in [`issues.md`](issues.md)._

**Unblocked** — every blocker is answered ([blockers.md](blockers.md)). The packed layout is
FROZEN ([F13](forks.md#f13)): this stream changes no data structure. It authors the taxonomy, moves
numbering to the registry, and leaves `definition_reference` exactly as it is.

## P0 — freeze what a def id means today

- [ ] Inventory every site that PACKS or UNPACKS a `definition_reference`, with what it reads and
      whether a table lookup is reachable there. Acceptance: `issues.md` names file:line for each;
      any site not already in [I1](issues.md#i1)/[I2](issues.md#i2) is investigated before P1.
- [ ] A golden dump of today's `(name → packed def)` for every corpus def, committed as a fixture.
      Acceptance: two runs byte-identical; the fixture is what P4 replays to prove resolution
      through the registry yields the same ids for unchanged content.
- [ ] Grep the corpus + code for every place a def id is STORED (zone kinds, pawn defs, payload
      `PART` words, `cold_row`). Acceptance: the list names each table/field, so P4's re-point can
      be checked against every reader of a stored id.

## P1 — the schema, documented before parsed

- [ ] Write the taxonomy + applicability arrays into `VARIABLES.md` § TOML content schema
      ([F2](forks.md#f2)): `type`/`kind` scalars, `subType`/`variant` arrays, and `texture = "white"`
      kept as the no-art fill. Acceptance: docs-check green; all 13 current defs have a spelling,
      including `biome-tile/default/smooth/wall`.
- [ ] Write the registry ROW into `TABLES.md`: `u32 id, u32 version, string type/subType/kind/
      variant`, its keys, and its readers/writers. Acceptance: the doc names the unique key
      (4 strings + version) and the lookup index (4 strings → max version).
- [ ] Document the **16 variants per (type, subType, kind)** ceiling in `VARIABLES.md` beside the
      packed layout ([F13](forks.md#f13)). Acceptance: the doc states the limit and that the layout
      is unchanged; `pawn/animal/wolf` is named as the kind nearest it (15 of 16).

## P2 — the registry table + allocator

- [ ] Add the registry table to the `index` module with an `ensure(type, subType, kind, variant,
      version) → id` reducer, written by the master ([F11](forks.md#f11)). Acceptance: a module test
      shows a repeat call for one tuple returns the SAME id rather than a second.
- [ ] Allocate ids by burning `kind_id` per version ([F5](forks.md#f5)); `subType` ids stay stable
      across versions. Acceptance: a test bumps one kind's version and asserts the sibling kinds in
      that subType keep their ids.
- [ ] Refuse allocation past a field's width — u12 kind, u4 variant ([F13](forks.md#f13)).
      Acceptance: the reducer errors loudly at the boundary; a test asserts a 17th variant of one
      kind fails rather than wrapping. The wolf is one slot from this today.

## P3 — the loader: taxonomy in, cross-product out

- [ ] Parse `type`/`kind`/`subType[]`/`variant[]` in `shared/content`; DELETE `id = N` and the
      explicit-id law it enforced ([F1](forks.md#f1)). Acceptance: crate tests — a def with arrays
      expands to the right tuple set; an `id` key is now an unknown-field load error.
- [ ] Derive the texture stem from the taxonomy instead of authoring it. Acceptance: the corpus has
      no `texture` field except the `white` no-art fill; the resolver receives the same stems it does
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

## P6 — versioning live

- [ ] Implement `version` bumping on SIMULATION-visible change only ([F12](forks.md#f12)), minting a
      new row + id and leaving the old. Acceptance: editing a data field mints v1; editing art or a
      tint alone does not; both asserted by test.
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
