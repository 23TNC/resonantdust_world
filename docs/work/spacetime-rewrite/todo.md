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

## P2 · #1 word DSL — ✅ **was already done** (verified T-9, 2026-07-14)

Not built by us this session: `EventLog.actions : Vec<u64>`, `codec::event_word`, and the
purpose-built `tick::vm` stack machine all landed with S1/S2. Divergence #1's "Code" line described
`actor_key`/`target_key`/`data0/1` — **identifiers that exist nowhere in the repo**. The real work
T-9 found was #9's `Phase` half, now done → [completed.md](completed.md).

## P3 · #6/#7 lifecycle + holder GC — ✅ **was already done** (verified T-9, 2026-07-14)

Same story: the `STATUS_*` machine, the `claim → stand_up → ready` enqueue phase, `OpenRows`,
`Holder`/`release_holds`, the master's `drop_timed_out` **before** `bump`, and a `tick_gc` that is
already the zero-holder/non-latest/old rule. All present and live-verified since 2026-07-13 —
`completed.md` even said so; the divergence ledger just never caught up.

## P4 · #8 · Event/data shard split — **deferred, the only open divergence**

`event_log` on event shards; `state`/`state_log`/`cold` + `Holder` on data shards. Same generic
module, a **deployment** split. The design defers it; nothing waits on it. Not scheduled.

---

**This work-stream has no open items.** What's genuinely left is listed under "So what *is* next?"
in [`shard/plan/README.md`](../../components/server/spacetime/modules/shard/plan/README.md) — the
strongest candidate being **D-7** (the workspace `check` still skips the `js`-gated wasm, so one
blind gate remains).
