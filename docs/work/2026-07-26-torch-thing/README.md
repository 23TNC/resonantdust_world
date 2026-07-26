# Torch as a real THING — seeded in spacetime at init — 2026-07-26

_Components: [`server/spacetime`](../../components/server/spacetime/) (`modules/thing`) · `server/edge`
(DSL + zone seeding — no component doc yet) · [`client/webgl`](../../components/client/) (already consumes
it). Phases in [`todo.md`](todo.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md)._

## The decision (user, 2026-07-26)
**Implement torches properly: real THINGS seeded in spacetime as part of init, giving a full stack end to
end.** The current torches are a stopgap and should be retired once this lands.

## What is wrong with the current torches
[primitive-graph P11](../2026-07-25-primitive-graph/todo.md) got lights on screen by adding `torch` to the
**biome DSL scatter** (`10 ^rand call 0.006 lt if torch &thing.1 set`). That proved the content→light chain,
but it makes a torch the wrong *kind of thing*:

1. **A torch became terrain.** Biome scatter is for landscape cover — grass, trees, reeds. A torch is an
   OBJECT: it has an identity, a position someone chose, and eventually state (lit/unlit, fuel).
2. **Placement is probabilistic**, so no test can assert "the torch at (12, 9) lights these tiles". Every
   verification so far has been "count the lights and look at a screenshot".
3. **Only newly-generated zones have them.** Worldgen output is persisted, so the spawn area has **0 torches
   across 3,429 prims** while a fresh region has 74 — recorded as
   [primitive-graph B-4](../2026-07-25-primitive-graph/blockers.md). Seeding at init **dissolves B-4**: the
   torches exist in the zone regardless of when its terrain was generated.
4. **Nothing can ever interact with one**, because it has no entity identity — it is a `kind_reference` in a
   cell, indistinguishable from a shrub.

## The target path
```
thing module reducer (init-time seed)
  → entity_state overlay (cold thing storage, carries kind_reference)
    → edge composite → WS
      → client onColdThings → Primitive + .light   (WorldBridge.lightFor(kindId))
        → carried light LEAF under the billboard's carrier prim
          → light_presence → LIGHT_FRAG bake → lit world
```
Everything from `onColdThings` rightward **already works** — it is what P11 proved. This stream replaces only
the leftmost box: probabilistic worldgen scatter becomes a deliberate, addressable placement.

## Two findings that fix the architecture
Both came out of reading the code before planning, and both rule out the obvious approach:

**A torch must live in COLD THING storage, not the entity/mover path.** The mover path cannot carry a kind:
`PLACE` is position-only, which is exactly why the npc's wolves render as debug circles —
*"its content kind (wolf sprite) isn't set yet — `PLACE` carries position only"*
([`client/npc/src/main.rs`](../../../client/npc/src/main.rs)). Since **light is per-kind**
(`WorldBridge.lightFor(kindId)`), a torch placed as a mobile entity would have no kind, therefore no light —
the feature would silently not exist. Cold thing storage carries `kind_reference` in every write
(`seed`/`set_thing`), which is why the scatter works at all. See [I1](issues.md#i1).

**The thing module cannot resolve `"torch"` itself.** Its `Cargo.toml` depends on `spacetimedb` +
`resonantdust-codec` only — **not the DSL** — so it has no way to turn a name into a `kind_reference`. The
**edge** owns the DSL runtime (worldgen) and already drives `thing.reducers().seed(...)`. So the placement
reducer must be **parameterised by `kind_reference`** and called from where the DSL lives. Hardcoding a kind
id in the module would put a second authority next to the DSL, which
[VARIABLES](../../VARIABLES.md)-style ownership forbids. See [F2](forks.md#f2).

## Shape of the change
- A new **`place_things(macro_position, cells, kind_reference, data)`** reducer on the `thing` module —
  idempotent, overlay-based, kind-agnostic. It is not torch-specific: it is "put this kind at these cells",
  which is the primitive a builder/editor will want later anyway.
- The **edge** resolves `thing_object_id("torch")` from its DSL bundle and calls it for the seed zone at
  startup, so the torches are present in the world the camera opens on.
- The **biome scatter is retired** ([F3](forks.md#f3)) — keeping both would make the torch count
  nondeterministic again and defeat the point.

## What this unlocks
- **Deterministic lighting tests.** A torch at a known cell means an assertion like "tile (12,9) receives
  light from exactly one source" replaces "count them and squint".
- **B-4 dissolves** — the world you actually look at is lit.
- **A real object to build on**: lit/unlit state, fuel, being carried by a pawn (the primitive graph already
  models a torch as `prim{billboard, light}`, and a *carried* torch is that same prim under a hand prim).
- **The full stack is exercised** by one small feature, which is the point: reducer → cold storage → edge
  composite → WS → client → prim graph → presence → lightmap.
