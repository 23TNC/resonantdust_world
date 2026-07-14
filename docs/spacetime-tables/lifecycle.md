# The resolution lifecycle — how an event resolves *safely*

This is the load-bearing correctness doc: how a row goes from "requested" to "live data,"
crash-recoverable at every step, without ever modifying a settled tic. It builds on the event
log shape ([events.md](events.md)) and the DSL ([event-dsl.md](event-dsl.md)).

The one invariant everything protects: **a settled `(entity, tic)` is immutable.** We only ever
*add* resolved state at a tic; we never rewrite one. That's what makes crashed work safe to
re-run — re-executing sealed, immutable inputs is deterministic, so idempotent re-drive
converges to the same result.

---

## Event shards vs data shards

The event log is separated from the data it produces:

| class | holds | serviced by |
|-------|-------|-------------|
| **event shard** | the `event_log` — rows (plans) + their lifecycle `status` | workers *read* work from here and drive its lifecycle; the **master** sweeps drops here |
| **data shard** | `state_log` / `state` (game data) + `cold` + the **holder table** (which `event_reference`s hold a pending read/write on each `state_log` row) | workers *write results* here via reducers |

Workers service event shards and write into data shards. A row's targets live on data shards
(possibly several); the row itself lives on an event shard. This is why both *standing up* work
and *executing* it are multi-shard writes — and therefore both need the recoverable treatment
below. **There is no read watermark** — causality is enforced structurally by the staging + the
master's drop, not by a per-entity timestamp column (see [Concurrency](#concurrency--strict-staging-not-a-watermark)).

---

## The pipeline, right-shifted

Making enqueue recoverable adds a phase, so the tic gap widens from +2 to **+3**:

```
   request        enqueue          execute        live
   (edge)         (worker)         (worker)       (promoted)
   tic N+3   ──▶  tic N+2     ──▶  tic N+1   ──▶  tic N
   status=        stand up         run the        state
   enqueue        targets          action         visible
                  (mint cold →     vector
                   hot here) +
                  queue
```

At 2 Hz (500 ms/tic) that's ~3–4 tics ≈ **1.5–2 s** from request to visible. Fine for
world/NPC simulation (nothing waits on its own input). **Player-facing** actions must be hidden
by the client's prediction + render-delay (the synced-clock sync model) — the latency is
*correct* but would *feel* laggy raw. Robustness costs one extra tic vs. the old +2 design.

---

## The event lifecycle (a state machine on the `event_log` row)

One row, no side table. Columns: **`status`**, **`failed`**, **`tic_state_change`** (the tic the
row entered its current `status`, for timeout/eviction).

```
        (edge writes)
            │
            ▼
        enqueue ──picked up──▶ queueing ──stand-up ok──▶ in_queue ──picked up──▶ running ──done──▶ complete
                                  │                                  ▲
                                  └── can't stand up / master drop ──┴──▶ queue_failed   (terminal)
```

| `status` | meaning |
|----------|---------|
| `enqueue` | written by the issuer; no worker has taken it to stand up yet |
| `queueing` | a worker is standing up its target `state_log` rows |
| `in_queue` | stood up successfully; ready to execute |
| `running` | a worker is executing the action vector (writing data) |
| `complete` | done — all target writes committed |
| `queue_failed` | never made it out of enqueue in its window; terminal (its action is dropped) |

`tic_state_change` gates **timeout eviction**: a row stuck in `queueing`/`running` past a budget
is evicted and reassigned. Eviction is always safe — re-drive is idempotent (below), so a
wrongly-evicted (merely-deferring) row just resumes on the new worker.

> **The master owns the drop.** The `event_log` lives on event shards; the **master** (sole tic
> authority) calls a reducer to `queue_failed` every still-`enqueue`/`queueing` row whose window
> has expired, **then** bumps the tic — a deterministic per-tic barrier that cancels stale work
> *before* the tic it missed can roll. The drop targets only pre-queued rows (`status` check); a
> row that just reached `in_queue` is safe (the reducer-serialized race is settled by SpacetimeDB).

