# Torch thing — issues

_Findings from reading the code before planning, and problems met while building. Chronological append._

### I1 — the MOVER path carries no kind, so a torch cannot be a mobile entity (2026-07-26)
The obvious reading of "add torches in spacetime" is to spawn them like the wolves — the npc's `Command::Place`
path, which is the most visibly "full stack" thing in the repo. **That would silently produce unlit torches.**

`PLACE` carries **position only**. From [`client/npc/src/main.rs`](../../../client/npc/src/main.rs):
*"its content kind (wolf sprite) isn't set yet — `PLACE` carries position only, so the mover draws a debug
circle until `CREATE` (or a definition verb) plumbs `definition_reference` through."* That is why the wolves
are circles rather than wolf sprites.

Light is **per-kind** — `WorldBridge.lightFor(kindId)` reads the DSL's `thing_light` row by kind. A prim with
no kind gets no light row, so a torch on the mover path would render as a debug circle emitting nothing, and
the failure would look like "the lighting is broken" rather than "the entity has no kind".

**Cold thing storage carries `kind_reference` in every write** (`seed`'s `DenseItem`, `set_thing`'s argument),
which is precisely why the biome scatter lights the world today. So a **static** torch belongs there.

**Consequence for later:** a torch *carried by a pawn* is a mover and will hit this wall. The primitive graph
already models that case (`prim{billboard, light}` under a hand prim), so plumbing `definition_reference`
through `PLACE`/`CREATE` is a prerequisite for carried light — worth its own stream, not this one.

### I2 — the `thing` module cannot resolve a kind name (2026-07-26)
`modules/thing/Cargo.toml` depends on `spacetimedb` and `resonantdust-codec` only. There is no DSL, so
`"torch"` → `kind_reference` is not expressible inside the module, and an `#[reducer(init)]` there could only
place a **hardcoded** id.

That would put a second authority for name→id next to the DSL. The ids are append-ordered on purpose —
`torch` was appended after `wolf` specifically so no existing `object_id` renumbers — and a server-side
constant would desync the first time content is reordered, silently placing the wrong kind.

Resolved by [F2](forks.md#f2): the reducer takes `kind_reference`, and the **edge** (which owns the live,
hot-reloadable DSL runtime that worldgen already uses) resolves it at the point of use.

**Note on the user's framing.** "Add them in spacetime as part of our init" is honoured in substance — the
torches are seeded, deterministic, and present from the start — but the *call* originates at the edge's zone
seed rather than a module `init` reducer, because that is the only place the name can be resolved. If a
module-side `init` is wanted specifically, it needs the DSL linked into the module first, which is
[F2](forks.md#f2) option 2 and a larger change.

### I3 — a schema change needs a LIVE check (2026-07-26, standing hazard)
Adding a reducer changes the module schema. The build gates are 2-pass native + wasm32 and go green without
exercising subscription SQL, which is a **string** — so a schema change can compile clean and then fail at
runtime with an SDK parse panic. Also, a stale DEPLOYED module produces the same panic.

So [P1](todo.md) carries an explicit live-check item: `rd redeploy`, confirm the edge connects, no parse
panic. Related standing trap: in-docker/WSL2 builds sometimes skip recompiling an edited file (reports
`Finished` with no `Compiling` and leaves a stale binary) — `touch` a source file to force it.
