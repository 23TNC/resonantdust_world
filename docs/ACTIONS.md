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

Every operand is a **literal** u32 — an `entity_reference`, a `position_reference`, a
`definition_reference`, or a plain number, per the action's signature. No stack, no indirection: a
written target is always spelled out, so it's known at grouping.

> The stack machine (`PUSH` / `POP` / `ECHO`, using one action's output as another's operand) is
> **deferred** — not needed by the current verbs, and it was the sole source of B-3 (a `POP`'d target
> isn't statically known). Recover it from `git show` if composition is ever wanted; see
> [`notes/actions.md`](notes/actions.md).

---

## Palette

`action_reference : u32`. **Append-only once a program is stored** — until then (nothing is built)
the numbers are free to renumber. Aliases are for reading; the wire is the number.

### Machine / visibility

| action | value | arity | signature |
|---|---|---|---|
| `NONE` | 0 | 0 | reserved null |
| `PROMOTE_STATE` | 1 | 1 | `target:entity_reference` — project this slot to `state` once settled |
| `PROMOTE_EVENT` | 2 | 0 | project this event to `event` on settle |

Both are latched, not executed: `PROMOTE_EVENT` sets `event_status.flags.PROMOTE` at `queue`;
`PROMOTE_STATE` sets `state_status.flags.PROMOTE` when the target's slot is created (`claim`). A
program carrying neither runs entirely server-side.

### World

Append below. Each entry must state, per operand, whether it is **written** (in the write set → its
slot is grouped/claimed) or **read** (in the read set → the worker blocks on it settling).

| action | value | arity | signature |
|---|---|---|---|
| `CREATE` | 3 | 2 | `def:definition_reference` (imm) · `position:position_reference` (imm) → **mints** a new entity. The written target is the *minted* id, not an operand. |
| `PLACE` | 4 | 2 | `obj:entity_reference` (**write**) · `position:position_reference` (imm) — set `obj`'s position absolutely. |
| `MOVE_TO` | 5 | 2 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) — step `obj` one tile toward `dest`, then queue the next hop. |

**`CREATE`.** A spawn insert isn't idempotent by value, so replay must not double-spawn. **Resolved:**
a small **spawn-log** on the data shard, keyed by `(event_reference, index) → minted entity_reference`
— replay reads it instead of re-minting (a 32-bit `event_reference` can't derive a 24-bit
`object_reference`, so it's recorded, not computed). The object lands on the data shard owning the
spawn `position`'s zone. It's its own singleton component (nothing references it yet), so it never
conflicts at grouping. The spawn-log table is added to `data_shard` *when `CREATE` is built*.

**`PLACE`** is the absolute set. It is where the `state.macro_position_reference` invariant is
enforced: writing a position in a new zone must promote the row with the **new** zone key, or the
object goes invisible where it moved to. A cross-zone place moves the object between subscriptions.

**`MOVE_TO`** reads `obj`'s current position (so a mid-move `PLACE` isn't overrun — `src` is not an
operand, it's read), steps one tile toward `dest`, writes the new position, and **queues a
continuation** `MOVE_TO obj dest` for the tic the object reaches the next tile. See §Movement.

---

## Movement, and the client's tic estimate

`MOVE_TO` is a self-perpetuating chain: each hop writes one tile and queues the next, until `dest`.
Two capabilities it uses:

- **A verb that queues an event.** `MOVE_TO`'s effect appends a future `MOVE_TO` to `event_log` —
  baked into the verb, not a general `QUEUE` action (yet).
- **Queue-at-a-future-tic.** The next hop lands `k` tics out (`k` = tics-per-tile from the object's
  speed). So `queue` accepts `event_tic ≥ master + 3`, not exactly `+3`. The completeness barrier is
  unaffected — a tic's set is frozen at `T-2` regardless of *when* its events were born, and the shard
  accepts an event for `V` while `master ≤ V-3`. Speed is per-kind (from `definition_reference`); the
  tics-per-tile values are content, settled when `MOVE_TO` is built.

**Don't promote every hop.** Promoting `state` on each tile is exactly the per-tile fan-out we're
avoiding — one `state` upsert per tile, streamed to every subscriber. Instead:

- **`PROMOTE_EVENT` once**, on the initial `MOVE_TO`, announces the move: the client now knows `obj`
  is heading to `dest`.
- **`PROMOTE_STATE` sparingly** — seed the start, then re-anchor periodically to correct drift, and
  land the final tile. Not every continuation.

So the initial program is `MOVE_TO obj dest PROMOTE_STATE PROMOTE_EVENT`; the self-queued
continuations are bare `MOVE_TO obj dest`, promoting only on the re-anchor cadence. Default: first +
final + every N tiles; the `N` is a bandwidth-vs-smoothness knob, tuned against a running client when
`MOVE_TO` is built.

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

Every operand is a literal, so both sets are fully known at grouping — the whole point of dropping the
stack. (`CREATE`'s written target is the *minted* id, not an operand; a fresh entity is its own
singleton component, so it never conflicts.)
