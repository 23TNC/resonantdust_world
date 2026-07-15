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

## P1 · Restore the gates, clear the deck — ✅ **DONE (2026-07-14)**

T-6/T-7/T-8 landed → [completed.md](completed.md). `cargo test --workspace` gates again (107
tests, green for the first time). **D-7 still stands:** the workspace `check` skips the wasm's
`js`-gated code, so `rd build shared` remains the real compile gate — that one is a tooling fix we
have not made.

**One item didn't finish as scoped:** #9's `Phase` half is live ordering, not vestigial, so it
moved into **T-9** where its replacement gets built. See T-8 in [completed.md](completed.md).

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
- **Absorbs the `Phase` half of #9** (from T-8): `resolve_events` orders a tic's events
  Inbound → Data → Outbound today. The design replaces that with explicit `await`s + the read rule
  (reads at ≤ T−1 → DAG by tic). `Phase` + `action_phase` die **when that replacement exists**, not
  before — deleting them earlier changes resolution order with nothing behind it.
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
