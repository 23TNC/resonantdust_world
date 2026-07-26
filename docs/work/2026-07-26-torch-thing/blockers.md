# Torch thing — blockers

_Things needing human input: what blocks, why it needs you, and a suggested path. Newest first._

## B-1 — how many torches, and where? — RESOLVED 2026-07-26 by the user: **3, around tile (100, 50)**
**The fixture** (derived; each address level is an `x:4 | y:4` nibble pair):

| torch | global tile | zone | in-zone tile | `macro_position` | `tile_reference` |
|---|---|---|---|---|---|
| A | (100, 50) | (6, 3) | (4, 2) | `0x0063` = 99 | `0x42` |
| B | (108, 52) | (6, 3) | (12, 4) | `0x0063` = 99 | `0xC4` |
| C | (104, 58) | (6, 3) | (8, 10) | `0x0063` = 99 | `0x8A` |

All three sit in **one zone**, so seeding is a single reducer call and the whole fixture streams together.

**On overlap.** Zone edge is 16 tiles and torch reach is 6, so three torches inside one zone cannot be fully
non-overlapping (that would need >12 tiles of separation). Pair distances here are 8.2 / 8.9 / 7.2 tiles, so
each torch keeps a large isolated region AND there is a measurable overlap band. That is arguably better than
the strict isolation [F4](forks.md#f4) suggested: it exercises accumulation while still allowing a
single-source falloff assertion away from the seams.

<details><summary>B-1 as originally raised</summary>
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

</details>

## B-2 — module `init` reducer or edge-driven? — RESOLVED 2026-07-26 by the user
**"Wherever we seed the initial conditions, we will move them later."** So: **edge-driven**, at the zone
seed, which is where the DSL can resolve the kind. Explicitly provisional — the placement site is expected to
move once there is a proper initial-conditions story, so keep the reducer kind-agnostic and free of any
torch-specific assumption, and keep the CALLER thin enough to relocate.

<details><summary>B-2 as originally raised</summary>
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

</details>

_No open blockers._
