# Notes — actions

Supporting material for [`../ACTIONS.md`](../ACTIONS.md), which is authoritative and carries the
encoding alone. If the two disagree, ACTIONS.md wins.

## Why the action leads

The old frame tagged every word (`op_code:4 | reserved:12 | server_reference:16 | payload:32`), so a
reader could pick up any u64 and know what it was. That cost 4 bits on every word to answer a
question only the *first* word of each instruction actually asks.

Leading with the action instead: the action's arity says how many u32s follow and the signature says
what each is, so the operands need no tag at all. It is the same trick that let `entity_reference`
drop `reference_id` — don't tag the value, know it from context. Here the context is the action.

The payoff is that a word is a **whole u32**, which is exactly what an `entity_reference` needs. The
old frame couldn't do that: `op_code:4 + entity_reference:32 = 36` doesn't fit a u32, so a tagged
encoding forced either a u64 word (28 bits wasted) or a reference narrower than a reference.

## Why `ECHO` exists

It looks redundant — why say `ECHO 5 PUSH` when `5` is right there? Because an operand slot is
**interpreted**: it's a literal *or* `POP`. So the value that `POP`'s sentinel occupies cannot be
written inline. Any encoding where operands are interpreted has values it can't express, unless
there is one slot that is never interpreted.

`ECHO`'s operand is that slot. Read raw, always. So the unexpressible value goes
`ECHO <value> PUSH` onto the stack, and reaches any operand position via `POP`. The escape hatch is
what makes the interpreted slot safe.

It also means `POP`'s numeric value doesn't have to be chosen carefully to avoid colliding with real
data — a constraint that would otherwise leak into `entity_reference` allocation.

## The one real constraint: `POP`'d targets

The write set must be known at **enqueue** (T=1). `declare_pending` stands up the `(entity, tic)`
slots one tic *before* the program runs, so `dirty` is right and the composition can be ordered. But
a `POP`'d operand has no value until run time (T=2).

So a program whose target arrives via `POP` cannot have its slots declared. Three ways out, and this
needs deciding before W4:

| | |
|---|---|
| **targets must be literal** | simplest. `POP` is allowed for numbers and read operands, never for a written `entity_reference`. Costs expressiveness: no "damage whatever the last action returned". |
| **static-fold `ECHO`-sourced pops** | a pass tracks the stack; a `POP` that traces to `ECHO <literal> PUSH` is knowable, one that traces to an action's *output* is not. Buys a little, and the "is not" case still needs a rule. |
| **over-approximate** | declare a superset and withdraw what wasn't touched. Costs slots on entities the event never writes, which is contention the four-slot cap will feel. |

The first is the honest default: the enqueue/execute split is what buys the whole design its
determinism, and a dynamically-targeted event is asking to opt out of it.

## Why the signature carries read-vs-write

This is what closes the read-set question the design's §Open has carried since `targets`/`reads`
stopped being columns. Both sets fall out of one scan, because the action's signature is a table
lookup — `PUSH` and `POP` are structure, not semantics, and the spine never has to interpret a verb
to know what it touches.

The alternative was `action_reads_actor`-shaped knowledge: a function that knows, per verb, which
operands are read. That is game semantics living in the scheduler. The signature puts the same fact
in a declaration instead.

## Palette discipline

**Append-only.** A stored program is a `Vec<u32>` of numbers; renumbering an action silently
changes what every settled `event` row means. Retire by leaving the value dead, never by reusing it.

Arity is part of the wire, not a convention — a reader with the wrong arity mis-frames the rest of
the stream, and there is no re-sync point. Changing an existing action's arity is a wire break, the
same class as changing a bit layout.
