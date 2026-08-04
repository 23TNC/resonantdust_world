# Forks — TOML content

## F1 — ids are EXPLICIT in TOML {#f1}

The `.rd` loader assigns ids by FIRST-APPEARANCE order with an append-only discipline — a
convention held by comments ("appended last so ids don't renumber"). Those ids are STORED DATA:
tile `kind_reference`s in zones, thing `object_id`s, packed pawn defs, `need_id`s inside payload
words. TOML table order is an editing accident waiting to renumber the world.

**Chosen**: every def authors `id = N` (1-based; `0` reserved); the loader REFUSES a duplicate,
a missing id, or id 0 — load errors, not warnings. Holes are legal (a deleted def retires its id
forever). Rejected: order-derived ids (the current fragility, made worse by TOML's reorderable
tables); a separate id-map file (two files describing one fact). The biome `@subtype` already
proved this shape — it went explicit the day stored zones carried it.

## F2 — the Bundle API is the preservation seam {#f2}

Consumers (wasm `Content`, npc, edge worldgen + `/content`, worker speeds) keep the exact
accessor surface — `thing_layout()`, `thing_subframe()`, `need_params_all()`, `generate()`, all
of it. Only the loader entry (`load(sources)`) changes what it parses. Rejected: redesigning the
runtime tables while migrating — two moves in one diff is how equivalence becomes unprovable.
This seam is also the user's "for now": a future scripting layer is just another loader.

## F3 — biomes become declarative RULES {#f3}

The one behavioral DSL surface is a fixed shape: a conjunction of threshold tests on the sampled
dimensions, then a tile + ordered salted scatter (last write wins). Encoded as data:

```toml
[[biome]]
name = "wetland"
subtype = 4
# every listed dimension must pass; keys mirror the .rd ops EXACTLY (gte/lt/lte half-opens)
when = { humidity = { gte = 0.60 }, elevation = { lt = 0.46 } }
tile = "dirt"
scatter = [ { salt = 3, p = 0.25, thing = "reed" } ]   # ordered, least→most dominant
```

Array order in the ONE biome file = evaluation priority (first match wins), exactly today's
file-order rule. A rust classifier evaluates rules; `generate()`/`GenTile` keep their signatures
(F2). Acceptance is a golden worldgen grid — bit-identical tiles/things/subtypes. Rejected:
keeping the VM just for biomes (the user said phase out; the shape doesn't need it) and
hardcoding the biomes in rust (they are content and must stay tunable without a rebuild).

## F4 — the crate renames to `shared/content`, LAST {#f4}

"dsl" becomes a lie the moment the parser dies, and authoritative names hold current truth only.
But the rename touches every consumer Cargo, the docker bind mounts, and `bin/rd` — pure churn
that would bury the load-bearing diffs. **Chosen**: land loader + corpus + swaps under the
existing crate name, then rename in the deletion phase as its own mechanical commit. Rejected:
renaming first (drowns review), never renaming (docs-authority says names may not lie).

## F5 — the golden gate: nothing swaps until byte-identical {#f5}

A fixture dumped from the `.rd` corpus — every registry (names + ids), every flat table
(layout/subframe/light/speed/packed/lanes/heights/needs/moodlets/materials), a worldgen sample
grid, and the needs-eval probe values — committed to the repo. The TOML loader must reproduce it
EXACTLY before any consumer changes, and the old loader + corpus die one commit after the gate
passes (both-loaders is a state this stream passes THROUGH, never a resting point —
delete-don't-deprecate). Rejected: eyeballing the world as the only check (subframe drift taught
that lesson at I7/I8 cost).

## F6 — one record per def; the facet split dies with the hooks {#f6}

`:data` / `:visual` facets existed so two files could author one def's hooks. In TOML a def is
ONE table carrying all its fields — "our objects are almost entirely data definitions" said it
plainly. File layout stays by CATEGORY (`content/things.toml`, `tiles.toml`, `biomes.toml`,
`materials.toml`, `needs.toml`) so diffs stay reviewable; nothing splits one def across files.
Rejected: mirroring the data/visual file split (an artifact of hook merging, meaningless for
data), one-file-per-def (churn for a ~40-def corpus).
