# SpacetimeDB tables — how a shard is *supposed* to work

This folder is the **authoritative design** for the shard module's tables: the event log,
the tick working set, the hot `state`, and the cold store. It describes the *intended*
model — the one we spent hours on — not necessarily what the code does today. Where the
code diverges, that's a bug in the code (or in this doc), tracked in
[divergences.md](divergences.md).

## Why this folder exists

So the plan stops getting lost. The rule going forward:

1. **This folder is the source of truth.** If you (Claude) are about to implement or
   change how a shard table works, read this folder first and conform to it. Do not
   re-derive the design from the current code — the code is the thing being corrected.
2. **If the design changes, this folder changes in the same breath** — before or with the
   code, never after-the-fact only.
3. **If the code and this folder disagree, that is a defect.** The user can point at this
   folder and say "make the code match" *or* "the doc is stale, fix the doc" — and either
   is a well-defined task because the intent lives here.

## Status legend

Every claim in these docs is tagged so you know how much to trust it:

- ✅ **AGREED** — settled; recorded in [`object-model.md`](../object-model.md) "Decided"
  or stated directly by the user.
- ✏️ **DRAFT** — my reconstruction of what we discussed; **confirm before relying on it.**
- ❓ **OPEN** — genuinely undecided; needs a call.
- ⚠️ **CODE DIVERGES** — the implementation today does *not* match this; see
  [divergences.md](divergences.md).

## Index

- **[tables.md](tables.md)** — the table taxonomy: hot side (`state` + `state_log`), cold
  side (`cold`), the metronome/meta tables, and the generic-engine principle (the shard
  never inspects the payload).
- **[hot-cold.md](hot-cold.md)** — the hot/cold model, per-shape modules (tiles/pawns/things,
  each hot+cold), and why cold→hot mint is **absorbed into enqueue** (no `MINT`/`GET` verb);
  `pack` (settle) is an execute op.
- **[events.md](events.md)** — the event log carries `actions : Vec<u64>`; the shape of the
  row and where processing lives (the worker, not the shard).
- **[event-dsl.md](event-dsl.md)** — the **stack DSL**: each word is one `u64`, a worked
  two-row plan (move → await inspect) with `AWAIT`/`TIMEOUT`/`?:`/`FAIL`, the `u64` wire
  encoding, and why the worker becomes a generic interpreter.
- **[lifecycle.md](lifecycle.md)** — **how an event resolves safely**: the event-shard/data-shard
  split, the two-phase recoverable lifecycle (`enqueue → … → complete`), the drop-on-miss
  **drop-on-miss** causality, ≤T−1 reads, deterministic composition, convergent cross-shard writes, refcounted GC, and crash
  recovery. The correctness core.
- **[divergences.md](divergences.md)** — the honest ledger: what the code does today vs
  this design, per table, with the fix.
- **[risks.md](risks.md)** — trade-offs and what could bite: build-a-slice-first, determinism,
  the drop-as-real-time-deadline trade, latency, and what's already been closed.
- **[../spacetime-implementation/](../spacetime-implementation/README.md)** — the staged
  build plan that closes those divergences (its own folder: S0 → S7, one file per stage).

## The load-bearing ideas (read these first)

1. ✅ **One generic engine, any payload.** A shard is `decl_tick_pipeline!` invoked with a
   payload field list. The scheduling spine (event/state/resolve/GC/fence) never inspects
   the payload — it copies game fields verbatim. One engine serves every shard class.
2. ✅ **Hot and cold live behind one system; the worker treats them uniformly.** A per-shape
   module (tiles/pawns/things) carries a hot side (ticking `state`) and a cold side (settled
   `cold`). Cold→hot mint is **absorbed into enqueue** (you can't stand up work for a
   `cold_reference`, so standing it up *is* the mint) — no special-cased cold path at execute.
   *(The point the code missed.)*
3. ✅ **An event carries a DSL *program*, and the worker runs it.** The event log stores a
   `Vec<u64>` stack program (actions are *written* in a small DSL, not enumerated in the
   engine); references are its operands. The shard is a dumb store + a metronome + a fence;
   **the worker is the VM.** No per-action Rust branch to add for a new verb.
   See [event-dsl.md](event-dsl.md).
4. ✅ **Nothing modifies a settled tic.** Work is right-shifted (data at N, execute at N+1,
   enqueue at N+2, request at N+3), so a resolve only *adds* immutable resolved state. Writes
   only land at the frontier and reads only touch the sealed past, so "write behind a read" is
   **structurally impossible** — no watermark; the master just **drops** any row that misses its
   enqueue window. Every step is crash-recoverable by idempotent re-drive, and GC is dumb
   (refcounted). The correctness core — see [lifecycle.md](lifecycle.md).
