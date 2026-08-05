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

## 2026-08-04 · P4 item 3 — the resolution seam (4/4, phase complete)

`Bundle::with_registry(map)` injects `name → kind_id`; `tile_def_id` / `thing_object_id` consult it
first and **fall back to the authored id** for a name it does not carry, so a partially seeded
registry degrades to today's behaviour rather than to nothing.

Deliberately a SEAM rather than a rewrite. Today it is a **no-op by construction** — the registry is
seeded from the same authored ids ([I9](issues.md#i9)), so injecting it cannot move an answer, and
the golden replays unchanged. Its value is entirely for [P5](todo.md): once `id = N` leaves the
corpus, the accessors keep their signatures and every caller keeps working, with resolution coming
from the table instead of the file. The test drives it with a deliberately *different* number (77
for grass) precisely because an equal one would prove nothing.

**P4 is complete.** The registry is populated by the master, served by the edge, resolved locally by
the client, and the loader has the seam it needs for the corpus to stop carrying numbers.

## 2026-08-05 · P5 item 1 — the edge resolves through the registry

`Worldgen::load_versioned_with` takes an optional `name → kind_id` map and injects it into the
`Bundle`. The edge builds that map from its live `index` subscription in `def_registry`, pairing
each registry row with the corpus def carrying the same taxonomy — which is
[F14](forks.md#f14)'s two hops (`name → tuple` authored, `tuple → id` from the table) collapsed into
the one lookup callers already make.

Loading is **two passes**, deliberately: the map is keyed by name and names come from the corpus, so
the edge loads once to learn the taxonomy, builds the map, and reloads with it injected. The corpus
is five small files, and it keeps `def_registry` a pure function of its two inputs instead of
something threaded through the loader.

Verified live in `edge-dev.log` — the same corpus version, loaded twice:

```
worldgen content loaded  dir=content tiles=6 definitions=0   version=8d572b28f7c17983
worldgen content loaded  dir=content tiles=6 definitions=17  version=8d572b28f7c17983
```

`definitions=17` is 6 tiles + 11 things bound from the table. An **unseeded index yields an empty
map and the edge keeps its bare load**, so a cold boot resolves from the corpus exactly as it does
today.

**Honest about what is NOT proven** ([I11](issues.md#i11)): the item also asked for a corpus-reorder
drill, and that test is vacuous while `id = N` is authored — the loader resolves from the authored
id, so a reorder renumbers nothing whether the registry is injected or not. It would pass for the
wrong reason. The drill moves to the item that deletes the seed, where the two answers finally
differ. What is proven here is that the map is built, injected, and (by unit test, with a
deliberately different number) actually consulted.

## 2026-08-05 · P5 item 2 — the client's wasm bundle resolves through the registry

`Content.withRegistry(isTile[], names[], kindIds[])` takes three parallel arrays — the cheapest
thing to hand across the wasm boundary — and the TS `DefinitionRegistry.bindTo` builds them by
pairing each registry row with the corpus def carrying the same taxonomy. Same shape as the edge's
`def_registry`, same reason: [F14](forks.md#f14)'s two hops (`name → tuple` authored,
`tuple → id` from the table) collapsed into the one lookup `tileDefId` already performs.

Three accessors had to reach JS for the binding: `thingNames`, `tileTaxonomy`, `thingTaxonomy` —
the last two flattened to `[type, subType, kind, variant]`, taking the first of each applicability
array, which is sufficient because a def's `kind_id` is shared across every tuple it covers.

Binding happens inside `swapTo`, so it re-runs on every hot-swap rather than only at boot, and it
is wrapped: a bind failure logs and the corpus's own ids answer.

Verified in the browser — `[content] 17 definitions bound from the registry`, world renders,
`tileDefId('grass')` → `1` and `tileDefId('wall_smooth')` → `6` through the injected map, with
`tileTaxonomy(6)` reading back `["biome-tile","default","smooth","wall"]`.

## 2026-08-05 · P5 item 3 — the npc resolves through the registry (all three injections done)

`fetch_corpus` now fetches `/definitions` alongside `/content` and binds it by name, the third and
last copy of the same pairing the edge and the webgl client do. Best-effort by design: an older
server with no `/definitions`, or an unseeded index, leaves the corpus's own ids answering — which
is today's behaviour, and the only sane fallback for a process that must keep driving wolves.

Live:

```
npc: definitions bound from the registry  definitions=17
npc::brains::wolves: wolf def + speed + needs resolved  def="0x30010070" speed=12 thirst_need=1
```

**Every consumer now injects the registry** — the edge (17), the webgl client (17), the npc (17) —
which is exactly the prerequisite [I10](issues.md#i10) identified before `id = N` can leave the
corpus. The next item is the actual cutover, and it is the first point at which the reorder drill
([I11](issues.md#i11)) means anything.

## 2026-08-05 · P5 items 4 + 5 — THE CUTOVER: the corpus stops carrying numbers

**17 `id = N` lines deleted** — 6 tiles, 11 things. `TileToml`/`ThingToml` lost the field, so a
corpus still carrying one now fails loudly (`unknown field 'id'`) rather than being ignored:
`deny_unknown_fields` doing the work the id law used to.

**Narrowed, not deleted** ([F15](forks.md#f15)). Materials, needs and conditions keep the explicit
id law untouched — their ids are stored data too (a `need_id` lives inside a payload word) but
nothing numbers them but the corpus, so removing theirs would leave them with no authority rather
than a better one. The id-law test moved to those kinds and still passes.

**The reorder drill, and it is decisive.** `dirt` authored FIRST in `tiles.toml`, so positional
resolution would give `dirt=1, grass=2`. The client, live in the browser:

```
corpus_order: ["dirt","grass","sand","water","stone","wall_smooth"]
grass → 1     dirt → 2     stone → 5     wall_smooth → 6
```

The ORIGINAL numbering, straight out of the registry, against a corpus that says otherwise. That is
the whole stream in one result: the corpus can be reordered freely and stored zones still mean what
they meant. (Order restored afterwards — the reorder was a drill, not a change.)

**The golden's re-bless diff is exactly the retired section and nothing else** — 19 lines removed,
every table, every stem, the 9,261-cell worldgen sweep and the needs probes byte-identical. The
cutover moved no data.

`name → kind_id` retired from the fixture with its reasoning ([F15](forks.md#f15)): it loads with no
registry, so it could only have pinned corpus ORDER while reading as identity — the third test this
stream has caught passing for that reason. **The guard moved from a fixture to a constraint**:
`index.definitions` has `id` as its primary key and refuses a second id for one (tuple, version),
so an existing numbering can never be contradicted. A fresh DB seeded from a reordered corpus does
get different numbers, correctly — a fresh DB is a fresh world.

Holes moved with the numbering: a retired tile used to be a skipped `id`, and now simply stops being
authored while the registry keeps its row, so the id is never handed to anything else ([F7](forks.md#f7)
leaves reclaim unbuilt).

**The client-rebuild lesson landed twice.** Removing `id` is a corpus schema change, so the
build-time wasm rejected the embedded TOML again until `rd build shared` ran — same black-screen as
when the taxonomy went in. Worth stating plainly: **any corpus schema change is a client-rebuild
event.**

Verified: 83 tests across the shared workspace (incl. the re-blessed golden), 5 `defs::`, 2
`def_fixture::`, and the live browser drill.

## 2026-08-05 · P5 items 6 + 7 — the stem-parsing and the species palette die

**The npc no longer reads taxonomy off a file path.** `resolve_thing_in` used to take the texture
stem, `split('/')`, `nth(1)`, and look the segment up in a code-owned palette — a definition's
identity derived from where its pictures live. It now takes the def **whole** from the registry,
whose id already carries type, species, kind and variant, because that is what the registry numbers.

That forced a shape change worth naming: the injection now carries the **full** `definition_reference`
rather than just the kind half. Dropping the type/subtype halves at bind time was precisely what had
made the stem-parsing necessary. `Bundle::definition_reference` hands the whole id back; the
`*_def_id` accessors extract the kind half from it.

**No registry is now a hard error for a pawn**, deliberately, and the fixture asserts both
directions: without one, `resolve_thing_in` fails naming the registry; with one, the wolf resolves
to `0x30010070`. A guessed subtype would be adopted and rendered wrong forever — the same stance the
stem-parsing took, for the same reason.

**The species palette is deleted**, and its own comment said when it could be: *"a content registry
can own these only once it guarantees append-only numbering."* It does — `index.definitions` never
renumbers a row and refuses to hand one id to two definitions.

Species ids move to **`content/subtypes.toml`** ([F16](forks.md#f16)), `animal = 1` and `human = 2`
carried over unchanged so every stored pawn def reads the same. The asymmetry is stated rather than
hidden: **a subtype id is authored on its own record where one exists** (a biome authors its own,
because a biome IS a record) **and in `subtypes.toml` where none does** (a species exists only as a
segment of other defs' taxonomies). Adding a species is now two lines of TOML and no code.

Live after both deletions: edge `definitions=17`, npc `def=0x30010070 speed 12 thirst 1`, and
`grep` finds no `split('/')` on a stem and no `pawn_species_subtype_id` anywhere. 85 tests green
across the shared workspace, 5 `defs::`, 2 `def_fixture::`.

## 2026-08-05 · P5 item 8 — the append-compat guard dies (phase complete)

`is_append_compatible_with` refused a hot-reload whose corpus reordered or removed a def, because
tile/thing ids came from corpus ORDER and a stored zone would be misread. Ids come from the registry
now and survive both, so the guard was protecting a fragility that no longer exists.

Deleted, and **proven by doing the thing it forbade**: a reordered `tiles.toml` hot-reloaded live —
`{"changed":true}` and `worldgen content hot-reloaded` — where the guard's message was
*"tile/thing ids changed (reorder or removal) — refusing hot-reload; restart to apply"*.

**A live bug found while deleting it.** `reload_content` called the BARE `load_versioned`, so every
hot-reload would have silently reverted the edge to positional resolution — the exact failure the
registry exists to prevent, reintroduced on a code path nobody would have thought to check. It now
does the same two-pass load as boot.

Its tests were replaced rather than dropped, because the property is still worth pinning — just the
opposite one:

- `a_reorder_keeps_every_id_when_the_registry_answers` — with `dirt` first and a registry saying
  `grass=1`, `grass` resolves to 1.
- `without_a_registry_a_reorder_renumbers_positionally` — the same reorder with no registry gives
  `grass=2`, so the first test cannot pass for the wrong reason.

**P5 is complete.** Every consumer injects, the corpus carries no numbers, and all three
workarounds the numbering had forced — the stem-parsing, the species palette, the append guard —
are gone. 20 edge tests green, 11 shared suites green.

## 2026-08-05 · P6 items 1 + 2 — versioning is live (item 3 blocked)

**The apple case works at the registry.** Live, against `index.definitions`:

- wolf speed 12 → 9 minted **v1 beside v0** — `805372016 v0` and `805372096 v1`, the old row
  untouched.
- an art-only recolour (`#ffffff` → `#ff0000`) minted **nothing** — still exactly two rows. [F12](forks.md#f12)'s
  boundary holding where it matters.
- the npc resolved the newest version live, so `max(version)` works end to end.

An existing entity keeping its old id needs no mechanism at all: [I7](issues.md#i7) established that
every store holds a NUMBER and nothing stores a name, so a stored row already means what it meant.
That is what makes [F6](forks.md#f6) free rather than a migration.

**[I12](issues.md#i12), found by the drill's CLEANUP rather than by any test.** Reverting speed
9 → 12 matched v0's fingerprint and resolved the wolf *back* to v0 — leaving `max(version)` (v1)
pointing at a definition the corpus no longer described. Only the NEWEST row may match: a revert is
a change, so 12 → 9 → 12 gives v0, v1, v2. Every unit test passed because they only ever drove one
transition.

**Two more live bugs on the way**, both on paths no test covered: the master had no subscription to
`definitions`, so `known_versions` read an empty cache and a data edit silently failed to bump; and
`reload_content` used the bare load, silently dropping the registry injection on every hot-reload.

**Item 3 is blocked on [B5](blockers.md#b5)** — and the finding is worth more than the item. With
v2 live the npc resolved the right id and then read **`speed=3 thirst=0`**: `kind_id` is both the
stored identity (which a bump must change) and an index into the corpus's positional per-def tables
(which must stay in range). v2's kind 13 against an 11-entry table falls through to defaults.
[I1](issues.md#i1) asserted the opposite and is corrected in place.

Dev restored: `index` republished and re-seeded, wolf back to `0x30010070 speed 12 thirst 1`, trips
flowing.

## 2026-08-05 · P7 — reclaim designed, not built (2/2)

[`index/intent/definition-reclaim.md`](../../components/server/spacetime/modules/index/intent/definition-reclaim.md)
records the sweep the user sketched, and the reason it stays unbuilt: **reclaim is the only
irreversible step in the design.** Everything else is additive — rows insert, never rewrite; a bad
allocation is refused at the reducer; a wrong version is a new row. Reuse is the one operation that
can make an existing, correct stored id start meaning something else, silently.

Six places an id can survive are tabled with how each fails. The one that turns a leak into
corruption is **`entity_state_log`** — append-only history, never cleaned: an event written when the
id meant a conifer replays as moss, so the world's *past* changes retroactively. A leaked id costs a
number; a wrongly reclaimed one costs the truth of the history.

Also recorded: what building it would need (a definition of "referenced" that includes history, a
liveness protocol where an unanswerable shard BLOCKS rather than being skipped, a quarantine tier
before reuse), and the cheaper first move if pressure ever arrives — retiring a deleted kind's whole
v0..vN lineage together, which is a far smaller claim than reclaiming one version of a live kind.

**The counter that makes it a measured decision** ships in the master, logged every boot:

```
definition kind-space  kind_ids_used=11  kind_ids_free=4085  bumps_on_record=0
```

11 of 4096 used at ~17 kinds. Reclaim gets built when that number says so, not on principle.

## 2026-08-05 · P6 rebuilt on B6/B7 — THE APPLE CASE, in the client (4/4)

The user's correction ([B6](blockers.md#b6)) turned the last phase into something smaller and
better. `version` is now an **authored** field ([F17](forks.md#f17)), the corpus retains every live
revision, and everything else fell out of that:

- **The fingerprint machinery is gone.** `version_for`, `KnownVersions`, `highest_kind_id` and the
  table's `sim` column — all deleted. An auto-derived bump *replaces* a definition; it cannot
  produce two coexisting ones, which is the whole point ([F12](forks.md#f12) superseded).
- **[B5](blockers.md#b5) dissolved, and the live numbers show it.** wolf v0 took `kind_id` **7** and
  v1 took **8** — both inside the corpus's positional range, because both are authored. The
  two-masters problem only ever existed because the master was allocating ids for definitions the
  corpus did not contain.
- **The duplicate check is the id law's successor**: sharing a taxonomy IS versioning; sharing a
  `(taxonomy, version)` pair gives one identity two definitions, and is refused.
- **The reverse lookup** landed both sides — `lookup(id)` client-side, `lookup_definition` in the
  module. Every row is indexed, not just the newest, because an OLD id is precisely the one whose
  meaning you need. It is also the prerequisite for
  [version predicates](../../components/server/spacetime/modules/index/intent/version-predicates.md).

**The apple case, run in the browser** with `wolf` authored at v0 (speed 12) and v1 (speed 4):

```
old_entity:            wolf v0 → speed 12
new_entity:            wolf v1 → speed 4
new_placements_get:    0x30010080          (v1 — the newest)
behaviours_differ:     true
one_definition_two_revisions: true
migration_run:         none
```

Two revisions of one definition, coexisting, behaving differently, with nothing swept. That is
[F6](forks.md#f6) — *"if all old apples are old apples and function like old apples"* — working
rather than promised. Corpus and dev registry restored afterwards; the drill was a drill.
