# Plan — definition registry

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md), measurements in [`issues.md`](issues.md)._

**Unblocked** — every blocker is answered ([blockers.md](blockers.md)). The packed layout is
FROZEN ([F13](forks.md#f13)): this stream changes no data structure. It authors the taxonomy, moves
numbering to the registry, and leaves `definition_reference` exactly as it is.

## P0 — freeze what a def id means today

- [x] Inventory every site that PACKS or UNPACKS a `definition_reference`, with what it reads and
      whether a table lookup is reachable there. Acceptance: `issues.md` names file:line for each;
      any site not already in [I1](issues.md#i1)/[I2](issues.md#i2) is investigated before P1. →
      [I6](issues.md#i6): 7 sites, 3 pack / 4 unpack. The finding that reshapes P4 — the relay
      COMPOSES a def from the row's own fields, so the READ path never needs the registry.
- [x] A golden dump of today's `(name → packed def)` for every corpus def, committed as a fixture.
      Acceptance: two runs byte-identical; the fixture is what P4 replays to prove resolution
      through the registry yields the same ids for unchanged content. → split by crate boundary:
      `name → kind_id` in the golden corpus (blessed), packed defs in `npc::def_fixture` (codec
      lives there). Wolf pins at `0x30010070`, the value the live npc logs.
- [x] Grep the corpus + code for every place a def id is STORED (zone kinds, pawn defs, payload
      `PART` words, `cold_row`). Acceptance: the list names each table/field, so P4's re-point can
      be checked against every reader of a stored id. → [I7](issues.md#i7): 5 stores, two shapes —
      tiles/things keep the KIND HALF only, pawns store the WHOLE def (table + payload `PART`).
      Nothing stores a NAME.

## P1 — the schema, documented before parsed

- [x] Write the taxonomy + applicability arrays into `VARIABLES.md` § TOML content schema
      ([F2](forks.md#f2)): `type`/`kind` scalars, `subType`/`variant` arrays, and `texture = "white"`
      kept as the no-art fill. Acceptance: docs-check green; all 13 current defs have a spelling,
      including `biome-tile/default/smooth/wall`. → the id law replaced by the taxonomy table;
      `name` survives as the resolution key ([F14](forks.md#f14)) since it diverges from `kind`.
- [x] Write the registry ROW into `TABLES.md`: `u32 id, u32 version, string type/subType/kind/
      variant`, its keys, and its readers/writers. Acceptance: the doc names the unique key
      (4 strings + version) and the lookup index (4 strings → max version). → `index` §
      `definitions`; master is the sole writer; old rows never deleted or rewritten.
- [x] Document the **16 variants per (type, subType, kind)** ceiling in `VARIABLES.md` beside the
      packed layout ([F13](forks.md#f13)). Acceptance: the doc states the limit and that the layout
      is unchanged; `pawn/animal/wolf` is named as the kind nearest it (15 of 16). → and separates
      the TWO overflow behaviours: extra ART truncates (existing, by design), an extra AUTHORED
      variant is a load error.

## P2 — the registry table + allocator

- [x] Add the registry table to the `index` module with an `ensure(type, subType, kind, variant,
      version) → id` reducer, written by the master ([F11](forks.md#f11)). Acceptance: a module test
      shows a repeat call for one tuple returns the SAME id rather than a second. → `definitions` +
      `ensure_definition`, published and exercised LIVE: repeat call = one row; a colliding id is
      rejected by name. Signature deviates ([deviations.md](deviations.md), [I8](issues.md#i8)).
- [x] Allocate ids by burning `kind_id` per version ([F5](forks.md#f5)); `subType` ids stay stable
      across versions. Acceptance: a test bumps one kind's version and asserts the sibling kinds in
      that subType keep their ids. → `master/src/defs.rs::compose`; the sibling `rock` is
      bit-identical across a conifer bump, and subType/variant nibbles are untouched.
- [x] Refuse allocation past a field's width — u12 kind, u4 variant ([F13](forks.md#f13)).
      Acceptance: the reducer errors loudly at the boundary; a test asserts a 17th variant of one
      kind fails rather than wrapping. The wolf is one slot from this today. → `AllocError` with
      three named arms; the test also DEMONSTRATES the wrap (slot 16 packs to variant 0), which is
      why refusing beats absorbing.

## P3 — the taxonomy, ADDITIVE ([I9](issues.md#i9) — `id` stays as the seed)

- [x] Author `type`/`kind`/`subType[]`/`variant[]` on all 19 defs in `tiles.toml` + `things.toml`,
      leaving `id = N` in place. Acceptance: the golden fixture passes UNCHANGED — the taxonomy is
      additive and moves no id. → 6 tiles + 11 things; golden byte-identical.
- [x] Parse the four taxonomy fields in `shared/content` and expose them on the `Bundle`.
      Acceptance: a crate test reads `wall_smooth` back as
      `biome-tile / default / smooth / wall`; an unknown key is still a load error. → a `Taxonomy`
      type + `tile_taxonomy`/`thing_taxonomy`; 3 tests, incl. a HALF-authored taxonomy being a load
      error (it would mint wrong rows silently).
- [x] Derive the texture stem from the taxonomy, keeping `texture` only for the `white` no-art
      fill. Acceptance: `tile_texture_stems()` / `thing_texture_stems()` are byte-identical to the
      golden's current values for all 19 defs. → 6 authored `texture` lines DELETED and the golden
      still passes; the rule is named-variant-is-part-of-the-address, numeric-is-appended.
- [x] Expand the cross-product into `(tuple, seed id)` pairs in `master/src/defs.rs`. Acceptance: a
      unit test over the real corpus yields one entry per tuple, and every id matches the one P0's
      golden pins for that def. → `allocations()`; the wolf lands on `0x30010070`, grass on kind 1,
      conifer on 16 rows sharing one kind_id, and no id is allocated twice.

## P4 — the registry becomes the authority

- [x] Call `ensure_definition` per tuple from the master at content load. Acceptance: a fresh
      `index` DB ends up with one row per tuple and `resolve_definition` returns the seeded id for
      every def in P0's golden. → **182 rows seeded live** on a wiped DB; wolf lands on
      `0x30010070` and `smooth/wall` on kind 6 / slot 0. Non-fatal like every other uplink.
- [ ] Serve the registry to clients (initial table + updates on change) alongside `/content`.
      Acceptance: a client booting with an empty cache receives the table and can resolve
      `("biome-thing","default","conifer","4")` locally.
- [ ] Re-point `Bundle`'s `tile_def_id` / `thing_object_id` at the registry, keeping the accessor
      signatures. Acceptance: P0's golden replays — every unchanged def resolves to the id it had
      before the stream.
- [ ] Order the two updates so a client never holds content referencing ids it lacks. Acceptance: a
      drill that hot-swaps content and the registry together shows no unresolved id in the client
      console.

## P5 — the consumers swap, and the workarounds die

- [ ] DELETE `id = N` from the corpus and the explicit-id law from the loader
      ([F1](forks.md#f1), [I9](issues.md#i9)) — only now, with the registry populated and
      authoritative. Acceptance: an `id` key is an unknown-field load error; the golden replays.
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
