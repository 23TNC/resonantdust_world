# Blockers — definition registry

_Things that genuinely need the user. A decision I can make is a [fork](forks.md), not a blocker.
Every row here is a call whose answer changes the SCHEMA or the WIRE LAYOUT — get them wrong and the
cost is stored data, not a refactor._

## B1 — what are the four-segment stems, and what is `white`? {#b1}

_Opened 2026-08-04. Blocks P1 (the schema)._

The taxonomy fields cannot be authoritative until every current def has a spelling
([I4](issues.md#i4)):

- **`biome-tile/default/smooth/wall`** — four segments. Either `kind = "smooth"` with `wall` a named
  variant, or `subType = "default/smooth"` as a two-level subType, or the tree is wrong and it should
  be `biome-tile/default/wall/smooth`. Each reads differently and each mints different ids.
- **`white`** — not a path. It means "no art, flat tint", used by shrub, cactus, both torches, and
  the wall blueprint. Options: a reserved `kind = "white"` under some type; a `texture` field that
  stays optional and orthogonal to the taxonomy; or an explicit `art = none`.

**Why it needs you**: this is authoring semantics — what the taxonomy *means* to whoever writes
content, not a mechanical consequence. A wrong answer here is re-authoring the corpus later.

**My recommendation**: `white` becomes an absence (`art = none`), because it is a rendering fallback
rather than a taxon; and the four-segment case gets resolved by deciding whether a "form"
(`wall`/`fence`/`rock`) is a **variant** or a **kind** — I lean variant, since the linked-atlas forms
already sit where numeric variants sit in the art tree.

## B2 — does the def id keep `subtype`, and does `variant` widen? {#b2}

_Opened 2026-08-04. Blocks P1 (the schema); irreversible once ids are stored._

Two facts that only matter together:

- `def_subtype_id` has **zero runtime readers** — 12 bits written and never decoded
  ([I2](issues.md#i2)).
- `variant_id` is **u4 = 16 slots, and the wolf holds 15 today** ([I3](issues.md#i3)).

So the layout has 12 apparently-dead bits and one nearly-full field, and the registry is the moment
to decide. Options:

1. **Keep `type:4 | subtype:12 | kind:12 | variant:4`.** Nothing moves; the variant ceiling stays
   one wolf variant from being hit.
2. **Drop subtype from the def; re-spend the bits.** e.g. `type:4 | kind:14 | variant:14`, or
   `type:4 | kind:20 | variant:8`. Subtype still exists in the *registry row* and in the cold row
   header (`type_reference = type_id:4 | subtype_id:12`), just not in the def. Costs a codec layout
   change and a re-stamp of stored defs.
3. **Keep subtype, widen variant by narrowing subtype** — e.g. `subtype:8 | variant:8`.

**Why it needs you**: this is the wire layout for stored data. Option 2 is the one I would take on
the merits — but "12 bits appear unused" is an argument for *asking whether they are needed*, not for
assuming they never will be, and the answer depends on where you intend subtype to go.

## B3 — what counts as a change that bumps `version`? {#b3}

_Opened 2026-08-04. Blocks P5 (versioning live)._

[F4](forks.md#f4) says any change bumps. That needs a boundary:

- **Data change** (weight, expiry, speed, footprint) — unambiguously a bump: this is the apple case.
- **Art re-master** (same def, new pixels) — probably NOT. The texture manifest already hashes art
  per stem for cache-busting, so a re-master propagates without touching identity. Bumping here
  would mint an id per art tweak and burn kind space for nothing.
- **Purely cosmetic corpus edits** (a tint, a comment, reordering) — needs a rule, or every editorial
  pass mints ids.

**Why it needs you**: it is a content-workflow policy. Too eager and the id space churns on
cosmetics; too lazy and two objects that behave differently share an id, which is the exact failure
[F6](forks.md#f6) exists to prevent.

**My recommendation**: bump on a change to any field the **simulation** reads (the data half); do not
bump on art, tint, or comments. That makes "same id ⇒ same behaviour" the invariant, which is what
the old-apple policy actually needs.

## B4 — where does the registry live, and who allocates? {#b4}

_Opened 2026-08-04. Blocks P2 (the table + allocator)._

The registry is world-global; shards are per-region, so a `data_shard` cannot host it
([I5](issues.md#i5)). Two coupled questions:

**Placement.** `index` is the only existing per-env global DB — but it is a *routing* directory
(`servers`, `shards`, `region_shards`, `player_servers`), and putting content identity in it widens
what the module means. The alternative is a new `definitions` module.

**Allocation.** Whoever allocates must be a **single writer**, or two world servers booting together
race and mint two ids for one tuple. Options: the edge that wins a lease; a reducer that is
idempotent per tuple (insert-if-absent, returning the existing id); or a deterministic function of
the taxonomy (no allocator at all — but that forecloses [F7](forks.md#f7)'s reclaim forever and makes
a version bump un-representable, so I would not).

**Why it needs you**: a new module is a deployment surface — its own DB, publish step, bindings, and
`rd redeploy` unit.

**My recommendation**: a new `definitions` module with an idempotent `ensure(tuple) → id` reducer.
Idempotence beats leasing because it needs no coordination and is safe to call from every server on
every boot; a new module beats overloading `index` because "where servers are" and "what things are"
have no reason to share a lifetime.
