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

**Blocks:** W4, and the shape of a "legal program". Not W1–W3.

**Why.** The write set must be known at **enqueue** (T=1): `declare_pending` stands up the
`(entity, tic)` slots one tic before the program runs, so `dirty` is right and composition can be
ordered. A `POP`'d operand has no value until **run time** (T=2). So a program whose target arrives
off the stack cannot have its slots declared — the whole enqueue/execute split assumes the write set
is static.

**Three ways out** (fuller in [`notes/actions.md`](../../notes/actions.md)):

| | |
|---|---|
| **targets must be literal** | `POP` is fine for numbers and read operands, never for a written `entity_reference`. Costs "damage whatever the last action returned". |
| **static-fold `ECHO`-sourced pops** | a pass tracks the stack: a `POP` tracing to `ECHO <lit> PUSH` is knowable; one tracing to an action's *output* still isn't, so the rule is still needed. |
| **over-approximate** | declare a superset, withdraw what wasn't touched. Burns slots on entities the event never writes — contention the four-slot cap will feel. |

**Recommendation:** the first. The enqueue/execute split is what buys the design its determinism,
and a dynamically-targeted event is asking to opt out of it. Cheap to relax later; expensive to
retrofit the other way.

---

## B-2 · Two events on one `(entity, tic)` — OPEN

**Blocks:** nothing yet. W3's `apply` will need an answer, but the schema doesn't change either way
if the answer is "can't happen" or "last write wins".

**The question.** Ordering *across* tics is settled: a row at T can't be written while an earlier tic
for that entity is dirty, and the worker sees that because it holds a slot on every row for its
targets. *Within* one tic it isn't. Two players hit the same door at tic T: both events carry
`event_tic = T`, both target the door, `dirty(door, T) = 2`, and both unblock the moment T-1 settles.
Whichever `apply` lands first wins, and the outcome depends on network timing.

**Three answers, any of which is fine — they just need choosing:**

| | |
|---|---|
| **can't happen** | the edge or `declare_pending` rejects a second event on an occupied `(entity, tic)`. `dirty` is then only ever 0 or 1 and nothing more is needed. |
| **last write wins** | accept the non-determinism. Cheapest; means the same inputs can produce different worlds. |
| **order by `event_reference`** | the events compose in ascending order — deterministic. Needs *something* per slot that says which is next; `dirty` counts, it doesn't order. |

**Why it's yours:** it's a gameplay-determinism call, not a schema one. The old design took the third
answer and called it "the one property that must not be broken" — but that was a different machine,
and I've been wrong once already assuming its reasoning carried over.
