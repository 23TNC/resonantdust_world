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
- **`promote_*`** — palette entries.
- **Target extraction** — the same scan.

And it made a word a whole u32, which the old tagged frame couldn't: `op_code:4 +
entity_reference:32 = 36` doesn't fit a u32, so tagging forced either a u64 word with 28 bits spare
or a reference narrower than a reference.

The stack machine (`PUSH` / `POP` / `ECHO`) it also defined is now **deferred** — see B-3.

---

## B-3 · `POP`'d targets can't be declared — DISSOLVED 2026-07-16

Dropped the stack (`PUSH` / `POP` / `ECHO`) for now — the current verbs don't compose in-program, so
every operand is a **literal** and every written target is spelled out and known at grouping. No
`POP`, no dynamic target, no problem.

If the stack ever returns (an additive palette entry), so does this constraint, and the resolution is
the honest default already worked out: a `POP` is fine for a number or a read operand, never for a
written `entity_reference`. See [`notes/actions.md`](../../notes/actions.md).

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

---

## B-5 · The first verbs — CREATE / PLACE / MOVE_TO — NOT A BLOCKER (2026-07-16)

Mis-filed. A blocker needs *your* input and gates work; B-5's items are mostly mine to decide, and
**none gate current work** — `PLACE` and `CREATE` are buildable now, `MOVE_TO` is far off, and the
shard plumbing (W2/W3) touches no verb. Sorted:

**Decided (engineering, mine):**
- **`CREATE`'s replay-safe id → spawn-log.** A small `data_shard` table keyed by `(event_reference,
  index) → minted entity_reference`; replay reads it instead of re-minting. (Adds one internal table
  to `data_shard` *when `CREATE` is built* — not before.) The new object lands on the data shard that
  owns the spawn `position`'s zone.
- **`MOVE_TO` bakes in the queue.** Its continuation appends a future `MOVE_TO` directly; no general
  `QUEUE` action yet.
- **`event_tic ≥ master + 3`** — confirmed. The barrier is unaffected (a tic freezes at `T-2`
  regardless of when its events were born), so a hop can land further out.

**Deferred (content/tuning, settle when `MOVE_TO` renders — has a safe default, not blocking):**
- **Speed** is per-kind, from `definition_reference`; the actual tics-per-tile values are content
  authoring.
- **`PROMOTE_STATE` cadence** defaults to first + final + every N tiles; the N is a bandwidth-vs-
  smoothness knob to tune against a running client.

These now live as notes in [`ACTIONS.md`](../../ACTIONS.md) §World / §Movement, not as a blocker.
