# Blockers — spacetime-again

_Needs your input. Open → resolved; resolved rows keep a date._

---

## B-1 · The word format for `actions : Vec<u32>` — RESOLVED 2026-07-15

**[`ACTIONS.md`](../../ACTIONS.md).** The action leads and its arity says how many u32s follow and
what they are — so an operand needs no tag, which is what replaces the reference type. `ECHO` /
`PUSH` / `POP` are the machine; `PROMOTE_STATE` / `PROMOTE_EVENT` are latched, not executed.

It closed all three riders at once:
- **Read set** — the action's *signature* declares, per operand, written vs read. Both sets fall out
  of one scan, no `action_reads_actor`, no game semantics in the spine.
- **`promote_*`** — palette entries 4 and 5.
- **Target extraction** — the same scan.

And it made a word a whole u32, which the old tagged frame couldn't: `op_code:4 +
entity_reference:32 = 36` doesn't fit a u32, so tagging forced either a u64 word with 28 bits spare
or a reference narrower than a reference.

**It exposed one new constraint** — see B-3.

---

## B-3 · `POP`'d targets can't be declared — OPEN

**Blocks:** the shape of a "legal program", and W2's grouping. Not the store schemas.

**Why.** The write set must be known at **grouping** (N=0–1): the event shard unions events by shared
write-target, and the orchestrator forms components from those targets — all before the program runs
(N=2+). A `POP`'d operand has no value until **run time**. So a program whose written target arrives
off the stack cannot be grouped — the whole model assumes the write set is static.

**Three ways out** (fuller in [`notes/actions.md`](../../notes/actions.md)):

| | |
|---|---|
| **targets must be literal** | `POP` is fine for numbers and read operands, never for a written `entity_reference`. Costs "damage whatever the last action returned". |
| **static-fold `ECHO`-sourced pops** | a pass tracks the stack: a `POP` tracing to `ECHO <lit> PUSH` is knowable; one tracing to an action's *output* still isn't, so the rule is still needed. |
| **over-approximate** | group a superset. Pulls entities the event never writes into the component — bigger components, more serialization. |

**Recommendation:** the first. The enqueue/execute split is what buys the design its determinism,
and a dynamically-targeted event is asking to opt out of it. Cheap to relax later; expensive to
retrofit the other way.

---

## B-2 · Two events on one `(entity, tic)` — DISSOLVED 2026-07-16

The orchestrator dissolved it. Two events touching the same entity **share a component** (union-find
by shared target), so they go to **one worker**, which composes them sequentially in ascending
`event_reference` order in local scratch. There is no second writer, so nothing to order at the store,
no first-arrival race. The determinism is `event_reference` (already a global total order); the
isolation is one-worker-per-component. `dirty` reverts to a boolean.

This is the same resolution that made cross-entity transactions and conditionals into ordinary code —
all three were the same problem (no agent saw the whole conflict set), and the orchestrator gives one
agent the whole component. See [`intent/spacetime-again/`](../../intent/spacetime-again/README.md)
§Why 1.

---

## B-4 · The cross-shard read block — RESOLVED 2026-07-16 (as a worker requirement)

`A += B` reads B; B may be on a different shard than A; so A's shard **cannot** enforce "B settled" —
a local fence sees only A's own chain. A worker that skips the block reads a stale B, writes a
wrong-but-clean A, and the next tic composes on the corruption. There is no store-side fix that stays
one-reducer-atomic.

**Resolution:** the block is a **worker correctness requirement**, not a store fence. The worker is
subscribed to every target it reads, so it *can* check, and must — defer the whole component if any
read target's previous row is dirty; never partial-write. This is safe because workers are our own
code (a trusted-server model). The enforceable-without-trust alternative is two-phase (write all
tentative → verify all settled → clear dirty), at more round trips; we take "block correctly" for now.
Recorded in [`TABLES.md`](../../TABLES.md) §"The block is a worker requirement" and intent §Why 4.
