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

## The stack machine is deferred (and why it dissolved B-3)

`PUSH` / `POP` / `ECHO` — using one action's output as another's operand — are dropped for now. The
current verbs (`CREATE` / `PLACE` / `MOVE_TO`) never compose in-program: each takes literal operands
and stands alone. So every operand is a literal, and the whole "an operand is a literal *or* a `POP`"
interpretation goes away.

That kills **B-3** at the root. The write set must be known at *grouping* (the event shard unions by
shared write-target before the program runs), and a `POP`'d target had no value until run time — so a
dynamically-targeted event couldn't be grouped. No `POP`, no dynamic target, no problem: every written
`entity_reference` is spelled out and statically known.

**If composition is ever wanted back**, the machine returns as an *additive* palette entry (its own
`action_reference`), and the rule it needs is the honest default from the B-3 analysis: a `POP` is
fine for a *number* or a *read* operand, never for a *written* `entity_reference` — because a written
target must survive to grouping. `ECHO` was the escape hatch that let an arbitrary value (even one
colliding with `POP`'s sentinel) reach the stack; it's only meaningful alongside `POP`. Recover the
full stack design from `git show`.

## Why the signature carries read-vs-write

This is what closes the read-set question the design's §Open has carried since `targets`/`reads`
stopped being columns. Both sets fall out of one scan, because the action's signature is a table
lookup — the spine never has to interpret a verb to know what it touches.

The alternative was `action_reads_actor`-shaped knowledge: a function that knows, per verb, which
operands are read. That is game semantics living in the scheduler. The signature puts the same fact
in a declaration instead.

## Palette discipline

**Append-only.** A stored program is a `Vec<u32>` of numbers; renumbering an action silently
changes what every settled `event` row means. Retire by leaving the value dead, never by reusing it.

Arity is part of the wire, not a convention — a reader with the wrong arity mis-frames the rest of
the stream, and there is no re-sync point. Changing an existing action's arity is a wire break, the
same class as changing a bit layout.
