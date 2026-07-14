# SpacetimeDB / DSL implementation plan

> **Status tracking has moved.** Per [`docs/CONVENTIONS.md`](../../../../../../../CONVENTIONS.md), the live
> execution state (completed / remaining / todo / issues / forks / blockers) now lives in
> [`docs/work/spacetime-rewrite/`](../../../../../../../work/spacetime-rewrite). This folder keeps the staged
> *plan/design detail* (`s0`–`s7`); it will migrate to `docs/components/shard/{plan,design}`.

The staged plan to move the code onto the [`spacetime-tables`](../../design/README.md)
design. That folder is *what the shard should be*; this folder is *how we get there*, one
landable stage at a time. [`divergences.md`](../../current/divergences.md) is the gap
list these stages close.

**How to use this folder:** each stage is one file with a Goal / Changes / Verify /
Depends-on. Land them in order; verify on the running stack before the next. Point me at a
single stage file to work or fix it. Status legend in the docs: ✅ done · 🔨 in progress ·
⬜ not started · ✏️ decision to confirm.

## The one idea that shapes everything

**The tic + fence spine stays; per-event computation *and* the ordering model change.**
Untouched: the metronome (`bump`, `master_tic`, the `event_tic = master_tic + N` gap — now +3, run
tic+1 work), `claim`/`resolve` + the `state_log` `server_id` **fence**, and the tables.

What changes:
- *What a resolve computes* — the `apply_event`/`resolve_events::<Spatial>` `ACTION_*` switch in
  [`shared/tick/domain.rs`](../../../../../../../../shared/tick/src/domain.rs) becomes a generic interpreter over
  `actions: Vec<u64>` ([event-dsl.md](../../design/event-dsl.md)).
- *How ordering/dependencies work* — the old **`Phase`** (inbound/data/outbound) and the
  implicit actor-read machinery are **replaced by explicit `await` on a row's alias + tic
  queueing**. One action *vector* per row (may hold several actions hitting several **targets**);
  a cross-tic dependency is a *separate* row chained by `await`. `await` **defers** (check
  timeout → `FAIL`, else park and revisit), never blocks. There is **no phase**, and **intra-tic
  order is not guaranteed**.

✅ **Decided** (was open): keep `read_rule` (`resolved_through`) as the `await`/defer substrate,
with cross-entity reads at **≤ T−1**; **drop `Phase` and `priority`** (they served the same-tic
actor-read model the DSL replaces). See [lifecycle.md](../../intent/lifecycle.md).

**The safety model** ([lifecycle.md](../../intent/lifecycle.md)): work is right-shifted
so a resolve only *adds* to an immutable settled tic; every phase is idempotent + re-driven;
writes only land at the frontier and reads only touch the sealed past, so **"write behind a
read" is structurally impossible** — the master just **drops** rows that miss their window (no
watermark); GC is dumb (refcounted holders); the row carries a durable `status` state machine.
That's what makes crashes recoverable and
cross-shard writes safe (convergent, not atomic).

