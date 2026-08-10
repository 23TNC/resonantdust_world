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

## 2026-08-10 — P1: the route is on the wire, alongside the chain

**Fanned on the SEED, not the hop.** The CONTINUE pass runs for both `MOVE_TO` (the order) and
`MOVE_STEP` (each hop); the route is stated once per ORDER, which is what the design means by one
route computation and what P3 will make literally true. The per-hop chain is untouched — this
changes no motion, by construction: a separate promoted event that nothing steers from yet.

Payload `MOVE_CHORDS pawn serial count [position, tic]×n`. Consecutive stamps share an endpoint, so
`n` stamps spell `n-1` chords and stamp 0 is the pawn's own subtile source. The operand is the
**trip serial**, not a queue-entry id — the serial is what `CANCEL` bumps and what lets a client
refuse a route belonging to a superseded trip, which P5's replay guards need.

Queued at `master + 4`, not `t + 1`: a tic the clock has already passed is rejected async and the
fan silently vanishes (the BUILD_WALL lesson, already written into this file's neighbours).

**Verified live.** Worker at `debug`, a bunny fluffle wandering at `124,75`:

```
chord route entity=0x30800004 serial=38 chords=2 src="112,72@5815" dest="123,71@5953"
move intent entity=813694980 tile_x=123 tile_y=71 event_tic=5815
```

`813694980 = 0x30800004`, and the chord route's stated destination is the same tile as the order's
— a free cross-check that the route describes the trip it belongs to, not a stale one. 13 routes
fanned in 35 s; headless decoded every one.

Core carries it as `Event::MoveChords` with the payload **verbatim** — decode only, no fold into
`MoverTrack`. Deciding what a route MEANS is P5; P1 only has to prove the wire carries it.

`bin/sim check worker` and `bin/rd build core` clean.

## 2026-08-10 — P2: how many chords does a real trip need?

`headless <name> chords <x> <y>` — `find_chords` over 1000 random pairs per band, against core's
own composed view (the same derivation the worker's pathability uses, so this is the map the pawns
actually walk).

Terrain: **9409 known cells, 19.09% impathable**, extent `76..172 x 27..123`.

| band | n | p50 | p90 | p99 | max | **needing >10 chords** |
|---|---|---|---|---|---|---|
| 3-12 tiles | 1000 | 1 | 3 | 5 | 8 | 0 (0.00%) |
| 12-40 tiles | 1000 | 2 | 6 | 9 | 12 | 4 (0.40%) |
| 40-90 tiles | 999 | 5 | 9 | 12 | 14 | 37 (3.70%) |

**The unknown resolves in the design's favour.** A cap of 10 truncates nothing at wander range,
0.4% of medium trips and 3.7% of cross-map ones. The re-request stutter is a rare case, not the
normal one — which was the outcome that would have made this design worse than what it replaces.

**Two false starts, both worth recording.** The survey first reported `p50 = 1, 0% over cap` across
every band. It was measuring nothing: the world generates on demand, `pathable` calls an unknown
cell OPEN, and the sampled square was 99.3% unseen, so every pair string-pulled to one straight
line. The fix was to draw pairs only from cells the view holds — which then reported an honest
`known=256`, one 16x16 zone, and refused the long band outright.

That is [I4](issues.md#i4), and it is now closed rather than carried: the harness **generates the
terrain it measures**, laying a grid of 49 retained anchors (distinct names, so the nested-radii
release cannot evict the early ones behind the late ones) and waiting for generation. Coverage went
from 256 cells at 2.73% impathable — one empty meadow — to 9409 at 19.09%, which is where the
chord counts above come from and why they are believable.

The lesson is the reusable part: **a probe over a default-open world reports confident numbers
about nothing**, and the number it reports is exactly the reassuring one.

## 2026-08-10 — P2: what a player order costs today

50 `EXECUTE_INTERACTION(move_to)` orders from the browser, timed from `queue()` to the arrival of
the promoted `MoveIntent` naming that pawn AND that destination. Attribution matters: several
pawns are in play, so a bare "something moved" would time somebody else's order. The intent is the
right stopping point because speculation arms on it — the render begins moving within the frame.

Idle pawns (npc stopped), short trips (3-6 tiles), n=50, 4 refusals re-drawn:

| p50 | p90 | p99 | min | max |
|---|---|---|---|---|
| 1437 ms | 1488 ms | 1767 ms | 1168 ms | 1767 ms |

**8.6 tics at p50, and that corrects this stream's own arithmetic.** The [README](README.md)
reasoned that cancel-first "adds a barrier round trip ... 6-8 tics (~1.1 s) vs **~4 today**". Today
is not 4 tics, it is 8.6 — the estimate was out by more than 2x. A cancel that costs another 3-4
tics is therefore a ~40% increase on an already-slow path, not the doubling the README implied.
That does not make it free, but it moves the cost from "disqualifying" to "worth what it buys".

**The number that is actually bad is the one this measures on an IDLE pawn.** A busy pawn has no
preemption today: the order joins the intent queue and waits for the current walk to finish, so
the player-visible latency is a whole trip — seconds to tens of seconds, unbounded by anything.
I hit this while measuring, timing out every trial until I stopped the npc. That is the case
`CANCEL` exists for, and it is why the +3-4 tics is a bargain rather than a regression.

**Also found here:** `EXECUTE_INTERACTION` is verb **12**, not 6. Every trial silently refused
until I checked, because a malformed program is dropped without a client-visible answer — webgl
routes `Event::Status` only to the login progress channel, so a post-login `QueueErr` reaches
nobody (noted in P0, still unfixed, and it cost time twice now).

## 2026-08-10 — P1 THE GATE: measured, and it misses its own threshold by one tic

`|stated dest tic − actual arrival tic|`, **n = 56** driven trips (the criterion asks 50):

| p50 | p90 | p99 | max | within 2 tics | within 5 tics | signed mean |
|---|---|---|---|---|---|---|
| 1 | **3** | 5 | 47 | 43/56 (77%) | 55/56 (98%) | +0.71 |

```
  0 tics  14 ##############
  1 tics  19 ###################
  2 tics  10 ##########
  3 tics   8 ########
  4 tics   2 ##
  5 tics   2 ##
 >8 tics   1 [47]      <- the only 2-chord trip in the sample
```

**The criterion is p90 ≤ 2. It is 3. The box stays unticked** — see [F3](forks.md#f3) for what that
means and why the next step is still P3 rather than a re-design.

### Measuring it took four attempts, and the first three lied

1. **One trip per pawn per entity.** A brain issues its next order the moment the pawn arrives, so
   supersession lands within a tic or two of the stated arrival — and dropping superseded trips
   discarded every accurate trip while keeping the inaccurate ones.
2. **"First state row on the destination tile" as arrival.** Log order is not tic order (a zone
   replay re-delivers old rows late) and a mid-route anchor can sit on the destination tile. The
   sound test is `dest tile AND subtile (0,0)` — the same `position == dest` the worker uses to end
   the chain — matched in TIC order. This produced the "arrivals" one tic after their own order.
3. **Scraping a subscribed client.** Three captures each went blind partway through: pawns walk out
   of the anchor's zones, `movers` falls to 0, and the observer keeps writing a log that no longer
   contains the arrivals being counted. It reported 16 arrivals for 28 driven trips.

The measurement now lives **in the worker**, which cannot miss its own chain ending: the route's
claim is recorded when it is fanned and compared when the CONTINUE pass sees `arrived`.

4. **And the instrumentation itself aliased.** Keyed on `(pawn, trip serial)` it reported arrivals
   **195 tics before their own order** — impossible, and the cause is the README's own watch-list
   item: the trip serial is 6 bits and wraps every 64 trips, so a superseded claim survived to
   match a later arrival. Fixed by dropping any prior claim for a pawn when a new route is stated
   (a pawn has at most one live route). That aliasing is going to bite the SIMULATION exactly the
   same way once cancel churn is routine — [P4](todo.md) already carries widening it, and this is
   the first hard evidence that it must actually happen.

### Where the residual 1-3 tics comes from

The claim and the motion are computed by **two different mechanisms**: `chord_schedule` accumulates
on the running total and rounds once, while the per-hop chain re-paths every hop and rounds each
`k = ceil(dist × pace)` up. A full-stride hop costs exactly `REANCHOR_TICS`, so it contributes no
rounding — but every CORNER and the final partial hop can each add a tic. The distribution matches:
55 single-chord trips give p90 = 3 and max 5, and the one 2-chord trip in the sample is the 47-tic
outlier.

That is the gap between the OLD chain and the NEW schedule — and [P3](todo.md) deletes the old
chain, making each hop's write the literal chord endpoint. After P3 the two cannot disagree,
because there is only one of them.
