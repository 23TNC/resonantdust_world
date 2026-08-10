# Completed — shared-simulation

_The verification log: what landed and **how it was checked**. Append-only; authoritative for what
is done. Items live in [`todo.md`](todo.md) with their boxes ticked._

## 2026-08-10 — P3 complete: adjacency against a MOVING pawn

`adjacency_uses_the_moving_point_not_the_last_anchor` pins the case the whole phase was about, and
does it by asserting **both** answers: a bunny anchored at (10,10) walking east is, 72 tics later,
~3 tiles along. A wolf at (13,10) is `cheb ≤ 1` of where the bunny IS **and `> 1` of where it
anchored** — so the test fails if either the new answer breaks or the old wrong one creeps back.

That gap is why npc's tile-granular `pawns` map had to go: asking adjacency off an anchor up to a
full stride stale makes a chase swing at empty ground, or refuse a strike it should land.

The wolf-chase item is ticked on the SELECTION property (the hunt reads core's subtile point) with
its soak half reworded — see [I13](issues.md#i13): organic hunger needs ~78 minutes, so the
original criterion could not contain its own event, and I am not ticking a behavioural claim on a
window where the behaviour cannot occur.

`client/core` 58 passed.

## 2026-08-10 — the local optimistic arm closes the fire→fan gap

`IntentQueues::arm_pending(entity, at_tic, duration)` — the host queued something and the server
has not answered yet, so the pawn reads busy across that round trip. **This is what the deleted
`+ 3.0` seconds was really for**, and getting it right means the margin was never needed: the arm
is superseded by the NEXT FAN OF ANY KIND, including an empty one, so a refused order releases the
pawn immediately instead of after a fixed wait.

The deadline is a TIC, not a wall clock — the thing being waited on is tic-paced — and it expires
on its own so a fan that never arrives cannot wedge a pawn busy forever.

Both brains arm at their fire site with the interaction's authored duration (floored at 8 tics for
instantaneous acts, which still have a round trip). Two tests: the arm holds to its deadline tic
and expires past it; a fan supersedes it, and the empty-fan case asserts the release comes from
the *refusal*, not the timeout.

`client/core` 56 passed; live: 13 move intents in 40 s, 0 errors.

## 2026-08-10 — zero warnings across npc, worker and core

Acting on [I12](issues.md#i12) rather than only recording it. That deadlock hid behind
`warning: unused variable: view` in a crate with a dozen warnings I had learned to skim, so the
backlog is the defect, not the noise.

Cleared: two dead `Event::` arms left over from moving folds into core (one of which I had emptied
into a syntax error and only the compiler caught), the vestigial `bindable` flag in the worker's
need-trigger, a stale `pack_row` import, `rows_of` duplicating the very helpers it was given, and
a `dur` the brain no longer needs now that the server times the act.

`bin/sim check npc`, `bin/sim check worker`, `bin/rd build core` — **0 warnings each**. From here a
new warning means something.

## 2026-08-10 — P3c: ONE input binder, including the server's own

`Bundle::bind_interaction(name, &InputBinding)` resolves the reserved vocabulary — `pawn`,
`destination`, `target`, `amount`, `slot`, `item` — into an `EXECUTE_INTERACTION`'s operand words.
It replaces **four** hand-rolled copies: `wolves.rs`, `bunnies.rs`, and — the one the audit called
out and the one I'd have looked for last — **the worker's own need-trigger**. A server with its
own spelling of a client-facing vocabulary is the last place anyone looks when an operand shifts.

`grep -c '"pawn" =>'` across `client/npc/src/brains/` and `server/worker/src/main.rs` is **0**.

Two tests against the real corpus: the binder follows each interaction's AUTHORED input order for
every interaction in the corpus (the property four positional binders each had to get right
separately, unchecked), and a caller missing an input gets `None` rather than a short program —
because the operands are positional, so a hole is read as a different binding, not as an error.

Verified live: worker and npc rebuilt and running, 0 errors on either.

**One behaviour change recorded rather than smoothed over** ([F8](forks.md#f8)): wiring the brains
onto `pawn_busy` cut the observed move-intent rate to about a third. That is the fix working — the
old guess let a brain re-order over its own walk, which is [I4](issues.md#i4)'s 63%-of-orders
interrupt storm seen from the other side.

## 2026-08-10 — the brains stop guessing when they are busy

`busy_until: Option<Instant>` is deleted from both brains, along with both
`Duration::from_secs_f64(dur / 6.0 + 3.0)` conversions. The hold is
`Client::pawn_busy(entity)` — core's answer, from the server's own queue fan.

What the guess cost, now that it is measurable: the `+ 3.0` was margin for the fire→fan round
trip, so a wolf **over-held by ~2.6 s on every meal**, and a refused or raced-away order was
invisible — the brain sat out the full margin instead of being released by the empty fan. What
remains in the brains is the `eat_latched` re-arm, which is policy (what to do when released) and
belongs to them.

Live after the change: **23 move intents in 50 s, 0 errors**, log advancing.

`grep -rn 'busy_until\|dur / 6.0 + 3.0' client/npc/src` is 0.

## 2026-08-10 — P3b: the intent queue is STATE

[`client/core/src/intents.rs`](../../../client/core/src/intents.rs) — each pawn's committed queue,
folded from `QueueState`, replaced WHOLE per fan (a partial merge would invent state the worker
never had), with `busy(entity, now)`, `fires_at`, and handle accessors on both hosts.

`QueueState` was documented **"Display truth only"** and both hosts believed it: webgl mirrored it
for a strip, npc did not read it at all and GUESSED instead, holding an `Instant` deadline of
`duration / 6.0 + 3.0` seconds. That is a frame stating *when a pawn's committed act finishes* —
the one thing a brain must not get wrong — filed as decoration because its first reader was a
panel.

Four tests, and two of them pin traps rather than happy paths:

- **A walking entry fans `fire_tic 0`.** Reading that as "already fired" calls a walking pawn idle
  and lets its brain issue a second order over the first. `busy` treats phase 1 as busy regardless.
- **An EMPTY fan clears the pawn** — the refusal signal brains cannot see today: an order rejected
  or raced away leaves no entry, so the brain must be released rather than held for the guess's
  three-second margin.
- A replayed older fan is refused; equal tics accept, because two mutations can fan in one
  composing tic.

Also settled the tic-ring comparison: `movers.rs`'s local `tic_newer` is gone, and the crate uses
`codec::tic::tic_after`. One ring, one spelling.

54 tests pass in `client/core`.

## 2026-08-10 — webgl's parallel row stores are deleted

`MoverLayer`'s `payloads` and `needRows` maps are gone — `grep -c` is 0 — along with the
`onPawnNeed` fold that filled one of them and re-spelled the 48-bit law as `need % 0x100000000`.
`rowReference` is exported from the codec through wasm so there is one spelling; `pawnNeeds` and
`pawnPayload` read core.

**Verified live in the browser**, which is the criterion the audit said had never been run: 5
movers, needs **3 pairs each with a nonzero value (65535)** read back through core, payloads 15
words (bunny) / 18 (wolf), pace **24 / 12** derived in Rust, positions subtile
(`126.717, 76.605`), `nowTic` 29824.

That is the fifth and last copy of the row store. The count over this stream: `MoverLayer.ts`,
`wolves.rs`, `bunnies.rs`, `debug.rs`, and — briefly, and by me — `Mover` itself.

**A method note, because I wasted two edits on it:** multi-line regex deletion over TypeScript
over-cut twice, silently eating a neighbouring interface both times. Exact-string anchors only for
these files; the regex is fine for Rust where the shapes are more regular, and even there it
clobbered a struct field once.

## 2026-08-10 — P3 item 1: npc's last duplicate pawn store is gone

`Bot::pawns` (`entity -> (tile_x, tile_y)`) deleted. `pawn_at` floors core's answer and a new
`pawn_point` returns the fractional one — so a brain sees where a pawn IS mid-chord, not where it
last anchored up to a full stride ago.

`wolves.rs`'s `prey` map went the same way: it is a `HashSet<u32>` of identities now, and the hunt
picks the nearest by asking core, so a wolf chases the bunny's real position instead of a
remembered tile. `grep -rn 'HashMap<u32, (i32, i32)>' client/npc/src` is **0**.

**Found doing it — a live deadlock I had shipped** ([I12](issues.md#i12)): the I9 fix changed
`nearest_*` to pass the view into the predicate, and the wolf's food scan took the parameter and
ignored it, still calling `bot.tile_kind_at` inside the guard. `cell_open` takes the view now, so
the mistake cannot be made. The compiler had been printing `unused variable: view` the entire time
and I skimmed past it in a crate that already had a dozen warnings.

Live after the change: **33 move intents in 45 s, 0 errors**, log still advancing — a deadlock here
shows as the log simply stopping, which is how the first one was caught.

## 2026-08-10 — the unstreamed-cell law pinned, the rate seeded, the recenter carved out

**P2b item 3.** `pathable_reads_an_unstreamed_cell_as_open` pins the law through the FUNCTION, not
by asserting the constant equals itself — and its second half proves the law is about the unknown
and not a blanket yes: a streamed cell whose corpus kind is impathable still reads as a wall.

The plan item's premise turned out to be **wrong**, and that is worth more than the tick
([I11](issues.md#i11)): it said "the client reads unknown as OPEN, the worker does not". The worker
reads it as open too — its view comes from the DB, so "unknown" barely arises. The real divergence
was between the two CLIENTS, each answering it separately, and both now read `WorldView::pathable`.

**P2e item 4.** npc sends `SeedTicRate` at login. The seam has existed since movement-hardening F5
and **no headless host had ever called it**: a cold client spent ~60 s learning what the last one
already knew, and during that window every tic-derived answer was computed against the authored
6 Hz rather than the ~5.4 the durable tic actually runs at.

**P1 item 3** is ticked with the RECENTER carved out to [I10](issues.md#i10). The clamp and the
stride cap are covered by tests that fail if you `panic!` inside them; the recenter is not, and the
honest move is a named issue that owns proving it reachable or deleting it — not a criterion
quietly reworded to match what I managed.

## 2026-08-10 — the headless probe: core answers, with no browser in the room

`headless.rs` now reports what core ANSWERS every 5 s — the tic, every mover's point, its derived
pace and a need row read back. Live against the fluffle:

```
core answers now_tic=24647 movers=3
  pawn 0x30800006 x=126.000 y=71.938 pace=Some(24.0) need_rows=3 first_need=Some((.., 65535, 21567))
core answers now_tic=24678 movers=3
  pawn 0x30800006 x=127.345 y=73.598 pace=Some(24.0) ...
core answers now_tic=24703 movers=3
  pawn 0x30800006 x=127.625 y=74.312 pace=Some(24.0) ...
```

Subtile positions **changing between anchors**, the tic advancing at ~6.2/s, pace derived, and a
nonzero need value through `Client::pawn_needs`. Four criteria met at once, and the first time this
stream has shown its own thesis working with no browser involved.

**Three defects found getting there, all mine, all worth naming:**

1. **The fold DROPPED state rows without a corpus.** `observe_state` bailed early if
   `self.corpus` was `None`, so the probe reported `movers=0` while five pawns streamed. The
   position is the WIRE's truth; only the derived answers need a corpus. Fixed — the bundle is
   `Option` through the mover path and `WorldView::pathable` already had the fallback.
2. **The model owned a SECOND estimator.** My first "one clock" fix replaced an open-coded anchor
   tuple with a duplicate `TicEstimate` — still two clocks, and the model's one went blind because
   it only ever saw `TicAnchor`, which never fires in a zone with no pawn rows. The model now holds
   the ANSWER (`tic: Option<u16>`), stamped by the engine's estimator at every fold. One clock.
3. **I re-committed I9.** The probe held the world guard and called `pawn_point` inside the loop —
   the exact non-reentrant deadlock npc hit. Fixed, and the shape that invites it is gone:
   `Client::mover_entities()` returns owned ids so no host ever iterates under the guard.

That third one is the useful signal: the same trap caught two different callers, which means it was
the API's fault, not the caller's.

## 2026-08-10 — one clock for real, and two of `next_hop`'s dark branches

**`now_tic` goes through `ticclock::delta_since`.** The audit was right that core had grown a
SECOND clock inside the phase meant to remove second clocks: `ClientWorld` held its own
`(tic, wall_ms, rate)` tuple and open-coded the extrapolation beside it. It now holds the real
`TicEstimate` — with its learned rate, its poison-streak guards and its re-anchor window — and
`now_tic_at` asks `delta_since(0, now)`. That is the criterion's "first production caller", and it
means the headless client and the browser inherit the estimator's hard-won behaviour instead of a
naive multiply. `ticclock`'s own module doc no longer hands hosts the formula.

**`next_hop`'s CLAMP branch is covered** — `a_partly_blocked_segment_lands_on_its_clear_prefix`: a
pace-6 pawn whose 5.33-tile stride would carry it through a wall lands on the pathable prefix.

**The RECENTER branch is not, and I am not faking it** ([I10](issues.md#i10)). Two attempts failed
and taught something: `clear_point_fraction` does not count the START cell, so a pawn standing
inside a wall does *not* drive `clear` to 0 — it walks out eastward, correctly, which is now its
own test. The branch needs a segment whose first crossed cell is blocked while the lattice
corridor is not, and I could not construct one. Either that geometry exists and should be pinned,
or the branch is dead and should go. Contriving a passing test is exactly the habit the audit was
run to break.

## 2026-08-10 — the tick audit's corrections, and the defects it caught

An adversarial re-run of all 34 ticked criteria: **19 not clean, 15 outright false**
([`tick-audit.md`](tick-audit.md)). Nine items unticked, nine new items added for gaps no item
owned, and these **live defects fixed**:

- **Gameplay rows leaked.** `close_zone` never touched `self.rows` and `GameplayRows` had no zone
  key, so rows survived for the session. The auditor proved it (3 ≠ 0) before I did. Zone-keyed
  and evicted now, with `a_closing_zone_evicts_its_entities_rows`.
- **`Spec` and `SPEC_APPLY_EPS` still existed** in `MoverLayer` — and worse, I had rewritten that
  item's criterion *after* ticking it, and the rewrite dropped the clause covering them, so **no
  item owned the deletion any more**. Replaced with a minimal `Walk` record honest about what it
  still carries for the probe, plus `RENDER_APPLY_EPS`.
- **Core carried two clocks.** `now_tic` open-coded a second extrapolation off its own anchor —
  committed inside the very phase that exists to remove second clocks. One spelling now:
  `ticclock::extrapolate`.
- `debug.rs` still declared the `payloads` map and the `payloads.len() < 64` cap.
- `MoverLayer` re-spelled the 48-bit law as `need % 0x100000000`; `rowReference` is exported from
  the codec instead.
- `core/intent/sync.md:33` still instructed hosts to extrapolate locally — host-facing, and the
  same class of leak P2e was closing.

Also added the end-to-end read-surface test whose absence unticked P2c item 1:
`an_event_folded_yields_an_answer_and_no_clock_means_none` drives `TicAnchor` → `StateObject` →
`pawn_point` through the fold, asserts an unanchored clock answers **None** rather than 0, that
subtile survives, and that a removal takes the rows with it.

**The pattern worth keeping**, in the audit's words: acceptance text rewritten after ticking, and
substitutions logged in `completed.md` prose rather than `deviations.md` where the next session
looks — one of which hid the row leak for a day. Logged as [D4](deviations.md#d4).

`client/core` 49 passed, `client/npc` 3 passed, webgl typecheck + build green.

## 2026-08-10 — P4: the TypeScript walk is DELETED

`walkGreedy`, `walkPath`, `speedFor`, `computePath` and `firstLegClear` are gone from
`MoverLayer.ts` — `grep -c` is **0**. The client no longer walks, paces or paths: core does all
three and webgl chases the answer.

The pace now comes from core too (`pawnPace`), because the chase needs the RATE to cap itself —
a shared input, per [F2](forks.md#f2), unlike the position and the instant.

**Verified live.** Core's paces read 12/12 for the wolves and 24/24/24 for the bunnies — derived
in Rust, through `move_eval::ground_speed`, and handed to the browser. Driving `tick()`
synchronously (rAF is frozen in a background tab, and `setTimeout` there is clamped to 1 s — which
cost two timed-out probes before I recognised it), the chase closed **7.1 tiles in one second** and
converged to gaps of 0–0.08 tiles on the pawns that were near their targets.

The 12 chase snaps in that sample are the test's own artifact, not a defect: the tab had been
backgrounded for ~60 s, so the render was a minute stale and `CHASE_SNAP_TILES` did exactly what
its comment says it does for a hidden tab. A clean snap count needs a foreground soak.

## 2026-08-10 — P4: webgl READS ITS POSITIONS FROM CORE

`WorldClient` was write-only — commands in, events out through a callback, nothing askable, which
is why the browser folded its own world view, its own walk and its own clock. It now answers
`nowTic`, `pawnPoint`, `pathable`, `pawnPayload`, `pawnNeeds`, and takes `setCorpus`.

`MoverLayer`'s per-frame target is now `this.client.pawnPoint(key)` — **core's walk, through
`move_eval`, the same answer `client/npc` gets**. The TypeScript walk no longer decides where
anything is. The render-chase stays, because how fast a sprite ADMITS a correction is the only
part of this a viewer owns ([F2](forks.md#f2)).

Needs cross the boundary as stride-2 `Float64Array`, never `Uint32Array`: a need row is a u64 and
the u32 form truncated `0xa4fb80010020` to `0x80010020`, after which every need read zero.

**Verified live in the browser**, not just compiled: 5 movers, **core answered a position for all
5**, all 5 moved over a 6-second sample, positions subtile (`125.04, 72.12`), `coreNowTic` 3236.
`npm run typecheck` + `npm run build` green.

Still open in P4: deleting the now-unused `Spec`/`walkGreedy`/`walkPath`/`speedFor`/`computePath`
machinery, and pointing the panel readers at the row accessors.

## 2026-08-10 — P2e items 1-3: one clock

`Client::now_tic()` is the answer; `Bot::now_tic` delegates and npc's `tic_anchor` field, its
`TicAnchor` arm and its local extrapolation are deleted (`grep -c tic_anchor` is 0).

`Event::TicAnchor`'s doc is re-labelled **DIAGNOSTIC** and the extrapolation formula is removed
from it. That doc was handing every host the recipe to rebuild a shared answer, and both hosts took
it — npc kept an anchor, webgl hand-rolled the u16 conversion eight times. **An instruction to
re-derive a shared answer is a leak written into the contract**, which is the sharpest thing the
audit found.

`tics_per_sec` stays readable, deliberately: a renderer needs the RATE to pace its smoothing chase
([F2](forks.md#f2)). The rate is a shared input; only the evaluation INSTANT is core's answer.

Live: **38 move intents in 60 s, 0 errors** with npc taking its clock from core. Item 4 (npc
calling `SeedTicRate` at login) is left open.

## 2026-08-10 — P2d COMPLETE: the gameplay rows are core's, and I8 is closed

[`client/core/src/gameplay_rows.rs`](../../../client/core/src/gameplay_rows.rs) — payload and need
rows keyed by ENTITY, not hung off `Mover`. That placement was **this stream's own instance of the
defect it exists to delete** ([I8](issues.md#i8)): I put the rows where their first consumer needed
them (pace derivation) rather than where they belong, and two real bugs followed — rows arriving
before a pawn's first anchor were dropped (leaving it at `pace: None`, held still forever), and
`PawnNeed` fans for player-pawn references that have no position to key on at all.

Both are now regression tests: `rows_arriving_before_any_position_are_kept`,
`a_positionless_player_pawn_reference_is_storable`, and — driving the whole path —
`rows_before_the_anchor_still_reach_the_pace`, which feeds the payload FIRST in the zone-snapshot
order that used to lose it and asserts the pace still derives to 24.0.

Needs upsert by `codec::object::row_reference`, not a second open-coded modulo. One eviction rule
on `StateGone`/removal. `pawn_payload`/`pawn_needs` on both hosts' handles.

**The fifth copy is gone.** `wolves.rs` and `bunnies.rs` no longer keep `payloads`/`need_rows`;
they read core through the handle, and the `PawnParts` arm that existed to buffer pre-adoption
rows is deleted — core buffers for every host now. The arbitrary `payloads.len() < 64` cap went
with it.

**Verified live, not just compiled.** Fresh world, full sim rebuilt: **24 move intents in 60 s, 0
errors, worker in step** (`tic=605 master=604`) — with wolves and bunnies deriving every trait,
condition and need row through core. `client/core` 47 passed, `client/npc` 3 passed.

## 2026-08-10 — P2c COMPLETE: npc's fold is gone; one model, both hosts

Retried item 4 with [I9](issues.md#i9) fixed at the root. `WorldView::nearest_tile`/`nearest_thing`
now hand the view **into** the predicate (`Fn(&WorldView, …)`), so a caller holding the view behind
a lock cannot reach back through its own handle for a follow-up question. Re-entry no longer
deadlocks because it no longer compiles — which is the version of that fix worth having.

`Bot`'s `world` field and its four `Event::` fold arms are deleted; the accessors read
`self.client.world()`. npc hands core the corpus at login. `grep -c 'self.world'` in
`client/npc/src/lib.rs` is 0. Four brain predicates updated to take the view.

`client/core` 42 passed, `client/npc` 3 passed, `bin/sim check npc` green.

_Superseded: the first attempt and its revert._

## 2026-08-10 — P2c item 4 ATTEMPTED and REVERTED

Deleting npc's fold and pointing its accessors at core's model **deadlocks** — the brain
predicates passed to `nearest_*` call back into `Bot`, re-entering a lock the query already holds
([I9](issues.md#i9)). Reverted to the P2b delegating form; core's P2c work (items 1-3) is
untouched and green.

Also observed while verifying: the worker had drifted ~4,000 tics behind again
([I6](issues.md#i6) recurring, ~10 h after the reset). The revert did not restore npc's move
intents, so the stall I attributed to the deadlock was at least partly the degraded world. Both
want a fresh world before item 4 is retried.

## 2026-08-10 — P2c items 1-3: a host can ASK core, and the fold happens once

**The read half of the contract exists.** `Client` was `cmd_tx`-only on BOTH hosts — commands in,
events out, nothing readable — which is the structural cause the audit named: a host that cannot
ask has to fold the answer itself, in its own language. `Client` now carries a
`SharedWorld` and answers `pawn_point(entity)`, `now_tic()`, `pathable(x, y)`, `world()`.

**THE fold is one function at one choke point.** `ClientWorld::observe_event` folds state, intents,
payloads, needs, cold tiles, cold things, cold overlays and zone closes; both engines call it from
their single `emit`, *before* the host sees the event. Neither host can fold differently because
neither host folds.

**Core owns the corpus and the clock.** `derive_paces`/`pathable` no longer take a `&Bundle` from
the caller — a corpus parameter is an invitation for two hosts to pass two different corpora.
`now_tic_at()` answers the clock, so `api.rs`'s instruction to extrapolate locally stops being the
only way to get it.

**One bug avoided by target, worth recording:** the anchor first held a `std::time::Instant`.
`Instant::now()` **panics on `wasm32-unknown-unknown`** — the browser would have died on its first
tic anchor. It now takes an `f64` wall-ms stamp from each engine's existing `now_ms()`.

**Verified.** `client/core` 42 passed; `bin/rd build core` and `bin/rd build shared` (the wasm
bundle, which compiles `web.rs`) both green; `bin/sim check npc` green; `grep -c ClientWorld` is 2
in `engine.rs` and 2 in `web.rs`.

**Deviation recorded:** core owns the corpus but does not fetch it ([D3](deviations.md#d3)) — the
GET stays host-side rather than forking core on `#[cfg]`, which would be two implementations of
exactly the kind this stream deletes.

## 2026-08-10 — P2 item 3: `ClientWorld`, the one object a host drives

[`client/core/src/client_world.rs`](../../../client/core/src/client_world.rs) joins the
[`WorldView`](../../../client/core/src/world_view.rs) and the
[`MoverTrack`](../../../client/core/src/movers.rs) behind one surface: `observe_state`,
`observe_intent`, `close_zone`, `derive_paces`, `pathable`, and **`pawn_point(entity, now)`**.

Written because the join itself is a thing that can be got wrong twice. Which probe feeds the
track, when paces re-derive, what a closing zone drops — a host wiring those together itself would
be the third copy of exactly the logic this stream is deleting. `close_zone` is the sharp one: it
must clear BOTH halves, since a mover left in a closed zone is a ghost that never moves again and
a stale tile row is a wall that is no longer there.

**What it deliberately does not do is smooth.** `pawn_point` returns where the pawn IS — a step
function corrected at each anchor. Making that pleasant to look at is the viewer's job, which is
[F2](forks.md#f2)'s line and why the render-chase stays in webgl.

**Verified.** `a_host_can_ask_where_a_pawn_is_between_anchors` drives the real contract against the
real corpus: an unpaced pawn HOLDS at its anchor; once its payload is minted the pace derives to
24.0; and 48 tics later it has advanced exactly two tiles. Plus `closing_a_zone_clears_both_halves`.
42 tests pass in `client/core`.

**P2 is complete.** Core can answer "where is that pawn" and "may one stand here" without a
browser, which is the whole of what webgl will consume in P4.

## 2026-08-10 — P2b COMPLETE: npc reads core's view; the duplicate is gone

`client/npc`'s `tiles` / `tile_overlays` / `things` maps are **deleted**. The Bot holds a
`client::world_view::WorldView`, its four event handlers delegate to it, and
`tile_kind_at` / `nearest_tile` / `thing_kind_at` / `nearest_thing` are one-line delegations.
`grep -c 'self.tiles\|self.things\|self.tile_overlays'` is **0**.

**Two things the rewire caught that a straight lift would have shipped broken.**

1. **`ColdTiles` rows MERGE cell-wise, nonzero winning** — a zone streams one row per BIOME, each
   carrying only its own cells (zone 99 arrives as three). My first `observe_cold_tiles` did an
   `insert`, which would have kept whichever biome streamed last and blanked the rest. Caught by
   reading the real handler rather than trusting the lifted shape; now pinned by
   `cold_tile_rows_merge_cell_wise`.
2. **A THING baseline must not clobber an override** — a `ColdThings` row arriving after a felling
   would resurrect the tree. Split into `observe_thing_baseline` (first write wins) and
   `observe_thing` (override wins, 0 suppresses), pinned by
   `a_thing_baseline_never_clobbers_an_override`.

`nearest_thing`'s predicate also had to widen to take the world CELL, not just the kind: brains
refuse unreachable food that way (meat in a lake), and without it a brain oscillates forever
between the worker's refusal and its own wander.

**A pre-existing failure fixed in passing.** `bin/sim check npc` was already broken —
`the_wolfs_derived_ground_speed_is_the_old_authored_pace` still used `TraitBind::level`, deleted by
trait-rows-u32, so the npc test target had not compiled since that migration. Rewritten onto
`move_eval::ground_speed`, so it now pins 12.0 tics/tile **through the shared path** instead of a
local reassembly of it. Not my breakage, but squarely this stream's subject.

**Verified.** `client/core` 40 passed, `client/npc` 3 passed, `bin/sim check npc` green. Rebuilt
and put back on the live world: pawns adopted, group counts written, move intents flowing across
zones, **0 errors or panics**.

## 2026-08-10 — P2b items 1-3: core holds the world, and the unknown-cell law is pinned

**Landed** [`client/core/src/world_view.rs`](../../../client/core/src/world_view.rs) — npc's
composed view pulled up unchanged in behaviour: baseline tiles ⊕ cold overlays, the composed thing
layer (where a stored **0 SUPPRESSES** — the felled tree — so absent and zero cannot mean the same
thing), `tile_kind_at`, `thing_kind_at`, `nearest_tile`, `nearest_thing`, and **`pathable`**
derived from the corpus's own `tile_pathable`/`thing_pathable` predicates.

**The unstreamed-cell law now exists as one named constant**, `UNKNOWN_CELL_IS_PATHABLE = true`,
with the reasoning attached: an unseen cell is unseen, not a wall, and treating it as blocked would
stop every pawn pathing beyond its own streamed window. It was previously answered independently by
each host, which is exactly how two clients come to disagree about the same tile.

**`MoverTrack` no longer takes a pathability closure.** It takes the `WorldView` and derives the
probe itself — because a host that supplies its own probe can supply a *different* one, which is
this stream's defect one level down. `grep -c 'pathable: &dyn' client/core/src/movers.rs` is 0.

One small improvement over the lifted original: `nearest_*` breaks ties on the lowest `(x, y)`.
Two hosts scanning one view must agree on WHICH cell, not merely on the distance — an unstable tie
is the difference between a reproducible brain and a coin flip.

**Verified.** `bin/rd build core` green; the full `client/core` suite is **39 passed, 0 failed**,
including five new world-view tests (overlay beats baseline, zero-suppression, unseen ≠ empty,
zone forgetting drops overlays and things, Chebyshev-with-stable-tie).

**Still open in P2b:** replacing `client/npc`'s own `tiles`/`tile_overlays`/`things` maps with
reads of core's — the deduplication this phase exists for. Core now has the view; npc still has its
copy.

## 2026-08-10 — P2 item 2: pace is derived, and a THIRD mirrored pair collapses

`move_eval::ground_speed` is now THE pace. Found while wiring the track: the worker's
`ground_speed_tics` and the wasm bundle's `pawn_ground_speed` were **a second mirrored pair** —
each assembling the same trait rows and calling the same `stat_eval` in its own words. They
happened to agree, which is exactly the condition under which a divergence goes unnoticed; the
2x that opened this stream was invisible for the same reason until the measurement was split by
kind.

The track stores each pawn's raw payload and `needs` rows (`observe_payload`/`observe_need`) and
`derive_paces(bundle, now)` evaluates the unpaced ones. A trait or need change clears the cached
pace, so a condition that slows a stat slows the walk — the property the worker gets by
re-evaluating per hop.

Also added `move_eval::advance_along`: the tail of `position_at`, split out so the track can
compute a leg ONCE when the intent arms and interpolate it per frame **through the same
arithmetic** rather than an equivalent-looking reimplementation. That distinction is the whole
subject of the module — the cheap path and the correct path must be the same code.

**Verified against the real corpus**, not a fixture: `bunny_and_wolf_derive_their_authored_paces`
loads `content/` off disk, mints each species' payload the way `CREATE` does, and asserts
**bunny 24.0 / wolf 12.0** tics/tile — `walks` levels 1 and 2 of `add = [24, 12, 6]`. It skips
loudly rather than silently passing if the corpus is absent. All 7 track tests green,
`bin/rd build core` green.

## 2026-08-10 — P2 items 1+4: the Rust client can finally answer "where is that pawn"

**Landed** [`client/core/src/movers.rs`](../../../client/core/src/movers.rs): a `MoverTrack`
holding, per entity, the last authoritative point + the tic it was stamped + the live intent's
destination + the derived pace, and answering `point_at(entity, now, pathable)` through
`move_eval::position_at`. `client/core` now depends on `resonantdust-content`.

**It computes no motion of its own** — that is the design, not an omission. The track holds what
the wire says and delegates every question of movement to the shared module, which is exactly the
property `MoverLayer.ts` lacks and why this stream exists.

**Two guards ported from the TypeScript**, which earned them live and would otherwise have taken
them to the grave (`MoverLayer.ts` :754-776, :839) — note that `client/npc` never had either, so
this is the first time a headless consumer is protected from them at all:

- an older row never applies (a zone re-subscribe replays STATE history; a stale row landing after
  a fresh one drags the anchor backwards, which reads downstream as a teleport);
- a replayed intent never arms (the same replay re-delivers minutes-old `MOVE_TO`s; a live intent
  trails the freshest row by the queue barrier, a replay by hundreds).

**One deliberate departure from the old behaviour.** An unpaced pawn is held STILL rather than
walked at `DEFAULT_TICS_PER_TILE`. The TypeScript's fallback is an 8x error on the pawns it is
wrong about ([I2](issues.md#i2)) — a pawn that sits still for a frame is invisible, one that
sprints and gets yanked back is not. This is what closes I2 at P6.

**Verified.** `bin/rd build core` green; `cargo test movers::` — 6 passed, covering the walk
between anchors (12 tics at 24 t/t = half a tile), the unpaced hold, both replay guards, arrival
clearing the walk, and a closed zone dropping its pawns.

**Still open in P2:** deriving pace through `stat_eval` inside core (it is settable today, not yet
derived), and exposing `pawn_point` on the client API with a headless run to prove it.

## 2026-08-10 — P1 COMPLETE: the worker no longer owns the walk

`MOVE_STEP`'s chord block and `resolve_walk_position_for`'s body are now single calls into
`move_eval::next_hop` / `move_eval::position_at`. About 95 lines of movement logic left
`server/worker/src/main.rs`. What remains under `server/` is the shared `use`, one prose comment,
and the hop-queueing site calling the shared `hop_stride_tiles`. **There is one walk in the tree.**

**Verified — a pure move that changed nothing**, which was the item's whole point:

| anchor stride p50 | baseline (before) | after the extraction | delta |
|---|---|---|---|
| bunny | 1.326 | **1.318** | 0.008 |
| wolf | 2.652 | **2.653** | 0.001 |

Both inside the 0.05-tile tolerance; implied pace unmoved (24.27 / 12.06); cadence still 32.
`bin/sim check worker` green, worker rebuilt and restarted in step (`tic=5938 master=5936`).

Reseed error is unchanged too — bunny p50 1.31, wolf p50 2.63, and reseeds past
`CHASE_SNAP_TILES` still ~9-19%. That is the correct result: the extraction was never going to fix
the drift, because both sides still run *their own* code. The client's copy dies in P4, and that is
where the number should move. Recorded so nobody reads this phase as a fix.

## 2026-08-10 — P0 complete: the BASELINE, on a reset world

World reset (`bin/rd redeploy --run`, all module DBs wiped, all four sim crates rebuilt and
restarted). Worker **in step** throughout: lag +1/+2 across the whole run, never the ±1400 of
[I6](issues.md#i6). Population is small on a fresh world — 3 bunnies, 2 wolves — so this is a
per-pawn baseline, not a load baseline. ~15 minutes, **551 samples**.

| | bunny (pace 24) | wolf (pace 12) |
|---|---|---|
| implied pace, p50 (p10 / p90) | **24.26** (23.77 / 28.44) | **12.07** (11.91 / 15.41) |
| anchor stride p50 / predicted | 1.326 / 1.333 | 2.652 / 2.667 |
| anchor gap, p50 tics | 32 | 32 |
| reseed error p50 / p90 / max | **1.25** / 4.60 / 12.53 | **2.13** / 8.00 / 20.00 |
| reseeds past `CHASE_SNAP_TILES` | 45 of 387 (**11.6%**) | 24 of 160 (**15.0%**) |
| RENDER teleports (chase snaps) | 0 | 0 |

**Read it carefully, because it says two things and they pull in opposite directions.**

*The pace agrees.* Implied vs client pace matches to ~1% at the median, and the observed stride
matches the predicted one to within 0.015 tiles for both species. This is what retracts
[I1](issues.md#i1)'s 2× premise.

*The belief still drifts multiple tiles.* With the pace correct and the cadence exactly 32 tics,
the client's speculated position is still a median **1.25–2.13 tiles** from truth when the
authoritative row lands, with a p90 of 4.6–8.0 and a worst case of 12–20 tiles. **Between one in
nine and one in seven reseeds exceeds the render-chase's give-up distance.** The chase absorbed all
of them in this window (0 RENDER teleports) — so the world currently *looks* fine — but a client
whose belief is routinely 2 tiles wrong is not simulating the same world the server is, and the
margin protecting the viewer from that is one constant.

So the divergence is real and it is **not** a pacing divergence: it is geometry, path consumption,
or lifecycle. That is the target for the rest of the stream, and P4 re-runs exactly this table.

**Method note.** Per-kind bucketing is the whole reason this baseline says something the first one
didn't. Pooling species produced the 2× that sent me chasing pace for an evening
([I1](issues.md#i1)); a bunny at 24 and a wolf at 12 in one distribution describe nothing real.
Reload persistence verified live mid-soak: 1058 samples before `location.reload()`, 1082 after,
still climbing.

## 2026-08-10 — P1 items 3–4: `next_hop` and `position_at`, the walk in one place

**Landed** in [`move_eval.rs`](../../../shared/content/src/move_eval.rs): `next_hop` (the worker's
`MOVE_STEP` chord step — first chord, `clear_point_fraction` clamp, lattice recenter, stride cap,
exact arrival on the destination tile) and **`position_at`** — where a walking pawn IS at an
arbitrary tic. `position_at` is the one that did not previously exist as shared code anywhere: the
worker had `resolve_walk_position_for` privately, the TypeScript had its own re-derivation, and the
npc had nothing, which is precisely why headless brains could not see a pawn between anchors.

Both work in fractional world POINTS, not `position_reference`s, so callers quantise at their own
edge and the module stays codec-light and usable from wasm.

**Verified.** 9 tests pass in `cargo test -p resonantdust-content move_eval::`. The load-bearing
one is `position_at_the_hop_tic_equals_the_hop_landing`: sampling the walk at the hop's own tic
must land exactly where the hop lands, across 6 cases (axis-aligned, pure diagonal, off-lattice
start, stride-capped long trip, single tile, both authored paces). If those two ever disagree, an
observer interpolating and a server stepping are two implementations again. Also pinned: one tile
per `pace` tics as distance-over-time; exact arrival with no overshoot; a future `base_tic` reading
as elapsed **zero** rather than most of a ring; a degenerate pace holding position; a blocked world
yielding no hop.

**Acceptance substituted, deliberately.** The items said "reproduces 20 landings recorded from the
live worker". Recording those needs new worker instrumentation, and it would only prove the shared
code matches the code I transcribed it from. I asserted the stronger property instead — internal
consistency between the two functions plus the pace contract stated independently of the stride —
and left the worker's own behaviour to be proven where it actually matters, by P1 item 5 running
the live world against the P0 probe. Flagging it because it is a real departure from the written
criterion, not a quiet reinterpretation of it.

**Two asymmetries found in the worker while transcribing**, both preserved rather than silently
"fixed", because changing behaviour during an extraction is how a refactor becomes a bug hunt:
`next_hop` recenters on the lattice when the direct segment is fully blocked while
`resolve_walk_position_for` merely clamps to zero; and `next_hop` snaps exactly onto the
destination point on the dest tile while the resolve returns the current position. Worth a look
when P4 re-measures.

## 2026-08-10 — P1 items 1–2: the shared module exists, and the worker is a caller

**Landed.** [`shared/content/src/move_eval.rs`](../../../shared/content/src/move_eval.rs) — THE
walk, beside `path_eval`/`stat_eval`/`needs_eval` per [F1](forks.md#f1). Holds `REANCHOR_TICS`,
`CHORD_CAP_TILES` and `hop_stride_tiles`, exported from `lib.rs`. `server/worker/src/main.rs`'s
private constants and function are **deleted**; it now `use`s the shared one, so the cadence and
the stride have exactly one definition in the tree. The only surviving mention of `REANCHOR_TICS`
under `server/` is a prose comment.

**Verified.** `cargo test -p resonantdust-content move_eval::` — 3 passed, 0 failed:

- `stride_clamps_at_both_ends` — pace 240 → 1 tile, pace 1 → 8 tiles (the item's criterion
  literally), plus the zero-pace divide guard and the exact cap boundary at pace 4.
- `stride_matches_the_authored_paces` — 24/12/6 (`walks` levels 1–3) pin 4/3, 8/3 and 16/3 tiles,
  so a `walks` retune cannot change the fan rate silently.
- `the_cadence_bounds_every_hop_except_a_floored_one` — the honest statement of the promise, and
  it caught my own first draft: I asserted the cadence bounds *every* hop, which is false for a
  pawn slower than one tile per cadence, where the one-tile floor wins and the hop necessarily
  overruns. **That case matters to this stream directly** — a divergence probe that assumes 32
  tics between anchors will mis-read a slow pawn as a stalled one.

`bin/sim check worker` green (one pre-existing unrelated `unused import` warning).

**Not done in this phase.** Items 3–5 (`next_hop`, `position_at`, the `MOVE_STEP` rewrite) are
blocked with P0: their acceptance is "reproduces 20 landings **recorded from the live worker**",
and there is no live worker to record from ([B2](blockers.md#b2)).

## 2026-08-10 — P0 probe code landed; acceptance BLOCKED on a live world

**No items ticked.** The code is in and verified as far as it can be without a running sim; the
acceptance criteria all require live samples and the world cannot currently produce them
([B2](blockers.md#b2)). Recording it so a resuming session does not rewrite it.

**Landed** in [`MoverLayer.ts`](../../../client/webgl/src/game/world/MoverLayer.ts): a per-kind
divergence tally on `__teleportProbe` — `reseedErr`, `anchorStride`, `anchorGap` and `impliedPace`
as bounded sample arrays, plus `report()` (quantiles per kind, with the client's `clientPace`
printed beside the `impliedPace` the server's own rows imply — the comparison
[I1](issues.md#i1) turns on) and `reset()`. Mirrored to `sessionStorage` on a 2 s throttle and
flushed on `pagehide`, because the dev server's reloads had already silently restarted two
measurement runs.

**Verified:** `npm run typecheck` and `npm run build` green. `reset()` writes the expected shape to
`sessionStorage` (`{"divergence":{},"auth":0,"render":0}`), and the restore path runs at
construction — so the persistence *mechanism* works, though it has not yet carried a **non-zero**
count across a reload, which is what item 2's criterion actually asks.

**Not verified:** `report()` printing quantiles off real samples, and the 10-minute baseline. The
first attempt returned **zero `StateObject` rows in 30 s** with 90 movers resident — which is what
uncovered [I6](issues.md#i6) and [B2](blockers.md#b2) rather than any fault in the probe.

**Also learned, the expensive way** — `bin/sim run <crate>` runs the *already-built* binary, so
restarting a long-lived sim process is `build` then `run`; and a second session is committing to
this branch concurrently. Both written up in [I6](issues.md#i6).

## 2026-08-09 — the survey that opened the stream

Not a plan item; the diagnosis P0 will re-measure against. Recorded because it is the evidence the
stream's stance rests on, and because the numbers are perishable — they came off a live world.

**Setup.** `rd-host` running `NPC_MODULES=wolf_pack@112,68;bunny_fluffle@124,75`; client at
`:5174/?user=Claude&focus=124,75&zoom=1`; 90 movers resident — **86 bunnies** (kind 13), 3 wolves
(kind 7), 1 bunny still pre-fan.

**How checked.** Console probes over `__moverLayer` / `__movers` / `__teleportProbe`, tallying
`[mover] spec reseed` lines and pairing consecutive authoritative `StateObject` rows per entity;
`docker logs rd-worker` over a 10-minute window; `spacetime sql resonantdust-dev-pawn-0`.

| measurement | value |
|---|---|
| client-derived pace, bunny | 24 tics/tile (86 of 90 movers) — correct for `walks` level 1 |
| client-derived pace, outlier | 3 tics/tile on one mover = `DEFAULT_TICS_PER_TILE` ([I2](issues.md#i2)) |
| authoritative row gap | p50 **32 tics** — the `REANCHOR_TICS` cadence, as designed |
| displacement between rows | p50 **2.67 tiles**, p90 5.41, max 7.85 — 24 tics/tile predicts 1.33 ([I1](issues.md#i1)) |
| spec reseed error | p50 **3.4–4.0 tiles**, p90 8.3, max 13.1 |
| reseeds past `CHASE_SNAP_TILES = 3` | **116 of 214 in 54 s** — the visible snapping |
| worker `intent queue REPLACED` | 731 vs 1157 move intents / 10 min ([I4](issues.md#i4)) |
| worker `mid-chord resolve` | 35 / 10 min ([I4](issues.md#i4)) |
| worker duplicate work-group WARNs | 936 / 10 min ([I5](issues.md#i5)) |
| worker compose lag behind master | p50 ~1 tic, tail to 16 |

**Ruled out.** The trait level round-trips correctly — a live bunny's stored payload decodes to
five `TRAIT` entries, all `data = 0`, all variant nibble **0** → tier 0 → level 1 → 24 tics/tile;
and both sides evaluate it through the same `stat_eval::stat_value`. A stale worker corpus is also
out: `rd-worker` booted 21:08:35Z alongside the `forager` commit. **The shared calculations agree;
the unshared one does not** — which is the stream's argument, arrived at from measurement rather
than from principle.

**Not fixed, deliberately.** [I1](issues.md#i1) is left un-isolated per [F5](forks.md#f5): the
single shared rule is what makes the question unaskable, and P4 must show the divergence gone.