---

## Phase 1 — enqueue (stand up the work), recoverable

A worker takes an `enqueue` row → `queueing` and stands up a pending `state_log` row on each
**target** data shard (the issuer-designated `targets`; a row may have several).

- **Cold target ⇒ mint (unpack absorbed).** A `cold_reference` has *no* `state_log` row and no
  id — so "stand up its pending row" is *impossible* without giving it a hot identity. Standing
  it up therefore **is the mint**: `find-or-mint` the hot entity at that location, and the fresh
  hot `state_log` row **is** the pending-work row; the target is rebound to the `hot_reference`.
  There is no separate `unpack` action — promoting a cold *target* falls out of enqueue for
  free. (`find-or-mint`, not just mint: idempotent **by location** so a re-drive or a second
  event targeting the same cold object reuses the one hot entity, never duplicates it.) The mint
  is a structural data-shard reducer the worker calls — the shard still never runs the DSL.
  - **Mint appends to `cold_removed`, it does not rewrite the `cold` row.** The mint records a
    `u16` removal tombstone (`x:4|y:4|layer:4|type_id:4`) in the zone's `cold_removed` delta
    instead of mutating the big cold `Vec` — so a subscriber isn't re-sent the whole zone on
    every unpack. That tombstone *is* the `find-or-mint` "already unpacked" marker. The `cold`
    `Vec` is compacted later — by a **`PACK` action** (a worker, not GC): see
    [hot-cold.md](hot-cold.md).
