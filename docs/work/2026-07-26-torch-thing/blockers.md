# Torch thing — blockers

_Things needing human input: what blocks, why it needs you, and a suggested path. Newest first._

## B-1 — how many torches, and where? (open, 2026-07-26)
[F4](forks.md#f4) needs answering before [P1](todo.md), because the cells become the test fixture that
later assertions cite by name.

**Why it needs you.** It is a look-and-feel call with a testing consequence, and the two pull apart:
- **A handful (3–4) at fixed, well-separated cells in zone 0.** Non-overlapping pools mean each torch's
  falloff is measurable in isolation, which is what makes a real lighting assertion possible. Matches your
  original "3 lights" framing.
- **A deliberate pattern** (a lit path, a ring). Better looking and better for judging the additive
  accumulator under overlap, but a worse fixture.

**Suggested:** the handful now; add overlap deliberately when
[primitive-graph P10](../2026-07-25-primitive-graph/todo.md)'s differential pass needs overlapping lights to
exercise. One torch is not enough either way — a single light cannot reveal an accumulation bug.

## B-2 — does "in spacetime as part of our init" mean a module `init` reducer specifically? (open, 2026-07-26)
Your words were "we will add them in spacetime as part of our init". This stream honours that in **substance**
— seeded, deterministic, present from a cold start — but the *call* originates at the **edge's zone seed**
rather than an `#[reducer(init)]` in the `thing` module.

**Why:** the module cannot resolve `"torch"` → `kind_reference`; it links `spacetimedb` + `codec` only, with
no DSL ([I2](issues.md#i2)). A module-side `init` could only place a hardcoded id, which puts a second
authority beside the DSL and desyncs the first time content is reordered.

**Why it needs you:** if you specifically want the placement to originate *inside* the module, that is
achievable but means linking the DSL into the data shard ([F2](forks.md#f2) option 2) — a bigger change with a
wasm size cost, and it duplicates a runtime the edge already hot-reloads. I did not want to quietly
re-interpret your instruction into the cheaper shape without flagging it.

**Suggested:** edge-driven for now. Say the word if you want it module-side and I will plan the DSL link.
