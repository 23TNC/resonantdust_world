# Actions

> **AUTHORITATIVE** for the event program — the encoding of `event_log.actions : Vec<u32>`, the
> action palette, and each action's arity. Anything that disagrees is the bug.
> Rationale and what it rules out: [`notes/actions.md`](notes/actions.md).

An event carries a program. `Vec<u32>` throughout — `action_reference`, `entity_reference` and
plain numbers are all u32; the stream says which is which.

---

## The stream

```
actions := (action operand{arity(action)})*
```

**An action always comes first.** Its `action_reference` names its **arity** — how many u32s follow
— and its **signature** — what each of them is. Read the action, read exactly that many operands,
repeat until the vector is empty. One program may hold several actions.

This is what replaces the reference type: an operand isn't tagged, the action already said what it
is.

## Operands

Each operand slot holds **either** a literal (an `entity_reference`, a number) **or** `POP`.

`POP` in a slot means *take this operand off the stack* instead of reading it inline. So
`<action> POP POP` runs the action with both operands popped.

## The stack

| | | |
|---|---|---|
| `PUSH` | arity 0 | push the **previous action's output** onto the stack |
| `POP` | — | an operand marker, not a standalone action. Consumes one stack entry. |
| `ECHO` | arity 1 | return the next u32, **read raw** |

`ECHO 5 PUSH` puts the literal `5` on the stack.

**`ECHO`'s operand is the one slot that is never interpreted.** That is what it's for: any value —
including one that would collide with `POP`'s sentinel — reaches the stack through `ECHO`, and from
the stack into any slot via `POP`. Without it the encoding would have values it cannot express.

---

## Palette

`action_reference : u32`. **Append-only** — a new action goes last so stored programs never
renumber. Aliases are for reading; the wire is the number.

### The machine

| action | value | arity | signature |
|---|---|---|---|
| `NONE` | 0 | 0 | reserved null |
| `ECHO` | 1 | 1 | `raw:u32` → returns it |
| `PUSH` | 2 | 0 | pushes the previous action's output |
| `POP` | 3 | — | operand marker only |

### Visibility

| action | value | arity | signature |
|---|---|---|---|
| `PROMOTE_STATE` | 4 | 1 | `target:entity_reference` — project this slot to `state` once settled |
| `PROMOTE_EVENT` | 5 | 0 | project this event to `event` on settle |

Both are latched, not executed: `PROMOTE_EVENT` sets `event_status.flags.PROMOTE` at `queue`;
`PROMOTE_STATE` sets `state_status.flags.PROMOTE` when the target's slot is created (`claim`). A
program carrying neither runs entirely server-side.

### World

Append below. Each entry must state, per operand, whether it is **written** or **read** — that is
what the write set and the read set are derived from, and neither is a column.

| action | value | arity | signature |
|---|---|---|---|
| *(none yet)* | 6.. | | |

---

## Deriving the sets

**Write set** — scan the stream; for each action, its signature names which operands are written
`entity_reference`s. **Read set** — the same scan, the operands the signature marks read. No
interpretation, no game semantics in the spine: the signature is a table lookup.

> ⚠️ **A `POP`'d operand has no value until run time.** The write set must be known at **enqueue**
> (T=1) to declare slots, one tic before the program executes. A target that arrives via `POP` is
> therefore not statically knowable, unless it traces to an `ECHO` literal. See
> [`notes/actions.md`](notes/actions.md) — this constrains what a legal program is.
