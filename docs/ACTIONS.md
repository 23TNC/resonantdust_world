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
| `PROMOTE_EVENT` | 2 | 0 | **prefix-style flag** — latch `event_status.flags.PROMOTE` at `queue`; the event projects to `event` on settle (the movement INTENT channel, §Movement) |

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

`PROMOTE_EVENT` (arity 0, latched at `queue` into `event_status.flags.PROMOTE`) marks the event
for projection to the client-visible `event` table at settle — the **intent** channel movement
broadcasts on (§Movement). A program carrying neither promote runs entirely server-side.

**Cell addressing — `tile_reference` resolves per row form.** An action's cell operand resolves against
the target's baseline form: on a **`dense`** row `tile_reference` is a **direct index** into `items`
(`ZONE_DIM²` allocated); on a **`sparse`**/`overlay` row it **matches** the item carrying that
`tile_reference`. The worker + actions are written against the generic macros, so this is the one place
the forms differ. See [`VARIABLES.md § Cold storage`](VARIABLES.md).

### World

Append below. Each entry must state, per operand, whether it is **written** (in the write set → its
slot is grouped/claimed) or **read** (in the read set → the worker blocks on it settling).

> **Designing a verb IS distributed-systems design.** The write set determines grouping: events
> sharing a written target merge — transitively — into one conflict-component, and a component runs
> on ONE worker. A verb with a broad write set welds unrelated events together and serializes them;
> a crowded plaza full of such events becomes one giant component on one core. So: keep write sets
> **minimal**, mark an operand **read** unless the verb truly mutates it, and never write what you
> only inspect. This rule is the system's real scalability lever — sharding cannot undo a verb that
> over-claims. (User-ratified 2026-07-28.)

| action | value | arity | signature |
|---|---|---|---|
| `CREATE` | 3 | 2 | `def:definition_reference` (imm) · `position:position_reference` (imm) → **mints** a new entity. The written target is the *minted* id, not an operand. |
| `PLACE` | 4 | 2 | `obj:entity_reference` (**write**) · `position:position_reference` (imm) — set `obj`'s position absolutely. |
| `MOVE_TO` | 5 | 2 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) — the CLIENT-issued move verb, always the SEED: stamps the trip-serial, turns facing toward the path, promotes the current position when `PROMOTE`-prefixed, steps NOTHING. The worker chains `MOVE_STEP` hops from it — §Movement. |
| `SET` | 6 | 5 | `cold_row:cold_row_reference` (**write**) · `type_id` (imm) · `tile_reference` (imm) · `kind_reference` (imm, `0`=clear) · `data` (imm) — **override one cold cell** through the `overlay` tier. The `cold_row` is spelled (so concurrent `SET`s to one row **group** — see routing below) and `type_id` names the shard (a `cold_row_reference` carries no type nibble). Replaces the old per-cell `cold_entity_reference` SET. |
| `INIT_ZONE` | 7 | variable | `cold_row:cold_row_reference` (**write**) · `type_id` (imm) · `count` (imm) · `item×count` (imm) — build a zone's **whole baseline row** and write it to `entity_state_log` (`ColdBaseline` tier); `PROMOTE INIT_ZONE …` projects it visible. The ONE variable-arity verb (F12). Built. |
| `MOVE_STEP` | 8 | 3 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) · `serial` (imm) — one chain hop, **WORKER-ONLY** (the edge rejects it from clients — movement-hardening F2): step `obj` one tile toward `dest` and re-queue, but ONLY while `serial` still matches the trip-serial in `obj`'s `data` — a mismatch means the chain was superseded, and the hop dies silently. §Movement. |
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

**Built** (first-pawns, 2026-07-28). The sync philosophy this implements: the server fans out
authoritative state, clients issue commands against whatever they think state is, and the edge +
workers validate/execute against authoritative state — never lockstep.

`MOVE_TO` is a self-perpetuating chain: each hop writes one tile and queues the next, until `dest`.
The step is a **greedy straight line** behind an explicit seam (`one function`, replaced by
pathfinding when it lands). Two capabilities the chain uses:

- **A verb that queues an event.** A non-final `MOVE_TO` hop makes the worker queue the
  continuation `MOVE_TO obj dest` — baked into the verb, not a general `QUEUE` action (yet).
