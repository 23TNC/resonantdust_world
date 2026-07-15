# Todo — spacetime rewrite (planned, not started)

Executes: the `shard` component. Planned work not yet begun; moves to [remaining.md](remaining.md)
when started. Newest-first.

The **sequence + reasoning** for all of the below is
[`shard/plan/README.md`](../../components/server/spacetime/modules/shard/plan/README.md) (P1…P4);
the per-item detail is
[`shard/current/divergences.md`](../../components/server/spacetime/modules/shard/current/divergences.md).
This file is the executable cut — it stays in this work-stream because it all falls out of the
rewrite. Validate every shape against
[reference-model.md](../../components/shared/codec/design/reference-model.md) as you go, and **log
any deviation in [deviations.md](deviations.md) the moment you make it** — D-3 is what happens
otherwise.

---

## P1 · Restore the gates, clear the deck

Cheap, and first: **two of our three build gates don't gate** — the workspace `check` skips the
wasm's `js`-gated code (**D-7**) and `cargo test` is red at HEAD. P2 is the largest refactor in the
backlog; starting it without a working suite is how a D-3-class bug reaches the browser again.

### T-6 · Get `cargo test --workspace` green (the pre-existing red test)

- `dsl::loader::material_registry_and_packed_channels` panics at
  [loader.rs:802](../../../shared/dsl/src/loader.rs) — `visual_for_def(2)` (stone) → `None`.
  Pre-existing at HEAD, unrelated to the re-cut (`dsl` doesn't depend on `codec`) →
  [issues.md](issues.md).
- Work out whether the **test fixture** drifted or the **loader** regressed (git-log
  `shared/dsl/src/loader.rs` + the `content/` corpus it builds against; the DSL maps tile
  name ⇄ def_id ⇄ visual). Fix the correct side.
- **Do not** delete the assertion/test to go green, and don't paper over a real content-loading bug.
- Done = `docker compose -f shared/compose.yml run --rm test` fully green.

### T-7 · #3 · Delete the dead modules

- Per [cleanup.md](../../components/server/spacetime/modules/shard/plan/cleanup.md):
  `git rm -r server/spacetime/server/modules/{cold_tiles,cold_things,experiment}`; remove the
  `cold_tiles) / cold_things)` fam arms in `bin/lib/redeploy.sh` + any
  `default_cold_tiles_db()` / `default_cold_things_db()` and config references.
- Their wire paths were retired in `2b2fc58` / `d47f152`; only the dirs + build wiring remain.
- Done = `rd redeploy` plans cleanly without them; nothing references the deleted dbs.

### T-8 · #9 · Drop `Phase` / `priority` from `shared/tick`

- The design's ordering model is explicit `await`s + the read rule (`resolved_through`), reads at
  **≤ T−1** → DAG by tic → deadlock-free. **No `Phase`, no priority-DAG** — it's vestigial.
- Remove `Phase` (`shared/tick/src/domain.rs`) + `priority` (`actor_read_tic`) and their call sites.
- `shared/tick` has **no component folder yet** — create it lazily only if this grows past a
  deletion.

---

## P2 · #1 · `event_log` → `actions : Vec<u64>` (the word DSL)

The main event. Replace the fat struct (`actor_key`, `target_key`, `data0/1`, the named
`*_server_reference` columns) with the design's flat postfix word stream
([event-dsl.md](../../components/server/spacetime/modules/shard/design/event-dsl.md),
[events.md](../../components/server/spacetime/modules/shard/intent/events.md)); `resolve_one`
becomes the interpreter.

**Do it now, not later:** the words are made of exactly the references we just re-cut, and that
vocabulary is settled + live-verified today — build on it once instead of twice. Meanwhile the edge
and npc already write the fat struct; every new caller widens the blast radius.

### T-9 · The word stream + interpreter

- **Scope bound (load-bearing):** hand-built words for the **existing verbs only** — *no surface
  syntax, no compiler*. The surface-plan → `Vec<u64>` compile step is unscoped and is exactly the
  thing that balloons; make it its own decision later, once real word streams exist to compile to.
- Module: `event_log` keeps `event_reference`, `tic`, `worker_reference`, `status`; the scalar
  target/actor/data columns become `actions : Vec<u64>`.
- Worker: `resolve_one` → RPN interpreter, reusing `shared/dsl`'s value-stack VM behind a
  `Vec<u64>` word decoder. Operands push, verbs pop+run. No `action` enum, no positional slot
  schema, no actor field, no operand table.
- Callers: the `append` signature, the edge's producers, npc.
- Word shape is settled: `op_code:4 | reserved:12 | server_reference:16 | payload:32`.
- Schema change → **live check, not just a green build** (D-6: subscription SQL is a string).

---

## P3 · #6 + #7 · Lifecycle state machine + drop barrier / holder GC

The biggest structural change; **adjacent to P2 on purpose** — it lands in the same `resolve_one`
P2 rewrites, so we touch that code once, deliberately, instead of twice.

### T-10 · Lifecycle (#6) + causality scaffolding (#7)

- **#6:** status (`enqueue → queueing → in_queue → running → complete` / `queue_failed`) + `failed`
  + `tic_state_change`; **enqueue** (stand up target `state_log` rows) and **execute** separate,
  each idempotent + crash-recoverable via re-drive; open-rows table keyed by `event_reference`;
  timeout eviction on `tic_state_change`.
- **#7:** holder table refcounting pending reads/writes per `state_log` row; the master's
  `drop_timed_out` barrier **before** `bump`; GC reduced to the trivial zero-holder,
  non-latest, old rule (today it's a horizon heuristic).
- Design: [lifecycle.md](../../components/server/spacetime/modules/shard/intent/lifecycle.md).

---

## P4 · #8 · Event/data shard split — **deferred**

`event_log` on event shards; `state`/`state_log`/`cold` + the holder table on data shards. Same
generic module, a **deployment** split. The design already calls it later and #1 doesn't depend on
it. Not scheduled — listed so it isn't forgotten.
