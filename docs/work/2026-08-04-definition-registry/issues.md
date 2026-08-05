# Issues — definition registry

_Measured on 2026-08-04, before planning, against the live tree._

## I1 — three hot paths unpack the def id's bits; one is a safety property {#i1}

This is what makes [F3](forks.md#f3) forced rather than chosen. Every site reads the packed fields
with no registry available:

| Site | Reads | Why it can't take a lookup |
|---|---|---|
| [`worker/main.rs:495`](../../../server/worker/src/main.rs) | `def_type_id` | routes `CREATE` to a type's shard arm and **rejects an unknown type BY NAME rather than silently pawning it** — inside the sim, per event |
| [`worker/main.rs:92`](../../../server/worker/src/main.rs) | `def_type_id` | `tics_for` branches on `TYPE_PAWN` to decide how to key the speed table |
| [`wasm/lib.rs:523-532`](../../../shared/wasm/src/lib.rs) | `def_kind_id`, `def_type_id`, `def_variant_id` | the per-cell render decode: namespace, visual-table index, subframe select — per cell, per zone |

**CORRECTED 2026-08-05.** This row originally claimed that nothing reads `def_kind_id` expecting a
stable meaning, so [F5](forks.md#f5) could burn kind ids freely. That was wrong — see
[B5](blockers.md#b5). The render decode does use
`kind_id` as an opaque table index, but an opaque index still has to be **in range**, and the
per-def tables are `Vec`s in corpus order. [F5](forks.md#f5) burning a fresh kind id per version
guarantees it eventually is not: the wolf's v2 landed on kind 13 against an 11-entry table, and
every lookup fell through to defaults. The live drill caught what the inventory had asserted away.

## I12 — matching ANY old fingerprint resurrects a stale version; only the newest may match {#i12}

_2026-08-05, P6. Found by reverting the drill, not by a test._

`version_for` matched the corpus's current fingerprint against **every** row on record and returned
that row's version. Bumping the wolf's speed 12 → 9 minted v1 correctly; reverting 9 → 12 then
matched **v0** and resolved the wolf back to v0's id.

That leaves the registry self-contradicting: `max(version)` — the rule every NAME lookup uses
([F6](forks.md#f6)) — answers **v1**, while the master says the corpus IS v0. A client placing a
wolf would get a definition describing behaviour the corpus no longer has.

**A revert is a change.** The corpus's current state must always be the HIGHEST version, so the
match has to be against the newest row alone:

- fingerprint == newest row's → unchanged, keep that version (the idempotent re-seed).
- anything else, INCLUDING an exact match on an older row → **bump**.

So speed 12 → 9 → 12 gives v0, v1, v2 — three rows, the third meaning the same as the first. That is
correct rather than wasteful: v1 may be on entities in the world, and "go back to how it was" is a
new decision, not the un-happening of an old one. It costs one kind id, out of 4096.

Worth noting how it surfaced: every unit test passed, because they only ever drove one bump. The
live drill's *cleanup* — restoring the corpus — is what produced the second transition.

## I11 — the "survives a reorder" test cannot be run until the seed is gone {#i11}

_2026-08-05, P5. An acceptance criterion that cannot mean what it says, yet._

Three P5 items ask for the same proof: reorder the corpus, confirm the consumer still resolves the
same ids. **That test is vacuous today.** While `id = N` is authored, the loader resolves from the
AUTHORED id, not from position — so a reorder does not renumber whether the registry is injected or
not, and the test passes for both the right and the wrong reason.

What CAN be proven now, and is:

- the map is **built and injected** — the edge logs `definitions=17` on its second load pass, versus
  `definitions=0` bare;
- injection **overrides** — `an_injected_registry_wins_and_falls_back` drives it with a deliberately
  different number (grass → 77), because an equal one proves nothing;
- an unlisted name **falls back**, so a partially seeded registry degrades to today's behaviour.

The reorder drill moves to the item that deletes the seed, where it becomes the actual acceptance:
after `id = N` is gone, a reorder either renumbers (positional fallback — the bug) or does not (the
registry — correct), and the two answers finally differ.

## I10 — after `id = N` dies, a corpus loaded WITHOUT the registry silently renumbers {#i10}

_2026-08-05, P5. Caught before deleting anything._

Deleting `id = N` does not break resolution — the loader's `id_of` walks the name cache, which is
built in corpus ORDER, so `tile_def_id("grass")` would still answer `1`. Today's authored ids
happen to equal corpus position exactly (tiles 1–6, things 1–11, no holes), so the golden would
replay and every test would pass.

**That is the problem.** It passes for the wrong reason, and it re-introduces precisely the
fragility [`toml-content` F1](../2026-08-04-toml-content/forks.md#f1) existed to prevent:

- Delete `conifer` from the corpus and every def after it shifts up one — *positionally*. The
  registry overrides for defs it knows, so the shift is invisible until a def the registry has
  never seen resolves to a number that belongs to something else.
- **Holes stop being expressible in the corpus.** F1's "a deleted def retires its id forever" was
  spelled `id = 4` skipped. With no ids there is no way to write a hole; retirement has to live in
  the registry (which keeps the row) instead. That is arguably better — but it means the CORPUS
  alone can no longer state the id law, and nothing in the loader will complain.
- The **golden fixture's `name → kind_id` section stops guarding anything**, because it loads the
  corpus with no registry injected. It would be asserting corpus order, dressed as identity.

And the prerequisite the plan omits entirely: **nothing calls `with_registry` yet.** The seam
landed in P4 item 3, but the edge's worldgen, the client's wasm `Bundle` and the npc all still
resolve through the corpus. Deleting the seed before wiring those three would leave the positional
fallback as the *only* authority everywhere — the exact opposite of this stream's point.

**So P5 item 1 is not next.** The order has to be: wire every consumer to inject the registry →
prove each resolves through it (not through position) → *then* delete `id = N` → and decide what
the golden's id section means afterwards. That last one is a real question, not a mechanical step:
a fixture that loads without a registry can only pin corpus order, so either it grows a registry to
load with, or that section retires and the registry's own uniqueness constraints become the guard.

Raised rather than resolved: it changes what P5 is, and the golden is the stream's safety net.

## I9 — P3 and P4 as written are a circular big-bang; the corpus `id` must become a SEED first {#i9}

_2026-08-04, P3. A plan-sequencing defect, caught before writing code._

P3 item 1 deletes `id = N` from the corpus. P4 item 2 re-points `Bundle`'s `tile_def_id` /
`thing_object_id` at the registry. **Each is broken without the other**: delete the ids and the
loader has no numbers until resolution moves; move resolution first and it resolves against a
registry nothing has populated. As written the two phases must land in one commit — a big-bang
cutover across the loader, the corpus, the master and every consumer, with the golden fixture
unable to run in between.

There is also a harder problem underneath. **`kind_id` must not change for any existing def**, or
every stored zone misreads — `tile_def_id("grass")` is `1` and there are zones full of `1`s. Today
those numbers come from the corpus. Tomorrow they come from the registry. Something has to carry
them across, and "allocate in corpus order and hope" is not a proof.

**Resolution — the corpus `id` becomes the allocation SEED, then dies.** Ordering:

1. Author the taxonomy **alongside** the existing `id = N`, which stays. Additive, nothing breaks,
   the golden fixture keeps passing throughout.
2. The master expands the cross-product and calls `ensure_definition` with ids composed **from the
   authored seed**. A fresh DB therefore reproduces today's numbering exactly — and *provably*,
   because P0's golden pins every one of them.
3. Only once the registry is populated and resolution reads from it does `id = N` leave the corpus.
   By then the numbers live in a table that persists across reorders, which is what
   [F1](forks.md#f1) wanted all along.

The seed is not a compromise: it is the migration proof. A registry seeded from the authored ids
is verifiably the same world; a registry allocated from scratch is a hope. P3 and P4 are re-planned
to this order, and the `id` deletion moves to [P5](todo.md) where it belongs — after the thing that
replaces it is real.

## I8 — the registry cannot COMPOSE an id; it can only record one {#i8}

_2026-08-04, P2. A plan gap, found on contact with the module._

The plan wrote the reducer as `ensure(type, subType, kind, variant, version) → id` — the module
composing the number. It cannot, because three of the four coordinates are not the module's to know:

| Coordinate | Where its number comes from today | Can the module see it? |
|---|---|---|
| `type_id` | the code palette in `shared/codec` (`TYPE_BIOME_TILE = 1`, …) | yes — it deps the codec |
| `subtype_id` | **AUTHORED IN THE CORPUS** — `biomes.toml` writes `subtype = 6` for forest; pawn species come from the codec palette | **no** — the module never reads `content/` |
| `kind_id` | corpus order today; the registry's job to allocate | it can allocate |
| `variant_id` | **CHOSEN AT PLACEMENT, not per def** — `worldgen.rs:127` packs `(seed >> 13) & 0x0F`, a random art variation per cell; pawns always pack 0 | no — it is not a per-def fact |

So the registry's real job is narrower than "allocate the id": it **records** the tuple → id
mapping durably and uniquely, while COMPOSITION stays where the corpus is readable. That is also the
better split — the module has no business knowing that `forest` means 6, and a module that had to
would need the corpus mounted into a WASM reducer.

Recorded as a [deviation](deviations.md) rather than silently re-scoped: the reducer takes the
composed `id` and the four strings, and enforces uniqueness and collision-detection. The caller
(master, at content load) does the composing.

Note the variant row above, because it matters for [F2](forks.md#f2)'s cross-product: a `variant`
array of `[0..15]` mints 16 ROWS so all 16 are resolvable, but nothing *assigns* a cell a variant —
worldgen rolls one. The rows exist so the roll always lands on a registered definition.

## I7 — where a def id is STORED {#i7}

_2026-08-04, P0 item 3. Every place a number allocated by the registry comes to rest._

| Store | Field | Width | Holds |
|---|---|---|---|
| `tile` module — dense rows | `DenseItem.kind_reference` | u16 | `kind_id:12 \| variant_id:4` — one per tile of a zone's baseline |
| `tile` module — overlay rows | `OverlayItem.kind_reference` | u16 | same, per overridden cell |
| `thing` module — dense + overlay | `OverlayItem.kind_reference` | u16 | same, plus `data` |
| `pawn` module | `definition_reference` | u32 | the FULL packed def — `spawn()`'s operand |
| payload opcode stream | `PART slot definition_reference` | u32 | a pawn part slot's full def ([`payload.rs:33`](../../../shared/codec/src/payload.rs)) |
| cold row header | `subtype_id` | — | the type half's other coordinate, supplied at relay ([I6](#i6)) |

Two shapes, and the split matters for [P4](todo.md):

- **Tiles and things store only the KIND HALF** (`kind_id | variant`). Their `type` is implied by
  which module the row lives in and their `subtype` comes from the row header, which is exactly why
  the relay can compose a full def without consulting anything ([I6](#i6)).
- **Pawns store the WHOLE def**, in two places — the `pawn` table and the payload's `PART` entries.
  These are the rows that carry a species nibble, and therefore the rows that P5's change to species
  resolution must not disturb. The `def_fixture` test in `client/npc` pins their current values.

Nothing stores a def NAME. Every store is a number, which is what makes
[F6](forks.md#f6)'s "old objects keep old ids forever" free: the stored row already means what it
meant, and the registry never has to rewrite one.

## I6 — the READ path never needs the registry; only allocation and name→id do {#i6}

_2026-08-04, P0 item 1. The full pack/unpack inventory, and the finding that reshapes P4._

**Every site that touches a `definition_reference`:**

| Site | Direction | What it does |
|---|---|---|
| [`edge/ws.rs:479`](../../../server/edge/src/ws.rs) | PACK | `pack_definition_reference(pack_type_reference(TYPE_BIOME_TILE, row.subtype_id), it.kind_reference)` — relaying a cold tile row |
| [`edge/ws.rs:508`](../../../server/edge/src/ws.rs) | PACK | the same for `TYPE_BIOME_THING` |
| [`npc/lib.rs:257`](../../../client/npc/src/lib.rs) | PACK | `pack_definition_from_ids(TYPE_PAWN, species, kind, 0)` — the stem-parsing site P5 deletes |
| [`worker/main.rs:92`](../../../server/worker/src/main.rs) | UNPACK | `def_type_id` → key the speed table |
| [`worker/main.rs:495`](../../../server/worker/src/main.rs) | UNPACK | `def_type_id` → route `CREATE`, reject unknown types |
| [`wasm/lib.rs:523-532`](../../../shared/wasm/src/lib.rs) | UNPACK | `def_kind_id`/`def_type_id`/`def_variant_id` → per-cell render decode |
| [`wasm/lib.rs:561`](../../../shared/wasm/src/lib.rs) | UNPACK | `def_type_id` in `cold_cell` |

No site outside [I1](#i1) needed investigating, and **`def_subtype_id` has no caller at all** —
consistent with [I2](#i2).

**The finding.** Look at what the two relay sites compose from: a code constant (`TYPE_BIOME_TILE`),
the **cold row's own header** (`row.subtype_id`), and the **stored** `kind_reference`
(`kind_id:12 | variant_id:4`, written by `worldgen.rs:126/132` and `worker/main.rs:612`). The def is
*assembled from fields that are already in the row* — no name is resolved and no table is consulted.

So the registry is needed at exactly two moments:

1. **Allocation** — content load, when a tuple first needs a number ([P2](todo.md)/[P3](todo.md)).
2. **Name → id resolution** — worldgen picking a tile/thing by name, the npc resolving `"wolf"`, a
   client placing by name ([P4](todo.md)).

It is **not** needed on the read path, the relay path, or the render path. That materially lowers
P4's risk: re-pointing `tile_def_id`/`thing_object_id` touches resolution only, and every stored id
keeps decoding exactly as it does today — which is also what makes [F13](forks.md#f13)'s frozen
layout free rather than constraining.

## I2 — subtype IS decoded at runtime — from the COLD ROW, not from the def {#i2}

_Corrected 2026-08-04 after the user challenged the first wording, which was misleadingly broad._

The narrow, accurate fact: **`def_subtype_id` — the copy of subtype inside a `definition_reference`
— has no runtime reader.** The npc packs species into it via `pack_definition_from_ids` and nothing
unpacks it.

But subtype itself is very much live. It is read from the **cold row header** (`cold_row_subtype`)
at four sites in the worker ([main.rs:380/395/431/450](../../../server/worker/src/main.rs)), feeding
`subtype_id:` into shard writes, and it is *written* from content in two places —
`biome_subtype_id` for a cell's biome ([worldgen.rs:119](../../../server/edge/src/worldgen.rs)) and
`pawn_species_subtype_id` for a pawn's species.

So the earlier framing — "12 bits written and never decoded, possibly free space" — described one
accessor and implied the axis. It is the def's *copy* that is currently redundant with the row, not
the concept. And the user's objection stands independently of today's call sites: `pawn/human` and
`pawn/animal` are a real distinction, and nothing can ask it without enumerating kinds unless the
def carries it. **Subtype stays in the def** ([F10](forks.md#f10)).

## I3 — 16 variants per kind is the ceiling, and the wolf holds 15 {#i3}

From the live art manifest:

```
pawn/animal/wolf var_variant:
  [1, 124, 125, 555, 777, 888, 7001, 7002, 8101, 8102, 8201, 8202, 8301, 8302, 1682748910]   → 15
```

Those are the art tree's variant **labels**, not slots — one is ten digits. The registry handles
that: the label lives as `string variant` in the row, the id carries the u4 slot.

`variant_id` is u4, so **16 variants per (type, subType, kind)**, and the wolf has one slot left.
Recorded as a constraint the allocator enforces ([P2](todo.md)), not as a problem to solve — the
packed layout does not change ([B2](blockers.md#b2)). A kind that outgrows 16 splits into more kinds.

## I4 — the taxonomy IS uniform; the stems are prefixes of varying depth {#i4}

_Corrected 2026-08-04. The original row claimed the stems were irregular and needed a ruling. They
are not, and it did not — I was reading the corpus without the art tree beside it._

`texture` in the corpus is the `type/subType/kind` PREFIX; the variant (and then the direction) are
appended when the stem is resolved against the art tree, which is why the live manifest holds
`biome-thing/default/conifer/0/e` while the corpus holds `biome-thing/default/conifer`.

So `biome-tile/default/smooth/wall` is not a fourth kind of thing: it is
`type=biome-tile, subType=default, kind=smooth, variant=wall` — a def that pins its variant because
a linked autotile's forms (`wall`/`fence`/`rock`) sit exactly where numeric variants sit. Uniform,
four axes, no exception.

`white` is likewise not a taxonomy question. It is the built-in no-texture fill —
`loader.rs:85` calls it the built-in `"white"` fill, the client compares `stem === "white"` to skip
linked resolution, `def_span.py` skips it as "not a texture tree path", and **no `white` file exists
anywhere under `textures/`**. It is already the marker for "this def has no art", which is what my
proposed `art = none` would have renamed it to. Orthogonal to the taxonomy; nothing to decide.

## I5 — the registry must be GLOBAL, but shards are per-region {#i5}

`definition_reference`s appear in every shard's stored rows, so the registry is world-global by
definition. The existing per-env global DB is `index` (routing: `servers`, `shards`,
`region_shards`, `player_servers`), seeded from `deploy/servers/<env>` and read by the gateway and
the world server. A per-region `data_shard` cannot host it.

**Resolved 2026-08-04 by the user** ([F11](forks.md#f11)): the registry lives in `index` and the
**master** allocates, being the single one. My objection — that `index` is a routing directory and
content identity widens it — was rebutted on the facts: the modules are split for **scaling**
(data shards can grow independently), not as a lifecycle division, and per-env global data has no
reason not to share a database. A sibling module stays available if the registry later grows its own
reducers, exactly as `chat`/`players` sit beside `index` today.
