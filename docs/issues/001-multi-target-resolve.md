# 001 — Committing N targets in one `resolve` (phase S2)

## Problem

A row is **atomic** and may touch several targets (an AoE; `a b MOVE ; c d MOVE`). The worker
computes *all* target effects in scratch and must commit them in **one** `resolve` reducer call
(same-shard = one transaction). But a SpacetimeDB reducer can't take a variable-arity argument
list, and the game payload is `decl_tick_pipeline!`-parameterized — so "write these N entities'
new states" has no obvious fixed signature.

## Options

- **A · `resolve(worker_reference, event_reference, results: Vec<TargetState>)`** where the macro
  emits `TargetState { entity_key, <payload…> }` as a `#[derive(SpacetimeType)]` struct. One
  atomic call; the reducer loops the Vec, writing each `state`/`state_log`. Type-safe.
- **B · Per-target sub-resolve** — the worker calls `resolve` once per target; the row completes
  when all targets are done. Simple fixed signature, but **not atomic across targets** (partial
  state is visible mid-row) and needs its own per-row completion barrier.
- **C · Pack all target states into a `Vec<u64>` blob**, decode in the reducer. Fixed signature,
  but loses type safety and re-implements payload (de)serialization by hand.

## Choice — **A**

`resolve(worker_reference, event_reference, results: Vec<TargetState>)`, with a macro-emitted
`TargetState`.

## Why

- **A preserves the row-atomicity invariant** ([lifecycle.md](../spacetime-tables/lifecycle.md)):
  same-shard targets commit in one transaction, exactly as the design requires.
- **B breaks that invariant** — partial rows become visible and it reintroduces a per-row barrier
  we'd otherwise get for free from the single call.
- **C** duplicates the codec and throws away the schema's type-safety for no gain; the macro
  already parameterizes `state`/`state_log` by payload, so emitting one more struct with the same
  fields is trivial and keeps everything typed.
- Cross-shard targets are a *separate* concern — those use the convergent multi-call path
  (idempotent per `(source_shard, event_reference)`), which is orthogonal to how one shard's
  batch is committed.

## Status

Implemented in S2 (the `TargetState` struct + `Vec`-taking `resolve`).
