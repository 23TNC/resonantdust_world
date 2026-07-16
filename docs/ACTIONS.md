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

Append below. Each entry must state, per operand, whether it is **written** (in the write set → its
slot is grouped/claimed) or **read** (in the read set → the worker blocks on it settling).

| action | value | arity | signature |
|---|---|---|---|
| `CREATE` | 6 | 2 | `def:definition_reference` (imm) · `position:position_reference` (imm) → **mints** a new entity, returns its `entity_reference`. The written target is the *minted* id, not an operand. |
| `PLACE` | 7 | 2 | `obj:entity_reference` (**write**) · `position:position_reference` (imm) — set `obj`'s position absolutely. |
| `MOVE_TO` | 8 | 2 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) — step `obj` one tile toward `dest`, then queue the next hop. |

**`CREATE`.** A spawn insert isn't idempotent by value, so replay must not double-spawn. **OPEN**: how
the minted `entity_reference` survives replay — a small spawn-log on the data shard keyed by
`(event_reference, index) → minted id` (replay reads it) is the leaning, rather than deriving a
24-bit `object_reference` from a 32-bit `event_reference`. Which data shard the new object lands on
(spawn `position`'s zone) also needs pinning. The new entity is its own singleton component (nothing
references it yet), so it never conflicts at grouping.

**`PLACE`** is the absolute set. It is where the `state.macro_position_reference` invariant is
enforced: writing a position in a new zone must promote the row with the **new** zone key, or the
object goes invisible where it moved to. A cross-zone place moves the object between subscriptions.

**`MOVE_TO`** reads `obj`'s current position (so a mid-move `PLACE` isn't overrun — `src` is not an
operand, it's read), steps one tile toward `dest`, writes the new position, and **queues a
continuation** `MOVE_TO obj dest` for the tic the object reaches the next tile. See §Movement.

---

## Movement, and the client's tic estimate

`MOVE_TO` is a self-perpetuating chain: each hop writes one tile and queues the next, until `dest`.
Two capabilities it needs, both **OPEN**:

- **A verb that queues an event.** `MOVE_TO`'s effect includes appending a future `MOVE_TO` to
  `event_log`. This is the continuation primitive — baked into the verb for now, not a general `QUEUE`
  action.
- **Queue-at-a-future-tic.** The next hop lands `k` tics out (`k` = tics-per-tile from the object's
  speed). So `queue` generalizes from `event_tic = master + 3` to `event_tic ≥ master + 3`. The
  completeness barrier still holds — a tic's set is frozen at `T-2` regardless of *when* its events
  were born, and the shard accepts an event for `V` while `master ≤ V-3`. **Speed has no home yet**:
  tics-per-tile is per-kind (content-derived from `definition_reference`), and nothing stores it.

**Don't promote every hop.** Promoting `state` on each tile is exactly the per-tile fan-out we're
avoiding — one `state` upsert per tile, streamed to every subscriber. Instead:

- **`PROMOTE_EVENT` once**, on the initial `MOVE_TO`, announces the move: the client now knows `obj`
  is heading to `dest`.
- **`PROMOTE_STATE` sparingly** — seed the start, then re-anchor periodically to correct drift, and
  land the final tile. Not every continuation.

So the initial program is `MOVE_TO obj dest PROMOTE_STATE PROMOTE_EVENT`; the self-queued
continuations are bare `MOVE_TO obj dest`, promoting only on the re-anchor cadence (**OPEN**: every N
tiles? first + last only?).

**The client speculates without a synced tic.** We never synced a tic/clock with the server — `state`
is simply the latest authoritative truth. `PROMOTE_EVENT` on a move gives the client the one thing
`state` can't: *intent* (`obj → dest`). The client anchors "event for tic `V` arrived at wall-time
`W`", and with the known `TIC_HZ` extrapolates elapsed tics from `W` — no absolute tic needed, just
*elapsed*, which its own clock gives. Each `state`/`event` arrival refines a loose wall↔tic mapping
(implicit sync from the data stream, not the ping/pong that never worked). The estimate drifts; the
sparse `PROMOTE_STATE` re-anchors it. Best-effort by construction — the server dictates truth, the
client makes it smooth.

---

## Deriving the sets

**Write set** — scan the stream; for each action, its signature names which operands are written
`entity_reference`s. **Read set** — the same scan, the operands the signature marks read. No
interpretation, no game semantics in the spine: the signature is a table lookup.

> ⚠️ **A `POP`'d operand has no value until run time.** The write set must be known at **grouping**
> (the event shard unions by shared target, before the program runs). A written target that arrives
> via `POP` is therefore not statically knowable, unless it traces to an `ECHO` literal. See
> [`notes/actions.md`](notes/actions.md) — this constrains what a legal program is.
