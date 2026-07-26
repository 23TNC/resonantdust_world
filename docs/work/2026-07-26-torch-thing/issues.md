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

### I4 — the OVERLAY relays `ColdState`, not `ColdThing`, so it cannot make a lit thing (2026-07-26)
[F1](forks.md#f1) chose the overlay because `seed` replaces a whole baseline row and would erase the zone's
trees. That reasoning was right; the conclusion was wrong, for a reason I did not check: **the two storage
paths relay different message types.**

- baseline → `ColdThing { zone, subtype_id, layer_id, tic, things }` — the batched scatter frame that
  `onColdThings` consumes, where `.light` is attached via `lightFor(kindId)`.
- overlay → `ColdState { entity_reference, position_reference, definition_reference, … }` — the per-ENTITY
  frame, handled by the entity/mover path, which never builds a cold-thing prim.

Symptom: the write succeeded at every server layer — `place_things` ran, the overlay row existed with the
right cells and kind, the subscription covered the zone — and the client showed nothing. I chased a
subscribe-timing race and an `on_applied` replay gap (real, and worth keeping: the subscription asks for
`overlay` but `on_applied` only replayed `entity_state`, so pre-existing overrides were never sent) before
noticing the frames were a different type entirely.

**Resolved by APPENDING to worldgen's payload instead.** `append_init_objects` pushes a `(subtype 0, entries)`
bucket into `layers.things` before the seed loop, so init objects ride the proven `ColdThing` path. Appending
rather than replacing is what addresses F1's original concern, so nothing is lost.

**Rule:** verifying a write reached the database proves nothing about whether the client can consume it. Trace
the READ path for the specific frame type before choosing a storage primitive.

### I5 — `kind_reference` is PACKED, not the raw object id (2026-07-26)
After the append fix the torches still did not appear. The tell was in the data: every worldgen
`kind_reference` in the table was large — 16, 36, 72, 98 — while mine was `8`.

`kind_reference = pack_kind_reference(kind_id, variant_id)` = `kind_id << 4 | variant`. Decoding the
neighbours confirms it: tree (id 1) → 16–31, shrub (2) → 32–47, flora (6) → 96–111. Passing the bare object
id `8` decodes as **kind_id 0, variant 8** — an invalid kind, which the client silently drops.

Fixed by using the existing `pack_kind_reference` helper. **The helper existed and I hand-rolled around it**;
reading the neighbouring values in the table is what caught it, not reading the code.