> ✅ **Full-stack hot path proven live** (S0–S5 + terrain). Real `master` (2 Hz, drop→bump) +
> real `worker` (two-phase loop + DSL interpreter) drove an appended **spawn** then **move**
> end-to-end: entity materialized, then moved with kind carried forward and facing computed —
> autonomously, not hand-driven ([issue 004](../../../../../../../work/spacetime-rewrite/issues.md)).
> **Everything still open is in [remaining.md](../../../../../../../work/spacetime-rewrite/remaining.md)** — including the simplifications
> inside the ✅ stages (single-shard only, no deterministic composition, `SKIP`/`?:` branch,
> DAMAGE verb, tic-gating), the functional-neutral reconciliations (#4/#5/#10), and the
> client/compose integration.

## Stages

| # | stage | closes | status |
|---|-------|--------|--------|
| S0 | [Foundation — decisions + codec](s0-foundation.md) | — | ✅ |
| S1 | [The interpreter (pure), proving `MOVE`+`SPAWN`](s1-interpreter.md) | — | ✅ hot verbs (MOVE/SPAWN); DAMAGE/actor-reads TODO ([remaining](../../../../../../../work/spacetime-rewrite/remaining.md)) |
| S2 | [Event schema + lifecycle + holder table](s2-event-schema.md) | div #1,6,7 | ✅ |
| S3 | [Worker: two-phase resolve (enqueue+execute)](s3-worker.md) | div #1,6,9 | ✅ lifecycle live; **single-shard, no composition/defer/tic-gate yet** ([remaining](../../../../../../../work/spacetime-rewrite/remaining.md)) |
| S4 | [Producers → word streams (gap +3)](s4-producers.md) | div #1,9 | ✅ **full-stack live-verified** (real master+worker drive spawn+move) |
| S5 | [Control flow (`AWAIT`/defer/`FAIL`)](s5-control-flow.md) | — | ✅ await-gate + abort live; **`SKIP`/`?:` branch TODO** ([remaining](../../../../../../../work/spacetime-rewrite/remaining.md)) |
| S6 | [`PACK` settle (cold→hot mint is in S3 enqueue)](s6-hot-cold.md) | div #2 | ✅ **live-verified**: seed_cold_row + find-or-mint (cold→hot) + pack_settle (hot→cold). Trigger (sweep) is the follow-up |
| S7 | [Cold/reference/shard-split tail](s7-tail.md) | div #3–5,8,10 | 🔨 cold modules retired (#3 ✅); region_zone (#4), hot_reference re-key (#5), server_reference (#10) remain — functional-neutral reconciliation |

## Sequencing

```
S0 codec+decisions ─┬─▶ S1 interpreter ─┐
                    └─▶ S2 event schema ─┴─▶ S3 worker runs it ─▶ S4 producers ─▶ S5 control flow ─▶ S6 mint/get ─▶ S7 tail
```

- **S1 ∥ S2** (pure logic vs schema) converge at **S3**.
- **The S1 equivalence oracle** (interpreter `MOVE` output == today's resolver) is the key
  de-risker — proven before anything is swapped.
- **Milestone after S4:** the DSL fully owns the existing spawn/move gameplay; S5–S7 add the
  new capability.

## Decisions (Stage 0) — all confirmed

Detail in [s0-foundation.md](s0-foundation.md). All five are settled; none block S0.

| id | decision | status |
|----|----------|--------|
| D1 | branch (`?:`) encoding | ✅ forward-only `SKIP` word w/ `LITERAL` count — no backward jumps → bounded execution |
| D2 | word tag name + values | ✅ tag renamed **`op_code`** (not `kind`, to avoid `kind_reference` collision); values `LITERAL/OBJECT/ACTION/ALIAS`, control verbs are action_references |
| D3 | hot identity | ✅ `entity_key` is a real reference (the `u64` `entity_reference`), not an opaque key; `hot_reference:u32` re-key deferred to S7 |
| D4 | `server_reference` layout | ✅ keep functional `server_type:6\|server_id:10`; geographic deferred |
| D5 | VM reuse | ✅ **purpose-built** worker VM — `shared/dsl` is a text/visual VM with nothing meaningful to share; reuse only if a primitive later falls out |

## Cross-cutting risks

- **Build/deploy dance:** a macro change (S2) needs redeploy + worker-binding regen; a worker
  op-set change (S3/S5/S6) needs `rd build worker`. The **stale master/worker-binary** failure
  (worker runs an old deleted-path binary, silently disconnects, `tic_meta` empty → nothing
  resolves) bites here — rebuild the worker on every op-set change.
- **Determinism:** resolution must stay deterministic + bounded under the fence (replayable).
  Cross-row dependencies are explicit `await`s (not implicit reads); intra-tic order isn't
  guaranteed, so a resolve must not depend on it. Proven at S3 — do not shortcut it.
- **Scope discipline:** hot_reference re-key, geographic `server_reference`, cold-module
  retirement all live in **S7**, deliberately off the critical DSL path.
- ✅ **Cross-shard row atomicity — resolved by convergence** (was KNOWN-OPEN). Per-shard writes
  are idempotent (keyed by `event_reference`) and the row completes only when all shards have
  applied; a worker that dies mid-write is re-driven (skip done, finish rest). Not atomic across
  shards — **eventually-consistent** over a tic or two — but exactly-once and safe. Same
  mechanism covers the enqueue phase's multi-shard stand-up.
- **Latency:** the enqueue phase widens the gap **+2 → +3** → ~1.5–2 s request-to-visible at
  2 Hz. Fine for world sim; player-facing actions ride the client prediction / render-delay.
