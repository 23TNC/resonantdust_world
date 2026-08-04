# Completed — definition registry

_Dated evidence: what landed and how it was checked. Append chronologically._

_Nothing yet — the stream is planned, not started. P1 and P2 are gated on
[`blockers.md`](blockers.md)._

## 2026-08-04 · P0 — freeze what a def id means today (3/3)

**The inventory turned up the finding that reshapes P4** ([I6](issues.md#i6)). Seven sites touch a
`definition_reference` — three pack, four unpack — and no site outside the pre-planning measurement
needed investigating. But look at what the two relay sites compose from
([`ws.rs:479`](../../../server/edge/src/ws.rs), `:508`): a code constant, the **cold row's own
`subtype_id`**, and the **stored `kind_reference`**. The def is assembled from fields already in the
row; no name is resolved and no table is consulted.

So the registry is needed at exactly two moments — allocation, and name→id resolution. Not on the
read path, not on the relay path, not on the render path. P4 re-points resolution only, and every
stored id keeps decoding exactly as it does today.

**The oracle split along the crate boundary**, because `shared/content` deliberately has no codec
dependency and adding one for a fixture would couple them:

- `name → kind_id` for tiles and things, as explicit pairs in the golden corpus (blessed; the
  registries already pinned it *implicitly* as line numbers, which is a reading convention, not an
  assertion).
- the packed composition in a new `npc::def_fixture` module — where codec lives, and where the
  composition actually happens. `wolf → 0x30010070` (the value the live npc logs on every boot),
  `human_female → 0x300200A0`, `human_male → 0x300200B0`, with speeds.

The second `def_fixture` test deliberately asserts the coupling P5 will **remove** — that a pawn's
species nibble equals `pawn_species_subtype_id` of the texture stem's second segment. Pinning it now
makes its removal a visible, deliberate change rather than a silent one; a re-subtyped pawn would be
adopted and rendered wrong forever.

**Stored ids** ([I7](issues.md#i7)): 5 stores in two shapes. Tiles and things keep only the KIND
HALF (`kind_id:12 | variant_id:4`) — their type is implied by the module and their subtype comes from
the row header. Pawns store the WHOLE def, in the `pawn` table and again in the payload's `PART`
entries. **Nothing stores a name**, which is what makes [F6](forks.md#f6) free: a stored row already
means what it meant, and the registry never has to rewrite one.

Verified: 12 content tests + the blessed golden green in the shared workspace; both `def_fixture`
tests green in `client/npc`.

## 2026-08-04 · P1 — the schema, documented before parsed (3/3)

`VARIABLES.md` § TOML content schema now opens with **the corpus describes; the server numbers**: a
table of the four taxonomy fields, `subType`/`variant` as APPLICABILITY ARRAYS with the no-wildcards
rule spelled out, the taxonomy-is-the-texture-path derivation, and the versioning policy. The old id
law is marked superseded in place rather than deleted, with the reason — it was correct while the
*loader* owned identity.

**The corpus forced a fork the plan hadn't anticipated** ([F14](forks.md#f14)): `name` and `kind`
are not the same string. `tree` is `biome-thing/default/conifer`, `human_female` is
`pawn/human/female`, `wall_smooth` is `biome-tile/default/smooth/wall`, and `reed` has no art at
all. `name` is a readable flattening of the tuple and it is what everything already asks for —
`biomes.toml` scatters `thing = "reed"`, the npc calls `resolve_thing("wolf")`. So `name` stays in
the corpus, the registry keys on the four strings and carries no name column (the table the user
specified), and resolution is two local hops: `name → tuple` from the corpus, `tuple → id` from the
registry. No caller changes signature and `biomes.toml` does not move.

`TABLES.md` gains `index` § `definitions` — `id` PK, `version`, the four taxonomy strings, one btree
over the tuple and a uniq over tuple+version. Master is the sole writer ([F11](forks.md#f11)); old
rows are never deleted or rewritten, and reclaim is documented as not-built.

**A doc conflict, caught and resolved rather than papered over.** VARIABLES already said extra
on-disk variants "truncate out of the manifest", while this stream's plan said the allocator must
refuse a 17th. Both are right about different things, so the doc now separates them explicitly:
extra **art** truncates (existing behaviour, by design — the art tree may hold more folders than the
id can address), an extra **authored** variant is a load error (wrapping would alias two definitions
onto one id). `pawn/animal/wolf` is named as the kind sitting one slot from the ceiling.

Verified: `bin/rd docs-check` clean across all three edits.

## 2026-08-04 · P2 — the registry table + allocator (3/3)

**`index.definitions` is live**, not just compiled. Published to `resonantdust-dev-index-0` and
exercised through the CLI:

- `ensure_definition(0x3000fe70, 0, biome-thing, default, conifer, "4")` → one row.
- the identical call again → still one row (the idempotent path, so every server may call it on
  every boot without coordination — belt-and-braces behind [F11](forks.md#f11)'s single master).
- the same id claimed by `moss` → **rejected by name**:
  `definition id 0x3000fe70 collision: registered as biome-thing/default/conifer/4 v0, now claimed
  by biome-thing/default/moss/4 v0`.
- a v1 of the same tuple with a new id → **two rows**, v0 untouched. That is [F6](forks.md#f6)
  working: the old definition keeps existing for the entities that hold it.

**A plan gap, found on contact and recorded rather than quietly re-scoped** ([I8](issues.md#i8),
[deviations.md](deviations.md)). The plan had the reducer composing the id. It cannot: `subtype_id`
is AUTHORED in `biomes.toml` (forest is 6) and the module never reads `content/`, and `variant_id`
is chosen at PLACEMENT — `worldgen.rs:127` rolls `(seed >> 13) & 0x0F` per cell — so it is not a
per-def fact at all. Composition moved to `server/master`, where F11 already put the allocator and
where the corpus is loaded. The module records, enforces uniqueness, and detects collisions.

`master/src/defs.rs` composes in the FROZEN layout and refuses rather than wraps, with three named
`AllocError` arms. Two of the four tests are the load-bearing ones:

- a conifer version bump takes a new `kind_id` while the `rock` beside it in the same subType stays
  **bit-identical**, and the subType/variant nibbles never move — [F5](forks.md#f5)'s isolation.
- a 17th variant is refused, and the test **demonstrates why** instead of asserting it away: the
  packer masks, so slot 16 silently becomes variant 0 and the 17th definition would alias onto the
  1st. My first version of that assertion was backwards and the failure is what surfaced it.

Two `pub const`s added to the codec (`KIND_ID_LIMIT`, `VARIANT_ID_LIMIT`), derived from the existing
masks so they cannot drift. They name the ceiling the layout already has — no field added, none
moved ([F13](forks.md#f13) holds).

`bin/sim` gains `test`, since these crates are SDK-client binaries with no lib target and their
tests live in `#[cfg(test)]` modules inside the binary.

Verified: index module builds + publishes; 4 `defs::` tests green via `bin/sim test master`.
