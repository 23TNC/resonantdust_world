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
| `PROMOTE` | 1 | 0 | **prefix modifier** — set the promote bit for the **next** action's writes |
| `PROMOTE_EVENT` | 2 | 0 | project this event to `event` on settle — **tabled** until movement |

**`PROMOTE` is a prefix, not a target-latch.** The stream runs left-to-right; `PROMOTE` (arity 0) sets
a promote bit in scratch, and the **next** action consumes it — writing the promote flag as part of
*its own* result, per target (both `a` and `b`). So `promote place obj dest`, `promote init_zone
macro` — the worker knows *during* the `place`/`init_zone` that its write should promote, and passes
the bit to the write reducer. No post-pass reasoning about what a trailing promote "meant."

**Promote is smart + atomic — it copies whatever changed, in one transaction.** The write reducer,
given the bit, promotes **both** `entity_state` (from `entity_state_log`) and `overlay` (from
`overlay_log`), but only where `visible.tic != log.tic` — so it syncs exactly the table(s) the action
touched, without the worker figuring out which shard/table. Because both project in the **same reducer
transaction**, a fold's baseline update + overlay clear land on the client **atomically** — there is
**no flash and no promote-ordering to get wrong** (atomicity replaces the old `PROMOTE_COLD`-before-
`PROMOTE_STATE` rule). A hot shard has no `overlay`, so its promote copies `entity_state` only.

`PROMOTE_EVENT` is unchanged in spirit (latch `event_status.flags.PROMOTE` at `queue`) but **tabled**
— revisited with movement. A program carrying no `PROMOTE` runs entirely server-side.

**Cell addressing — `tile_reference` resolves per row form.** An action's cell operand resolves against
the target's baseline form: on a **`dense`** row `tile_reference` is a **direct index** into `items`
(`ZONE_DIM²` allocated); on a **`sparse`**/`overlay` row it **matches** the item carrying that
`tile_reference`. The worker + actions are written against the generic macros, so this is the one place
the forms differ. See [`VARIABLES.md § Cold storage`](VARIABLES.md).

### World

Append below. Each entry must state, per operand, whether it is **written** (in the write set → its
slot is grouped/claimed) or **read** (in the read set → the worker blocks on it settling).

| action | value | arity | signature |
|---|---|---|---|
| `CREATE` | 3 | 2 | `def:definition_reference` (imm) · `position:position_reference` (imm) → **mints** a new entity. The written target is the *minted* id, not an operand. |
| `PLACE` | 4 | 2 | `obj:entity_reference` (**write**) · `position:position_reference` (imm) — set `obj`'s position absolutely. |
| `MOVE_TO` | 5 | 2 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) — step `obj` one tile toward `dest`, then queue the next hop. |
| `SET` | 6 | 5 | `cold_row:cold_row_reference` (**write**) · `type_id` (imm) · `tile_reference` (imm) · `kind_reference` (imm, `0`=clear) · `data` (imm) — **override one cold cell** through the `overlay` tier. The `cold_row` is spelled (so concurrent `SET`s to one row **group** — see routing below) and `type_id` names the shard (a `cold_row_reference` carries no type nibble). Replaces the old per-cell `cold_entity_reference` SET. |
| `init_zone` | *tbd* | *tbd* | build a zone's **whole baseline row** in scratch (worldgen) and write it to `entity_state_log` (`ColdBaseline` tier). `promote init_zone` projects it visible. The event-driven replacement for the direct `seed`. Payload is a whole row (F12 — event-carried `Vec` vs worker-side worldgen, decided at build). |
| `PACK` | *tbd* | *tbd* | fold a zone's settled `overlay` cells into its `entity_state_log` row (the GC write-back). Operands settle when built — likely the target `cold_row_reference` (**write**); the worker reads the row's settled `overlay` cells and composes the new baseline. GC queues `promote pack …` — the smart, atomic `PROMOTE` projects the folded `entity_state` **and** cleared `overlay` in one commit. |

**Cold routing (F11).** Hot targets (pawns) route to their shard by their own `server_reference`
nibble. A **cold** target is a `cold_row_reference` with no type nibble, so grouping still unions by the
spelled `cold_row` (`write_targets`) but **routing** reads the action: `codec::target_routes` pairs each
target with `Hot | ColdBaseline{type_id} | ColdOverlay{type_id}` — the claimer + worker pick the shard
**and** the `claim`/`write` (baseline) vs `claim_overlay`/`write_overlay` (overlay) reducer from it.
`SET`→`ColdOverlay`; `init_zone`/`PACK`→`ColdBaseline`.

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

> **Tabled.** This section predates the `PROMOTE`-prefix change and still writes `PROMOTE_STATE` /
> `PROMOTE_EVENT` postfix in its example programs. Movement (and `PROMOTE_EVENT`) is revisited as its
> own pass; read the promotes here as "promote this" pending that rework.

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
