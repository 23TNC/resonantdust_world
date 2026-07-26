# Torch thing — forks

_Decisions, with the option not taken and why. Chronological append._

### F1 — OVERLAY (`set_thing`-style), not BASELINE (`seed`) — proposed
The `thing` module has two write primitives and only one is safe here.

**`seed(macro_position, subtype_id, layer_id, things)` replaces a whole baseline row.** It builds one
`EntityState` keyed `cold_row_reference = (zone, subtype, layer)` holding a `Vec<DenseItem>`, and
inserts-or-updates it. Seeding torches through it would **overwrite every worldgen thing sharing that
`(zone, subtype, layer)`** — the trees and shrubs would vanish. It is the right primitive for *generation*
(it is what worldgen calls) and the wrong one for *adding an object*.

**The overlay is a per-cell override that composites over the baseline** and never touches it — which is
exactly "put a torch on this cell, leave the terrain alone". `set_thing` already does this for a single cell
(and uses `kind_reference == 0` as a removal override, which a future "extinguish/remove torch" wants).

**Taken: overlay.** `place_things` is a batched sibling of `set_thing` rather than a variant of `seed`.

**Cost, accepted:** the overlay is described as the path for *player mutations* while this is trusted
server-side placement, so it slightly blurs that distinction. The alternative — teaching `seed` to merge
rather than replace — changes the meaning of a primitive worldgen depends on, which is worse.

### F2 — the EDGE resolves `kind_reference`; the module never names a kind — proposed
`modules/thing/Cargo.toml` depends on `spacetimedb` + `resonantdust-codec` **only**. It cannot link the DSL,
so it cannot turn `"torch"` into a `kind_reference`. Three ways out:

1. **Hardcode the id in the module.** Rejected: it creates a second authority for name→id beside the DSL, and
   the ids are append-ordered — `torch` sits after `wolf` precisely so nothing renumbers, and a hardcoded
   constant would silently desync the first time someone reorders content.
2. **Link the DSL into the module.** Rejected for now: it is a wasm module with a size budget, and the edge
   already holds a live, hot-reloadable DSL runtime. Duplicating that into every data shard is the wrong
   direction.
3. **Parameterise the reducer and call it from the edge.** **Taken.** `place_things` takes
   `kind_reference` as an argument and is kind-agnostic; the edge resolves `thing_object_id("torch")` from its
   own bundle at the point of use — the same discipline `current_worldgen()` already documents ("read the live
   value **at the point of use**, never cache the `Arc` across a reload").

A pleasant side effect: `place_things` is not torch-specific. "Put this kind at these cells" is the primitive
a world editor or a build action wants, so the feature pays for itself twice.

### F3 — RETIRE the biome scatter once seeding works — proposed
The `10 ^rand call 0.006 lt if torch` draws in `forest` and `plains` were a stopgap to get any light on
screen. Keeping them alongside seeded torches would restore the exact problem this stream exists to fix:
the torch count becomes nondeterministic again, so no test can assert an exact number or position.

**Taken: remove the scatter, keep the KIND.** `content/{data,visual}/things.rd` keep `torch` and its
`&thing.light.*` — that is the content contract the client reads, and it is what makes the light work.
Only the placement moves.

**Sequencing matters:** retire in [P4](todo.md), *after* [P3](todo.md) proves seeded torches light the world.
Removing the scatter first would leave the world dark mid-stream with no way to tell a seeding bug from an
absence of torches.

### F4 — how many torches, and where? (OPEN — needed by P0)
Wants a decision before P1 so the fixture is nameable.

- **A handful at fixed cells in the seed zone** (e.g. 3–4 in zone 0, spread so their pools do not merge).
  Best for testing: exact positions, and non-overlapping pools mean each one's falloff is measurable in
  isolation. Matches the user's original "3 lights" framing.
- **A denser deliberate pattern** (e.g. a lit path or a ring) — better looking, more useful for judging the
  additive accumulator under overlap, worse as a test fixture.

**Suggested:** the handful, and add overlap deliberately later when
[primitive-graph P10](../2026-07-25-primitive-graph/todo.md)'s differential pass needs overlapping lights to
exercise. Note that **one** torch is not enough: a single light cannot reveal an accumulation bug.
