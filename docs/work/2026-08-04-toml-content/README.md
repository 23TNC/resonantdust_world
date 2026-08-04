# TOML content — the DSL retires, objects become data — 2026-08-04

_Components: `shared/dsl` (→ `shared/content`, F4), `content/`, `server/edge` (worldgen +
`/content`), `server/worker`, `client/npc`, `shared/wasm`, `bin/`. Plan in [`todo.md`](todo.md);
decisions in [`forks.md`](forks.md)._

## The user's call

> "We are going to phase out our DSL for now, and implement our objects using toml. Our game is
> not benefiting from the full DSL and it doesn't seem to be required for our new game design.
> Our objects are almost entirely data definitions." — 2026-08-04

## What the DSL is actually used for (measured, 2026-08-04)

The whole corpus is ~1,030 lines across 8 `.rd` files, and it exercises exactly FOUR features:

1. **Constants** — `&path set` into slots (`@define`/`@on_create` writing tints, layouts,
   subframes, speeds, lights, packed channels, material params, need bands). Pure data.
2. **`^prim call`** — visual part construction (the human's body+head). Data: a parts list.
3. **Registry hooks** — `<material>`/`<need>`/`<moodlet>` `@define`s and biome `@subtype`.
   Data — the biome `@subtype` is ALREADY an explicit stored-data id, hand-authored.
4. **The biome classifier** — the ONE behavioral surface, and it is perfectly regular: a
   conjunction of threshold tests on 3 noise dimensions (`lt`/`ge`/`le` + `and`), then a tile
   choice + ordered salted scatter draws (`<salt> ^rand call <p> lt if <thing> &thing.1 set`,
   last write wins). Declaratively expressible without loss ([F3](forks.md#f3)).

Never used: conditionals beyond the biome shape, loops, `@on_update`, cross-def reads. The
`@on_destroy` stubs in `visual/tiles.rd` are dead boilerplate (`0 return`) and die unmourned.

## The shape of the migration

**The `Bundle` is the seam** ([F2](forks.md#f2)): every consumer — the wasm `Content`, the npc's
`fetch_corpus`, the edge's worldgen + `/content`, the worker's disk-loaded speeds — reads Bundle
accessors, never the parser. The migration swaps the LOADER (TOML → the same `Bundle`) and
translates the corpus; consumers keep their API. This is also the "for now" insurance: if a
scripting layer ever earns its way back, it is another loader behind the same seam.

**Ids become EXPLICIT** ([F1](forks.md#f1)): the current first-appearance rule bakes def ids into
stored zones, pawn defs, and need payload words — TOML table order is too fragile to carry that.
Every def authors its id; the loader REFUSES duplicates/zeros. Migration pins each id to today's
positional value, proven by the golden gate.

**Nothing changes until proven identical** ([F5](forks.md#f5)): a golden fixture of every Bundle
table + a worldgen sample grid + the needs-eval fixture, dumped from the `.rd` corpus, must be
reproduced exactly by the TOML corpus before any consumer swaps. The old loader dies in the same
stream, one commit after the gate passes ([F4](forks.md#f4) renames the crate to `shared/content`
at the end — the name must stop lying, but only after the diffs stop being load-bearing).

## Exit

The world renders identically on the TOML corpus (user's eyes), the wolf still resolves
`0x30010070` + trips + thirst-flips in the standing drills, `.rd`/parser/vm are DELETED (git is
history), and docs (`VARIABLES.md`, memories) speak TOML.