- **Queue-at-a-future-tic.** The next hop lands `k` tics out, `k` = the kind's `tics_per_tile`.
  **Speed is CONTENT, authored in TICS PER TILE** (user, 2026-07-28 — supersedes the wall-time
  authoring rule): each kind's `:data` facet authors its speed in the DSL corpus (wolf = 12 →
  2 s/tile at 6 Hz); `shared/codec::speed` holds only the DEFAULT for unauthored kinds and the
  resolution rule. Every consumer — the worker's continuation spacing, the client's speculation
  rate, the npc's trip deadline — resolves the SAME per-kind value through the corpus/bundle, or
  speculation drifts by design. The accepted consequence: a `TIC_HZ` change changes wall-clock
  movement speed, because the game's time unit IS the tic — content reads in tics, not seconds.
  So `queue` accepts `event_tic ≥ master + 3`, not exactly `+3`. The completeness barrier is
  unaffected — a tic's set is frozen at `T-2` regardless of *when* its events were born.

**Don't promote every hop.** Promoting `state` on each tile is exactly the per-tile fan-out we're
avoiding. The cadence:

- **`PROMOTE_EVENT` once**, on the initial program, announces the **intent**: the client now knows
  `obj` is heading to `dest`, at which tic.
- **`PROMOTE` at the seed and the final hop** — the start position anchors speculation; the landing
  corrects it. Bare continuations fan **nothing**. **The seed does NOT step** (user, 2026-07-28):
  the intent event's `MOVE_TO` promotes the object's **current** position unchanged (facing turns
  toward the path, and the trip-serial is stamped — see chain identity below), so the anchor
  aligns every client to the server BEFORE speculation walks — a
  seed that stepped first fanned `start+1` and opened every trip with a one-tile snap. The first
  step lands on the first continuation, one `tics_per_tile` after the intent; a trip is
  `hops + 1` hop-slots end to end.
- **Resolve-on-touch is free**: any other event touching `obj` composes (and, promoting, publishes)
  its resolved position — no extra machinery.
- A re-anchor **every N tiles** is a held knob — added when the recorded speculation error
  (first-pawns F8) says what N buys.
- **Authoritative state SNAPS speculation** (user, 2026-07-28 — first pass): a `state` row for a
  speculating object snaps it to the server's tile (the landing clears the speculation; an interim
  resolve reseeds it), and every correction logs its observed error. A **tween-blend** from the
  speculated position to the authoritative one is the second held knob — future intent, tuned on
  that same error data when snapping reads as visible jerk; not built.

So the initial program is `PROMOTE_EVENT PROMOTE MOVE_TO obj dest`; the self-queued continuations
are bare `MOVE_STEP obj dest serial`; the hop that reaches `dest` is
`PROMOTE MOVE_STEP obj dest serial`.

**Chains have identity — at most ONE lives per pawn** (movement-hardening F1, closing
pawn-movement I7). The seed stamps a 6-bit **trip-serial** (the seed event's
`event_reference & 0x3F` — unique even for two same-tic intents, where a tic-derived serial
would let both chains live) into the pawn's `data` low bits (`TABLES.md` § pawn); every
continuation carries that serial and checks
it against the pawn before stepping — a mismatch means a NEWER intent re-stamped the pawn, and
the stale hop dies silently (no step, no re-queue). So a new `MOVE_TO` intent CANCELS the old
chain by construction: no cancel machinery, no queue scans, and a driver's deadline re-issue is
safe (the measured alternative was two live chains fighting over the pawn — per-hop promotes
and 10-tile landing errors). Serial collisions don't matter: a superseded chain dies at its
FIRST subsequent hop, long before the 64-tic ring re-aligns.

**The client speculates without a synced tic.** `state` is simply the latest authoritative truth.
`PROMOTE_EVENT` on a move gives the client the one thing `state` can't: *intent* (`obj → dest`).
The client anchors "a row for tic `V` arrived at wall-time `W`" on every `state`/`event` arrival
and extrapolates elapsed tics at a rate LEARNED from the stream (pawn-movement F6 — seeded at
`TIC_HZ`, refined from the anchor history, because the true rate measurably drifts from the
authored one; anchors age out so a wrong estimate can always correct) — a loose wall↔tic mapping
refined by the stream itself (implicit sync, not the ping/pong that never worked). It walks the pawn fractionally along the
line at the kind's authored `tics_per_tile`, and authoritative `state` snaps/reseeds it
(corrections log their error — the data the re-anchor knob will be tuned on). Best-effort by
construction — the server dictates truth, the client makes it smooth.

---

## Deriving the sets

**Write set** — scan the stream; for each action, its signature names which operands are written
`entity_reference`s. **Read set** — the same scan, the operands the signature marks read. No
interpretation, no game semantics in the spine: the signature is a table lookup.

Every operand is a literal, so both sets are fully known at grouping — the whole point of dropping the
stack. (`CREATE`'s written target is the *minted* id, not an operand; a fresh entity is its own
singleton component, so it never conflicts.)
