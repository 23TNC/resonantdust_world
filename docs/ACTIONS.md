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
| `CREATE` | 3 | variable | `def:definition_reference` (imm) · `position:position_reference` (imm) · `count` (imm) · `payload×count` (imm) → **mints** a new entity — THE creation verb for any object (human-pawns F2; no separate spawn action). The packed def's `type_id` **routes** the mint to its type's shard arm (`TYPE_PAWN` today; any other type is rejected by name in the worker, never silently pawned). `payload` is the minted entity's opcode-stream sidecar ([`TABLES.md` §payload](TABLES.md)) — e.g. a human's `PART(0, body def)`/`PART(1, head def)`; empty for the wolf. The written target is the *minted* id, not an operand. The second variable-arity verb (with `INIT_ZONE`); the `count` word sits third in both. |
| `PLACE` | 4 | 2 | `obj:entity_reference` (**write**) · `position:position_reference` (imm) — set `obj`'s position absolutely. |
| `MOVE_TO` | 5 | 2 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) — the movement-chain SEED, **WORKER-ONLY since input-rework F3** (it left `CLIENT_VERBS`; the worker's `move_to` interaction arm is its only composer — clients move by queueing `EXECUTE_INTERACTION(move_to)`). The seed's mechanics are unchanged: stamps the trip-serial, turns facing toward the path, promotes the current position when `PROMOTE`-prefixed, steps NOTHING; the worker chains `MOVE_STEP` hops from it — §Movement. |
| `SET` | 6 | 5 | `cold_row:cold_row_reference` (**write**) · `type_id` (imm) · `tile_reference` (imm) · `kind_reference` (imm, `0`=clear) · `data` (imm) — **override one cold cell** through the `overlay` tier. The `cold_row` is spelled (so concurrent `SET`s to one row **group** — see routing below) and `type_id` names the shard (a `cold_row_reference` carries no type nibble). Replaces the old per-cell `cold_entity_reference` SET. |
| `INIT_ZONE` | 7 | variable | `cold_row:cold_row_reference` (**write**) · `type_id` (imm) · `count` (imm) · `item×count` (imm) — build a zone's **whole baseline row** and write it to `entity_state_log` (`ColdBaseline` tier); `PROMOTE INIT_ZONE …` projects it visible. The first variable-arity verb (F12; `CREATE` is the second). Built. |
| `MOVE_STEP` | 8 | 3 | `obj:entity_reference` (**write** + **read**) · `dest:position_reference` (imm) · `serial` (imm) — one chain hop, **WORKER-ONLY** (the edge rejects it from clients — movement-hardening F2): step `obj` one tile toward `dest` and re-queue, but ONLY while `serial` still matches the trip-serial in `obj`'s `data` — a mismatch means the chain was superseded, and the hop dies silently. §Movement. |
| `BUILD_WALL` | 9 | 3 | `start:position_reference` (imm) · `end:position_reference` (imm) · `object:definition_reference` (imm, u32 slot) — the CLIENT-issued build order (build-walls D5). Writes NOTHING itself: the worker expands the `start..end` rect's **perimeter** and queues a `PROMOTE SET` per tile (the verb-that-queues-events pattern `MOVE_TO` established), so each cell routes to ITS zone and a multi-zone rect is safe by construction. FIRST implementation = immediate build; the documented FUTURE keeps this exact shape and swaps what the worker queues (blueprint-entity creates that pawns must build) — "immediate" is the worker's current policy, not the verb's contract. §Building. |
| `SET_NEED` | 10 | 2 | `obj:entity_reference` (**write**) · `row` (imm, the packed gameplay row `value:16 \| kind:12 \| variant:4` — stat-model F1/F4; value is u16 FIXED-POINT on the need's authored domain, quantized ONCE by the composer) — set one need's value on a pawn. The pawn module's `set_need` reducer upserts the `needs` sub-table row (`set_tic` = the composing tic); the worker relays, adding nothing to its read set. The write operand serialises it with the pawn's movement writes. **Arity 3→2 with the stat-model reshape** (the def-ref + f32-bits form is gone). CLIENT-open like `MOVE_TO` while no ownership model exists ([interactions I4](work/2026-08-06-interactions/issues.md#i4)). |
| `GRANT_CONDITION` | 11 | 2 | `obj:entity_reference` (**write**) · `row` (imm, the packed gameplay row `remaining_at_write:16 \| kind:12 \| variant:4` — stat-model F1/F3; the composer passes the condition's authored `duration` as `remaining_at_write`) — grant one TIMED condition (needs-moodlets F2/F7). `written_tic` = the composing tic; remaining-now and expiry are DERIVED at read, never stored; a re-grant refreshes the row. The re-stamp law (stat-model F7) binds the COMPOSER, not this reducer (the module holds no corpus — [I12](work/2026-08-06-stat-model/issues.md#i12)): whoever queues a grant of a need-modifying condition queues the re-stamping `SET_NEED`s beside it; the eval covers a lone FIRST grant exactly (its window starts at the written offset), but re-grants overwrite their own history. DERIVED (band) conditions have no verb — they are computed, not granted. `EXECUTE_INTERACTION` is the real producer (drink → quenched); the npc's `NPC_GRANT` drill remains the forcing lane. |
| `EXECUTE_INTERACTION` | 12 | variable | `interaction:definition_reference` (imm) · `version` (imm) · `count` (imm) · `inputs×count` (imm) — execute a corpus-defined interaction ([interactions F4](work/2026-08-06-interactions/forks.md#f4), the user's layout verbatim). **Writes NOTHING itself** — the `BUILD_WALL` pattern: the worker resolves the interaction's input SIGNATURE from the corpus (interactions F5), decodes value inputs as **f32 bit patterns**, validates (count vs signature; entity inputs resolve to live pawns; def inputs exist; non-finite floats REJECT — [I6](work/2026-08-06-interactions/issues.md#i6)), checks the location rule (`"on"`: the target stands on a carrier tile whose def OFFERS this interaction — F8, carrier binding per stat-model F9), gates on the interaction's affordance PREDICATES over the pawn's DERIVED stats (stat-model F5/F8 — traits + conditions rows → stats → `check` passes), computes the new satisfaction through the ONE `needs_eval` (clamped to the need's effective domain), then **queues** `PROMOTE SET_NEED` / `PROMOTE GRANT_CONDITION` on the target — the queued verbs carry the real write set, so the untyped input words never need top-byte routing. A failed validation logs and drops the event whole, never half-executes. Third variable-arity verb (`INIT_ZONE`, `CREATE`); the `count` word sits third in all three. |
| `QUEUE_STATE` | 13 | variable | `pawn:entity_reference` (imm — a pure DISPLAY fan, no write set) · `_reserved` (imm, 0) · `count` (imm, total payload WORDS — the fourth variable-arity verb, `count` third like the others) · `entry×4 per intent` (imm: `entry_id` — WORKER-MINTED and opaque to clients, stable across the entry's life; a composed `[walk, act]` pair holds TWO ids though one order made both, which is why the ORDER's reference cannot name a circle — `interaction_ref`, `phase` 0 pending / 1 walking / 2 executing, `started:16 \| fire:16`) — the pawn's INTENT QUEUE snapshot, **WORKER-ONLY**, always `PROMOTE_EVENT`-prefixed so it fans to the zone's subscribers (intent-queue-ui F1). Emitted at EVERY queue mutation from ONE `fan_queue` helper. Authority stays the worker's ephemeral map (lumberjack F1) — a lost fan is a stale display corrected by the next mutation, never a wrong action. Entries order bottom-up: entry 0 = the ACTIVE event. |
| `CANCEL_INTENT` | 14 | 2 | `pawn:entity_reference` (imm — resolves against worker MEMORY, writes nothing) · `entry_id` (imm — the fanned entry's worker-minted id; an index would race advancement — intent-queue-ui F4) — a CLIENT-open cancel request. Resolution by phase (F3): a PENDING entry is removed; the EXECUTING timed entry cancels only if its TOML authors `cancelable` — the already-queued completion then fires into a logged `cancelled` NO-OP via an EPHEMERAL cancelled-set; an EXECUTING walk clears the queue and the chain finishes its glide (the parked act dies — the replace law's shape). Unknown reference = logged no-op. **The cancelled-set is worker memory**: a bounce forgets it, and a still-valid completion then executes — the lumberjack F1 posture, stated. Every path refans `QUEUE_STATE`. |
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

**Built** (first-pawns, 2026-07-28; the front door re-hung by input-rework, 2026-08-06). The
sync philosophy this implements: the server fans out authoritative state, clients issue commands
against whatever they think state is, and the edge + workers validate/execute against
authoritative state — never lockstep.

**The FRONT DOOR is the `move_to` INTERACTION** (input-rework F2/F3): a client (browser pie
menu, npc brain) queues `EXECUTE_INTERACTION(move_to, [pawn, destination])`; the worker
validates it — the destination tile OFFERS `move_to` (location `"target"`, F4; walls don't
carry it, F9), the pawn's `can_move_ground` predicate passes — and queues the
`PROMOTE_EVENT PROMOTE MOVE_TO pawn dest` seed at `master+4` (the I11 barrier rule). From the
seed down, everything below is unchanged.

`MOVE_TO` is a self-perpetuating chain: each hop walks one CHORD and queues the next, until `dest`.

**The step is the shared path's first CHORD** (chord-movement, 2026-08-07 — the per-tile hop
retired the day pathfinding replaced the greedy seam). The chord law:

- **The route is the string-pulled polyline** — `path_eval` runs octile A* then string-pulls
  to the MINIMAL chord sequence (corner points on the doubled integer grid — chord-movement
  F5); a pawn walks straight lines of arbitrary slope between them. Chords split at ~8 tiles
  so a hop's window stays bounded (I7).
- **One hop event per chord** (F2): a hop writes the chord's END — a SUBTILE position
  (`sx:4|sy:4` in the position's low byte, VARIABLES.md) — and queues the next hop at
  `tic + ceil(chord_len × ground_speed)`. Event count = the chord count.
- **Speed flips at EVAL, not in content** (F6): `walks` still authors tics/tile; consumers
  derive `tiles_per_tic = 1 / ground_speed`. All schedule arithmetic stays in integer tics.
- **Mid-chord RESOLVE-ON-TOUCH is law** (F3): between chord writes the stored row is the
  chord's start + its write tic. Any event touching the pawn — a superseding seed, an
  interaction effect, a validation floor — first resolves
  `position = start + unit(chord) × (now − write_tic) × tiles_per_tic`, quantized to
  subtile, using the deterministic route recompute to know the chord. No new state: the
  row + the corpus + the tic are the whole computation. This is what makes an interrupted
  trip continue from where the pawn IS.
- **Tile identity = floor** (I2, half-open: subtile 0 belongs to the tile). VALIDATION
  floors only worker-stored/resolved positions; client flooring is display-advisory.

The pathing law beneath it (pathfinding, 2026-08-07):

- **ONE pathfinder** — `path_eval` in shared/content beside the other evals: A* on the 8-way
  tile grid over a caller-supplied cell probe. The worker feeds its mirrored composed
  tile ⊕ thing tiers, the wasm client its render-state view, the npc the same through its
  corpus — every observer computes the SAME path or speculation diverges by design.
- **Pathability is DERIVED, never stored** (pathfinding F1). A cell is pathable iff its
  composed tile kind authors `pathable` (VARIABLES.md; absence = true) AND no impathable
  thing occupies it (thing overlay kind-0 SUPPRESSES — a felled tree reopens its cell with
  no extra write). There is no pathability table.
- **Per-hop STATELESS recompute** (pathfinding F3): every hop re-runs `path_eval` from the
  pawn's current (resolved) position and walks the first chord. No stored route —
  supersession, re-issue, and mid-trip world changes stay correct because every hop
  re-reads the world.
- **NO corner clipping** (pathfinding F4, generalized by chord-movement F5/F7): a chord's
  supercover — inflated by the pawn's footprint radius (1×1 ships radius 0) — must be
  wholly pathable; the old diagonal rule is the degenerate one-tile case.
- **Impathable or unreachable destination = LOGGED NO-OP** (F5): the trip drops at seed
  time with a log line, the intent-completion posture. Bounded search — cap exhaustion
  reads as unreachable, never a stall.
- **Leaving an impathable cell is ALWAYS legal** (F6): pathability gates the cell being
  ENTERED; a stranded pawn (worldgen scatter, a tree grown underfoot) can walk out, and an
  impathable START cell is accepted.

Two capabilities the chain uses:

- **A verb that queues an event.** A non-final `MOVE_TO` hop makes the worker queue the
  continuation `MOVE_STEP obj dest serial` — baked into the verb, not a general `QUEUE` action (yet).
- **Queue-at-a-future-tic.** The next hop lands `k` tics out, `k = ceil(chord_len ×
  ground_speed)` (chord-movement F6) — `ground_speed` is the pawn's pace in TICS PER
  TILE, since input-rework F8 the DERIVED **`ground_speed` stat** (the pawn's `walks` level's
  authored tics/tile through the ONE `stat_eval`; the old per-kind `speed` field is DELETED).
  Every consumer — the worker's continuation spacing, the client's speculation rate, the npc's
  trip deadline — derives the SAME value from the pawn's rows + corpus, or speculation drifts by
  design. `shared/codec::speed` holds only the degenerate-fallback default. The accepted
  consequence: a `TIC_HZ` change changes wall-clock movement speed, because the game's time unit
  IS the tic — content reads in tics, not seconds.
  So `queue` accepts `event_tic ≥ master + 3`, not exactly `+3`. The completeness barrier is
  unaffected — a tic's set is frozen at `T-2` regardless of *when* its events were born.

**Don't promote every hop.** Promoting `state` on each tile is exactly the per-tile fan-out we're
avoiding. The cadence:

- **`PROMOTE_EVENT` once**, on the initial program, announces the **intent**: the client now knows
  `obj` is heading to `dest`, at which tic.
- **`PROMOTE` at the final hop only — the seed fans NO position** (user, 2026-08-07;
  chord-movement F4, retiring the 2026-07-28 start anchor). The start fan existed to align
  speculation at trip open, but it QUANTIZED an interrupted pawn to its last tile — the
  backtrack visual. With subtile positions the client arms speculation from its CURRENT
  belief instead; the landing still corrects exactly. The seed still stamps the
  trip-serial and turns facing (chain identity below); it steps nothing and fans nothing.
- **The every-N re-anchor is ON** (chord-movement F4 — the knob first-pawns held): a bare
  subtile-accurate `PROMOTE` every `REANCHOR_TICS` (32 to start; the recorded spec error
  tunes it) bounds drift for late joiners and bad estimates. Because it carries subtile
  position, a correction NUDGES — it cannot reproduce the old one-tile snap.
- **Resolve-on-touch is law** (chord-movement F3): any other event touching `obj` first
  RESOLVES its mid-chord position, then composes (and, promoting, publishes) it.
- **Authoritative state steers the SPECULATION; the RENDER chases** (user, 2026-07-28 —
  movement-hardening F6, the former tween knob, built): a `state` row snaps the *speculation*
  to the server's tile (the landing clears it; an interim resolve reseeds it; every correction
  logs its spec-space error), but the RENDERED position is its own track that chases the
  speculated one at ≤ **+20%** of the pawn's true speed (non-linear — the further behind, the
  harder the lean), snapping only past a hopeless gap. The contract: exact agreement at the
  destination, broad agreement along the path — mid-route events publish their own resolved
  positions, so the chase always has fresh truth to converge on.

So the seed program (worker-queued, from the interaction) is
`PROMOTE_EVENT PROMOTE MOVE_TO obj dest`; the self-queued continuations are bare
`MOVE_STEP obj dest serial`; the hop that reaches `dest` is `PROMOTE MOVE_STEP obj dest serial`.
The client's speculation `MoveIntent` is manufactured from that fanned seed exactly as before
(input-rework I1) — its `event_tic` is now the seed's `master+4` queue tic.

**Chains have identity — at most ONE lives per pawn** (movement-hardening F1, closing
pawn-movement I7). The seed stamps a 6-bit **trip-serial** (the SEED event's
`event_reference & 0x3F` — since input-rework F3 that is the worker-queued effect event's
reference, still unique even for two same-tic intents, where a tic-derived serial
would let both chains live) into the pawn's `data` low bits (`TABLES.md` § pawn); every
continuation carries that serial and checks
it against the pawn before stepping — a mismatch means a NEWER intent re-stamped the pawn, and
the stale hop dies silently (no step, no re-queue). So a new `move_to` order CANCELS the old
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
line at the pawn's DERIVED `ground_speed` (tics/tile), and authoritative `state` snaps/reseeds it
(corrections log their error — the data the re-anchor knob will be tuned on). Best-effort by
construction — the server dictates truth, the client makes it smooth.

---

## The intent queue, and interactions that cost tics

**The law** (lumberjack, 2026-08-07; consumes the `duration` seam the stat-model corpus
reserved). A pawn can hold a short sequence of intents — "walk there, then chop" — and an
interaction can cost tics. Both ride the machinery above; neither adds authoritative state.

- **The queue is EPHEMERAL** (user, lumberjack F1): a per-pawn pending list in WORKER
  memory, **cap 5**. The durable half is the standing event machinery — the work in
  flight is a queued event in the event shard, exactly like a `MOVE_STEP` hop. A worker
  bounce loses only the pending tail: "whatever happens to that queue… happens."
- **One composer.** The worker's `EXECUTE_INTERACTION` arm is where queues are built: an
  order whose location rule fails **only by distance** composes `[move_to(adjacent), act]`;
  an in-place order is just `[act]`. The menu and the npc need no queue logic — they issue
  one interaction and the worker sequences it (the same single-authority posture as the
  affordance gate).
- **A fresh order REPLACES the whole queue** (F3) — the `MOVE_STEP` one-chain law lifted a
  level. Preemption = issue a new order. A composition that would exceed the cap rejects
  WHOLE (log-and-drop, the interactions I6 law — never a partial queue).
- **Advancement is per intent kind**: a `duration = 0` intent completes in the pass that
  executes it; a `move_to` intent completes when its chain's FINAL hop lands — keyed by
  the pawn's **trip serial**, so a superseded chain advances nothing; a `duration = N`
  intent queues its **completion event** at `+N` (queue-at-a-future-tic, §Movement) —
  elapsed-tics tracking is implicit in the target tic. On completion the worker queues the
  pawn's next pending intent and drops it from the list.
- **Every completion RE-VALIDATES** (user, F1 — the safety is here, not in queue
  integrity): affordances AND the location rule re-run at the fire tic against
  authoritative rows. A stale intent — the drink whose walk was cancelled, the chop whose
  pawn wandered off — resolves to a **logged NO-OP**, never a wrong write. No cancel
  machinery, no partial credit, no refunds: a felled tree means the pawn was still
  adjacent when the full duration elapsed.

`duration` is authored on the interaction in TICS (`VARIABLES.md` § TOML content schema);
`duration = 0` — drink, the move_to seed — is the degenerate everything-in-one-pass case
and is exactly the pre-lumberjack behavior.

**The queue is VISIBLE and CLICKABLE** (intent-queue-ui): every mutation fans a promoted
`QUEUE_STATE` snapshot (palette above) that the details panel renders as the left-edge
strip — bottom circle = the active event, ring percentage derived client-side from the
fanned `(started, fire)` pair through the learned tic estimate. A circle click sends
`CANCEL_INTENT pawn order_ref`; the phase-by-phase law and the ephemeral cancelled-set
caveat live on the verb's palette row. Presentation only: neither verb touches the
write sets, and the no-op re-validation law remains the single execution safety.

---

## Deriving the sets

**Write set** — scan the stream; for each action, its signature names which operands are written
`entity_reference`s. **Read set** — the same scan, the operands the signature marks read. No
interpretation, no game semantics in the spine: the signature is a table lookup.

Every operand is a literal, so both sets are fully known at grouping — the whole point of dropping the
stack. (`CREATE`'s written target is the *minted* id, not an operand; a fresh entity is its own
singleton component, so it never conflicts.)
