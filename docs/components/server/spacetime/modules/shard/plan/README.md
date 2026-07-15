# Plan — `shard` (how we close current → design)

_Last updated: 2026-07-14._

## Done — the staged build (S0–S7)

The original path onto the [`design/`](../design/) is the staged plan in
[`stages/`](stages/) (S0 foundation → S7 tail). It's complete; the granular done-log
is [`work/spacetime-rewrite/completed.md`](../../../../../../work/spacetime-rewrite/completed.md).

With the 2026-07-14 conformance re-cut, **every identity/keying divergence is closed** (#2, #4, #5,
#10, #11) and the reference model is live-verified. T-9 then verified that the **pipeline-shape**
divergences (#1, #6, #7) were closed by this staged build too and merely never retired from the
ledger. **The shard matches its design**; the only open divergence is #8, a deployment split the
design already defers.

## Forward — the phases

> ⚠️ **Rewritten 2026-07-14 after executing P1.** The first cut of this section sequenced P1→P4 off
> [`current/divergences.md`](../current/divergences.md). **That ledger was stale**: #1, #6 and #7
> described code that no longer existed, so **P2 and P3 were phantom work** — already built by the
> S0–S7 staged build and never retired from the ledger. Corrected below, and the correction is kept
> visible because the failure mode matters more than the plan did: *a plan is only as true as the
> `current/` it was derived from.*

### ✅ P1 · Restore the gates, clear the deck — **DONE (T-6/T-7/T-8)**

`cargo test --workspace` gates again (107 tests, green for the first time — the red test was **born
red**, never a regression). Dead modules + the priority-DAG gone. Detail:
[`work/…/completed.md`](../../../../../../work/spacetime-rewrite/completed.md).

**D-7 closed too (2026-07-14):** `check` is now two passes — native, then wasm32 with `--features
js` — so it can no longer go green while the browser surface is broken. Verified by breaking a
`js`-gated call site on purpose: the old native-only gate said `Finished`, the new one fails.
`check-native` survives as an explicit escape hatch.

### ✅ P2 · #1 word DSL — **was already done** (verified T-9)

`EventLog` has carried `actions : Vec<u64>` since S1/S2;
[`shared/codec/src/event_word.rs`](../../../../../../../shared/codec/src/event_word.rs) is the word
frame + op_code palette, and [`shared/tick/src/vm.rs`](../../../../../../../shared/tick/src/vm.rs)
is the **purpose-built** stack machine (`SKIP`/`FAIL`/`DAMAGE`/`encode_if`), which the worker runs
per row. The only real work T-9 found was finishing **#9's `Phase` half** — see below.

### ✅ P3 · #6/#7 lifecycle + holder GC — **was already done** (verified T-9)

The `STATUS_*` machine, the `claim → stand_up → ready` enqueue phase separate from `resolve`,
`OpenRows`, `Holder` + `release_holds`, `drop_timed_out` called by the master **before** `bump`, and
a `tick_gc` that is already the trivial zero-holder/non-latest/old rule. Called "the biggest
structural change" here on the strength of a ledger entry that was a year of commits out of date.

### 🟡 P4 · #8 · Event/data shard split — **the only open divergence**

`event_log` on event shards; `state`/`state_log`/`cold` + `Holder` on data shards. Same generic
module, a **deployment** split — the design defers it, and nothing waits on it. **Not scheduled.**

### So what *is* next?

Nothing in this component is blocking. The honest options, none urgent:

1. ~~**Close D-7**~~ ✅ **done** — `check` is two-pass; both gates gate now.
2. **Retire the stale-ledger risk** — #1/#6/#7 sat "open" for a day and sent a whole plan chasing
   them. The ledger is now verified, but nothing *keeps* it honest. Worth a convention: close the
   divergence in the same commit that closes the code.
3. **P4/#8**, if the deployment split is wanted sooner than the design assumes.
4. **Leave the pipeline alone** and take the next real feature — the shard matches its design.

---

The live execution state stays in the work-stream
[`docs/work/spacetime-rewrite/`](../../../../../../work/spacetime-rewrite/) — P1…P4 all fall out of
the rewrite, so they keep its folder rather than minting new ones. This file holds the **sequence +
reasoning**; [`todo.md`](../../../../../../work/spacetime-rewrite/todo.md) holds the executable cut
(**T-6…T-8** landed; T-9/T-10 dissolved into "already done"), and items move todo → remaining →
completed as usual.
