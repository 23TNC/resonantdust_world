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

## 2026-08-04 · P3 — the taxonomy, additive (4/4)

**The taxonomy is authored on all 19 defs and moved nothing.** `type`/`kind` scalars plus
`subType`/`variant` arrays on 6 tiles and 11 things, with `id = N` left in place as the allocation
seed ([I9](issues.md#i9)). The golden fixture passes **byte-identical** — which is the whole point
of doing it additively: every step of this phase was verifiable against a fixture that never had to
be re-blessed.

**The stem now derives, and the derivation has a rule worth stating.** Six authored `texture` lines
are deleted (conifer, flora, wolf, female ×2, male ×2, wall) and the golden still matches, so the
derived stems are identical to the authored ones for all 19 defs. The rule the corpus forced:

- a **named** variant is part of the address — `biome-tile/default/smooth/wall`, because a linked
  object's form names a distinct art folder;
- a **numeric** variant is not — `biome-thing/default/conifer`, because it is an index worldgen
  rolls per cell and the resolver appends at draw time (`/4/e`).

That is exactly the linked-vs-plain split `VARIABLES.md` already draws for `variant_id`; the loader
now expresses it in one place. `texture = "white"` survives untouched as the no-art fill — an
authored `texture` still wins over the derivation, which is what keeps it working.

A **half-authored** taxonomy is a load error, deliberately: absent entirely is fine while the field
is additive, but `type` without `kind`, or an empty `subType`, would mint the wrong registry rows
silently — the one failure the registry exists to prevent.

**The migration proof landed** ([`master/src/defs.rs::allocations`](../../../server/master/src/defs.rs)).
Seeded from the corpus's authored ids, the expansion reproduces exactly the numbering the world
already stores:

- `wolf/animal/0` → **`0x30010070`**, the value the live npc logs on every boot and that
  `npc::def_fixture` pins independently.
- `female/human/0` → `0x300200A0`, `male/human/0` → `0x300200B0`.
- `grass` → `kind_id` 1 under `TYPE_BIOME_TILE` — the number every stored zone is full of.
- `smooth/default/wall` → `kind_id` 6, variant slot 0 (a named form is its kind's only variant,
  which is why it lives in the stem instead).
- `conifer` expands to **16 rows sharing one `kind_id`**, differing only in the variant nibble — so
  worldgen's per-cell roll always lands on a registered definition.
- and no id is allocated twice, checked by dedup, because a duplicate would mean two definitions
  aliased onto one number.

Two `AllocError` arms added for the expansion's own failure modes: an unknown `type` (the palette is
code-owned and structural) and an unknown `subType` (an unauthored biome, or a species outside the
palette). Neither invents a number.

Verified: 15 `shared/content` tests + the unchanged golden; 5 `defs::` tests via `bin/sim test
master`.

## 2026-08-04 · P4 item 1 — the master seeds the registry at boot

**182 definitions, live, on a wiped table.** `master` loads the corpus at startup, expands the
cross-product, and calls `ensure_definition` per allocation. The count is exactly what the corpus
implies: 5 ground tiles × 16 variants + `wall_smooth` = 81 tiles, 6 flora × 16 + 5 singletons = 101
things.

The ids are the ones the world already stores, read back out of SpacetimeDB rather than asserted in
a test: `wolf/animal/0` → `805372016` = **`0x30010070`**, and `smooth/default/wall` → `268435552` =
`0x10000060` (type 1, kind 6, variant slot 0 — `wall_smooth` is tile id 6, and its named form takes
slot 0 because it lives in the stem instead).

Seeding is **non-fatal**, matching every other upstream in the master: a down index or an unreadable
corpus logs and the metronome still starts. That is safe precisely because of
[I6](issues.md#i6) — the read path never consults the registry, so an unseeded registry costs new
name resolution, never stored data.

**A gap the live test exposed, and it was the important kind.** The P2 reducer enforced uniqueness
on `id` alone, but `TABLES.md` also specifies `uniq` over `(tuple, version)` — and the two leftover
test rows made the consequence concrete: two different ids for one tuple+version would BOTH satisfy
`max(version)` and resolution would pick arbitrarily. The same aliasing hazard as a duplicate id,
wearing the other hat. The reducer now rejects it by name, and the doc and the code agree again.

Tooling: `server/st-bindings` had **no regeneration path** in `rd` — `rd build spacetime <mod>`
writes the EDGE's bindings only (the compose service mounts `../edge` as `game-server`), which is
why st-bindings had drifted to 23 files against the edge's 31. Regenerated by hand for `index` to
unblock; the gap is real and worth closing separately.

## 2026-08-04 · P4 items 2 + 4 — the client resolves locally

`GET /definitions` serves the registry straight off the edge's live `index` subscription — the
`definitions` table rides the SAME subscription as the routing rows, so there is one apply barrier
and the edge is never half-ready with routes but no defs. Rows are sorted by id so the payload is
byte-stable for a given table state.

Client-side, `DefinitionRegistry` holds `tuple → id` with **highest-version-wins**
([F6](forks.md#f6) in one line) and an unknown tuple returning `null` rather than a guessed number.

**Verified in the browser, not just by curl** — 182 rows in the client, resolving to exactly the
ids the world stores:

```
wolf   pawn/animal/wolf/0            → 0x30010070
grass  biome-tile/default/grass/0    → 0x10000010
wall   biome-tile/default/smooth/wall→ 0x10000060
tree   biome-thing/default/conifer/4 → 0x20000014
dragon (unknown)                     → null
```

**Ordering** ([P4](todo.md) item 4): the registry loads BEFORE the corpus swaps in, on both the boot
path and the hot-reload path, so a client is never holding content whose ids it cannot resolve. A
registry failure is non-fatal — the corpus still boots and every STORED id still decodes from its
own bits ([I6](issues.md#i6)); only NAME resolution degrades until the next fetch.

**The incident worth recording: the client black-screened, and it was the corpus schema change, not
the registry.** Adding `type`/`kind`/`subType`/`variant` to the corpus means the client's build-time
wasm — which parses the embedded TOML with `deny_unknown_fields` — rejects the new keys until
`rd build shared` runs. Boot failed with `unknown field 'type'` on both files. This is the same
class of incident `toml-content` P6 hit (the embed black-screened until it embedded the four
client TOMLs) and it generalises: **a corpus schema change is a client-rebuild event**, because the
schema lives in wasm and the corpus is embedded at build time.

(Two unrelated environment things also had to come back up mid-verification: the vite dev server and
the gateway had both stopped, which is what made login fail with `gateway request failed` before the
registry could load. Neither was caused by this work.)
