# S1 — The interpreter (pure), proving `MOVE`

**Goal:** build the `Vec<u64>` interpreter as pure, SDK-free logic and prove it reproduces
today's `MOVE`, before wiring it anywhere.

**Status:** ✅ **done + live-verified** — the `Vec<u64>` stack machine (`shared/tick/vm.rs`) — hot verbs, `await_gate`, forward `SKIP`/`FAIL`; 32 unit tests green. Authoritative state: [`work/spacetime-rewrite/completed.md`](../../../../../../../work/spacetime-rewrite/completed.md). **Depends on:** [S0](s0-foundation.md). **Parallel with** [S2](s2-event-schema.md).

## Changes

- New module/crate (extend `shared/tick`, or a new `shared/event-vm`): a **purpose-built**
  stack machine (D5 — *not* built on `shared/dsl`), a function shaped like
  `run(actions: &[u64], base: EntityState, reads: &Reads) -> EntityState` that decodes words
  (`op_code` + `server_reference` + payload) onto a value stack of game references/values and
  executes them. Its value model is game references/state, not `shared/dsl`'s visual `Cell`
  tree — the only thing borrowed is the abstract postfix-dispatch shape.
- **Verb effects live in `shared`** (risks A2): the VM dispatches an `ACTION` to a shared rule
  fn (e.g. `apply_move`) that the edge (validate) and client (predict) also call — one
  deterministic implementation, so prediction matches execution.
- Implement the minimum op set to reproduce **both live verbs — `MOVE` and `SPAWN`** (both are
  on the wolves path the S3 verify exercises): `OBJECT` push + each verb writing the same fields
  [`apply_event(ACTION_MOVE)` / `ACTION_SPAWN`](../../../../../../../../shared/tick/src/domain.rs) do.
- `reads` is the operand-read interface (given an `OBJECT` operand → its `EntityState`); in S1
  it's a test double, in [S3](s3-worker.md) it's backed by the worker's cross-shard reads.

## Verify — the equivalence oracle

For a `MOVE` (and `SPAWN`) word stream, the interpreter's output **equals** the current
`resolve_events::<Spatial>` output for the equivalent `ACTION_MOVE` / `ACTION_SPAWN` event. This
equivalence test is the key de-risker for the whole plan: it proves the interpreter matches the
proven resolver *before* anything is swapped. Unit tests only — no wiring, stack stays green.

## Notes

- Keep it pure: no `spacetimedb`, no wasm, no I/O — same discipline as `shared/tick` today.
- Don't implement control flow / hot-cold ops yet (that's [S5](s5-control-flow.md) /
  [S6](s6-hot-cold.md)); S1 is only the operand-push + simple-verb core.
