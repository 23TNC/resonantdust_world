# Completed — server-chords

_Opened 2026-08-10._

## 2026-08-10 — P0 rest, and P1's schedule

**The fan-zone lookup.** `MOVE_CHORDS` joins `QUEUE_STATE` in the worker's write-less special
case: a verb with no write set completes **zoneless** and reaches no subscriber, so its fan zone
comes from the pawn it describes. The comment now names the shape rather than the verb — "operand
0 is the pawn" — because this is the third time a write-less fan has needed it.

**The edge allowlist.** `CANCEL` admitted, `MOVE_CHORDS` deliberately absent: a client that could
state its own route is the entire defect this design removes.

**`move_eval::chord_schedule`** — chords in, stamped endpoints out. Two laws, both tested:

- **Tics accumulate on the RUNNING total, never per leg.** Rounding each leg with `ceil` and
  summing drifts the arrival by up to one tic per chord; ten chords, ten tics, and the error lands
  exactly where the client's interpolation has to be right — the end.
  `the_schedule_does_not_accumulate_rounding` pins ten legs against the whole route's time.
- **The source is the pawn's SUBTILE point.** `find_chords` speaks lattice cells and a mid-walk
  pawn is not on one; scheduling from its tile centre would state a source it is not at.
- The 4-tic floor applies to the accumulated total, so a run of short chords costs their real time
  plus one floor rather than one floor each — `a_short_leg_takes_the_floor_and_the_floor_does_not_compound`.

14 `move_eval` tests, 76 codec tests, worker clean.

## 2026-08-10 — P0: the two verbs exist on the wire

`MOVE_CHORDS = 20` and `CANCEL = 21` in `shared/codec::action`, with palette rows in
`docs/ACTIONS.md` — the authoritative table, which outranks the code.

- **`CANCEL` is `&[ReadWrite]`.** The pawn must be a WRITE or the verb neither joins its write
  group nor serialises against the hops it is cancelling, and a Read so the worker sees the row it
  resolves from. This is why it cannot fold into `CANCEL_INTENT` ([F2](forks.md#f2)).
- **`MOVE_CHORDS` is the fifth variable-arity verb**, added to the framing list, the
  `collect_operands` skip list, and `target_routes`' write-less arm.

**Four tests, and one of them earns its keep:**
`move_chords_frames_and_a_verb_after_it_survives` puts a fixed-arity verb AFTER the variable one,
because a mis-framed variable verb has no re-sync point — everything downstream is garbage with no
error. Omitting the skip-list entry instead panics inside `collect_operands` at its
`signature().expect("parsed, so known")`, which the third test covers.

**Found while writing that test:** `SET_NEED`'s arity is **3 in code** while `ACTIONS.md` says
"Arity 3→2 with the stat-model reshape". My first draft used it as the canary and got
`Truncated { action: 10, want: 3, got: 2 }` — I read it as a framing bug in my own change for
several minutes. Docs outrank code, so one of them is wrong; logged as [I3](issues.md#i3) rather
than fixed in passing, since changing a verb's arity is a wire change.

**Also added the three palette rows the survey found missing** — `INV_ADD` 15, `INV_REMOVE` 16 and
`ACTIVATE_TRAIT` 19 were shipped verbs absent from the authoritative table, which is how 20 and 21
came to need checking by hand.

76 codec tests pass.

## 2026-08-10 — P0 closed: the verbs are live on the wire

Redeployed every module against the new codec (the codec is a bind-mount; a shard links its own
copy, so an un-redeployed shard would reject verb 21 outright). Rebuilt and restarted the sim —
`build` then `run`, per the stale-binary lesson — and re-adopted a fluffle at `124,75`.

**Verified with a raw WebSocket probe** against the edge at `ws://localhost:8473/ws`, logging in as
`ProbeP0` and queuing both verbs on one connection, because the criterion needs the *shard's*
verdict and webgl throws that verdict away (`Event::Status` is wired only to the login progress
channel, so a post-login `QueueErr` never reaches the console — noted, not fixed):

| queued | reply |
|---|---|
| `[CANCEL, pawn]` | `queue_ok` |
| `[MOVE_CHORDS, pawn, 7, 4, 11, 22, 33, 44]` | `queue_err — "server-only verb: 20"` |

**`queue_ok` is the shard's answer, not the edge's.** The edge replies from inside
`queue_then`'s reducer callback, and `queue_common` re-parses the program with its own linked
codec; `arity()` returns `None` for an unknown verb, so a stale module would have answered
`bad program: UnknownAction(21)`. It did not — the acceptance criterion, met at the layer it names.

The refusal is the other half of the design: a client that can state its own route is precisely the
defect being removed, and the edge names verb 20 as server-only by number.

The worker has no `CANCEL` arm until P4, so the accepted event is a no-op there — checked
deliberately, since an unimplemented verb reaching a `match` is how a worker panics. It stayed up
and logged nothing about it.

## 2026-08-10 — P1: the barrier floor, and a divide-by-zero I had just shipped

The clamp itself was already there — `MIN_LEG_TICS = 4` lifts a short first chord clear of the
event shard's `TIC_GAP = 3` barrier, and it matters because [P3](todo.md) queues each chord's hop
with `queue_at(program, chord.tic)`, where a tic inside the barrier is **rejected outright**. The
floor is what turns that rejection into a carried chord.

**Writing the test found a real defect in the schedule I landed an hour ago.** The tic came from
`run.ceil().max(MIN_LEG_TICS)`, so two chords whose accumulated runs fall in the same `ceil` bucket
— and *every* chord under the floor — got the SAME tic. That states a segment whose source and
destination tics are equal, and the client's lerp is `(now - src) / (dst - src)`: a divide by zero
on the single arithmetic every interpolation in this design routes through. The floor I added to
clear one barrier had quietly manufactured a worse failure behind it.

Fixed with a monotonicity law — **every chord spans at least one tic** — and the test asserts it
pairwise across the whole schedule rather than at one index, since the collision is positional.

The cost is honest and bounded: five 1-tile chords at pace 1 now state 8 tics instead of 5, because
the floor forbids stating the first four sooner. Only a pace of 1 tic/tile reaches the floor past
chord 0 at all; the authored paces are 12 (wolf) and 24 (bunny), where a full-tile leg costs 12-24
tics and the guard never fires. I updated the older test's expectation to 8 and wrote the
arithmetic into it — its property was "the floor is paid once, not per chord", and that still holds
(8, not 20).

15 `move_eval` tests pass.