- **Acquire holds.** Standing up a pending row registers the row's `event_reference` in the
  **holder table** for each target (a pending write) and for any source it will read (a pending
  read). These holds are what keep GC dumb: a `state_log` row is only reclaimable when nothing
  holds it (see [GC](#gc--dumb-reclamation-refcounted)).
- **Success:** all targets stood up → `in_queue`.
- **Idempotent + recoverable.** Standing up N targets is a multi-shard write; a worker can die
  mid-way. Dedup by the row's `event_reference`: each stand-up is keyed by it, and an
  **open-rows table** records which `state_log` rows the event holds open, so a re-drive skips
  the ones already stood up and finishes the rest (or backs out and `queue_failed`s). Another
  worker reaching a `queueing` row re-runs the same deterministic decision.
- **Miss your window ⇒ dropped.** A row that hasn't reached `in_queue` by the time the master's
  drop sweep runs is `queue_failed` (its holds/open-rows backed out). This is *the* causality
  mechanism, not just overload relief — see below.

---

## Phase 2 — execute (run the action vector), recoverable

A worker takes an `in_queue` row → `running`, runs the DSL interpreter over the row's action
vector, computing **all** target effects in scratch, then commits them to the data shards.

- **Defer until sources are settled (the read rule).** Reading a source `X` is only valid if `X`
  has **no pending work at or below the read tic** (`resolved_through`). If a source isn't
  settled, **defer** — leave the row, work other events, revisit (never block). Cross-entity
  reads resolve against **strictly earlier settled tics (≤ T−1)**, which makes the dependency
  graph a **DAG by tic** — deadlock-free by construction, so **no priority-DAG / cycle-breaker is
  needed** (that machinery only existed for same-tic mutual reads, which this forbids).
- **Deterministic composition.** When several rows target the same `(entity, tic)`, their
  contributions compose in **`event_reference` order** — never worker-arrival order — so
  re-execution always yields the same state. (Purely-commutative effects are order-free anyway;
  ordering by `event_reference` covers the rest.)
- **Convergent multi-shard write.** Committing to several data shards isn't one transaction;
  each per-shard write is **idempotent** (keyed by the row's `(source_shard, event_reference)`),
  and the row completes only once **all** target shards have applied. Crash after writing A, B →
  re-drive skips A, B (dedup), writes C, then `complete`. Same-shard writes are one atomic ST
  transaction; **cross-shard is convergent, not atomic** — safe (exactly-once effect) but
  eventually-consistent over a tic or two.
- **Release holds on `complete`.** Committing the row removes its `event_reference` from the
  holder table for every row it held (its reads and writes), atomically with the terminal
  transition. Release is tied to a *definite* terminal state (`complete` / `queue_failed` / the
  master's drop) so a crash can only *leak* a hold (harmless — the row just isn't reclaimed yet),
  never release one early (which would let GC drop a live row). The master's drop guarantees
  every row terminates, so holds always eventually release.
- **Done:** `complete`.

---

## Concurrency — strict staging, not a watermark

There is **no read watermark**. Causality is enforced by the *shape of the pipeline*, which makes
the violation it guarded against structurally impossible:

- **Every event enters at the frontier** (`event_tic = master + 3`) and flows forward; the master
  is monotonic, so **you can never target a past tic**. Writes only ever land at the advancing
  frontier.
- **Reads only touch settled tics** (`≤ T−1`, via the read rule). So a read only ever consumes
  the *sealed past*, and a write only ever lands at the *frontier* — the two can never cross.
  "Write behind a read" cannot happen because there is nowhere behind to write.
- The **only** guard left is "you missed your window": a row that doesn't reach `in_queue` before
  the master's drop-then-bump barrier is `queue_failed`. That's a check against the row's own
  `status` + the tic — no per-entity timestamp, no write-on-read.

This is why the watermark was redundant: the settled `state_log` frontier already encodes
everything it did, *given* that reads are always `≤ T−1`. The trade is explicit and deliberate —
see [risks.md](risks.md): **dropping is now load-bearing for correctness**, so each stage has a
hard ~1-tic budget and a missed action is *lost* (a real-time deadline, like dropping late frame
input). The edge/client must treat a `queue_failed` as a first-class outcome.

---

## GC — dumb reclamation, refcounted

GC makes **no correctness decision**. Every "is this still needed?" question is answered by the
**holder table** (which `event_reference`s hold a pending read/write on a row), maintained by
workers — not by a GC horizon heuristic.

A `state_log` row is reclaimable iff **all** hold:
- **no holders** — nothing has a pending read or write on it, and
- **not the latest** resolved row for its entity (that one *is* `state`, the current value), and
- **old** — behind the frontier.

That's the whole rule. GC never has to reason about await windows, read depth, or immutability —
those live in the holds (workers) and the drop (master). A leaked hold delays reclamation
(wasteful, safe); nothing GC does can drop a live row.

**Cold compaction is not GC's job either** — applying `cold_removed` tombstones / settling hot
objects back into `cold` is a worker-resolved **`PACK` action** ([hot-cold.md](hot-cold.md)). So
GC only ever deletes zero-hold, non-latest, old rows, and nothing else.

---

## Recovery, in one line

**Keep retrying until terminal.** Every phase is idempotent writes keyed by `event_reference` +
a durable `status`. A crash leaves the row mid-`status`; timeout evicts it; the next worker
re-runs the same deterministic decision — finishing partial work (skip done, do the rest) or
backing out — until the row reaches `complete` or `queue_failed`. No step can lose or duplicate
an effect.

**Dependencies need no cascade.** Every `await` has a required `TIMEOUT`, so a dependency that
disappears *for any reason* — dropped by the master, `queue_failed`, or a crashed worker — just
makes the dependent **time out on its own** and back out locally. There is no active propagation
from a dropped row to the rows that awaited it; it's N independent timeouts. (And because a cold
target is minted at the dependent's *own* enqueue, no row ever depends on another to create its
target — the only cross-row coupling is the execute-time `await`, which the timeout covers.)

---

## Worker-triggered rows

A worker resolving tic `N+1` work queues any follow-on at **tic `N+2` or later** (never the tic
it's processing), appended inside the same atomic commit that advances the row. Dedup is
therefore structural — a re-resolve finds the row complete (skip) or redoes the uncommitted
bundle — so no provenance key is needed *for dedup*. (The `(source_shard, event_reference)` pair
is still carried for the **cross-shard** idempotency key above, and for audit.)
