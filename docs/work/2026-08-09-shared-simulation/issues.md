# Issues — shared-simulation

_Problems hit, candidate solutions, which we chose and why. Chronological append._

## I15 — `pawn_point` answers `None` until the clock anchors, and callers read that as "no pawn"
**2026-08-10. Open — found at the end of the I14 chase; the remaining half of npc's silence.**

Adoption now logs `positioned=false` for every pawn. That is not a missing row: `Client::pawn_point`
does `let now = w.now_tic_at(..)?` and the tic estimate has not anchored one second after boot, so
it returns `None` for pawns whose rows core is holding perfectly well.

**The shape of the mistake is worth more than the fix.** `None` is doing two jobs — "I don't track
this pawn" and "I can't date the answer yet" — and every caller collapses them to "no pawn". The
adopt gate did exactly that and emptied the fluffle.

Two candidate fixes, and the second is better: return the last authoritative point when the clock
is unanchored (a position that is merely un-advanced beats no position), or split the API so
"where is it" and "where is it AT TIC T" are different questions. The second matches
[F9](forks.md#f9), where positions arrive stamped and the client never asks "where is it now".

**Still unexplained:** with adoption fixed and the stale-walk bound in, npc issues 0 move intents.
The gate is somewhere after the busy check in `Bunnies::tick`. Not chased — F9 re-plumbs this path.

## I14 — npc adopts NO pawns; `minds` stays empty (NOT a deadlock — I misdiagnosed it twice)
**2026-08-10. OPEN, root cause identified, fix not written.**

**The symptom I chased for an hour was wrong.** I read "npc stops logging at 0.06% CPU" as a lock
deadlock and bisected for it. It is neither a deadlock nor a hang: the process is idle because it
has **nothing to do**. `group count written (npc-host F8) count=0` — `self.minds.len()` is **0**
while six `module owns pawn` events arrive. No adopted pawns means no per-pawn tick, so no orders,
no logs, and no CPU. A quiet process and a wedged one look identical from the outside, and I never
checked which I had.

**The likely cause, and it is mine:** adoption runs through `Bot::pawn_at`, which P3 re-pointed at
core's mover track after deleting the Bot's own `pawns` map. If the track has no row for an owned
entity — because the fold populates it from the session that actually streams that zone, and the
host's own session may not — `pawn_at` returns `None` and the adopt silently fails. The old map
was filled from the same events but the deletion changed *which* session's stream fills it.

**Next step:** log `pawn_at`'s result at the adopt site, then either feed the module session's
track or adopt from the `OwnedPawn` event without needing a position first.

_The three re-entrancy fixes made while chasing this are real and worth keeping_ — the eat scans
genuinely did fetch rows under the scan's own lock, and `nearest_*` now snapshots rather than
holding the lock across a caller's predicate. They were just not this.

_Original (incorrect) diagnosis, kept because the ruling-out is still useful:_

`rd-npc` stops emitting ~1 s after boot and sits at **0.06% CPU** — a lock wait, not a spin. It
does not recover. Reproduces with a single module (`bunny_fluffle` alone), so it is not a
cross-module interaction.

**Where it stops:** immediately after the per-second `module's own player-pawn row` line and
before anything else in `Bunnies::tick`. The group-count block above it is skipped (the count is
unchanged), so the first thing it actually reaches is the per-pawn loop — whose first call into
core is `Client::pawn_busy`.

**Three re-entrancy sites found and fixed on the way, none of which was it:**

1. `Bunnies::usable_eat` → `rows_of` → `pawn_payload` → locks, called inside `nearest_thing`'s
   predicate while that scan holds the lock. Hoisted the row fetch out of the scan.
2. The same shape in `Wolves::usable_eat` via `payload_of`. Same fix (`eval_rows`).
3. Then I stopped patching call sites and removed the hazard: `nearest_tile`/`nearest_thing` now
   SNAPSHOT candidates (`WorldView::tile_cells`/`thing_cells`), drop the lock, and filter. A
   predicate can call anything.

It still hangs. So the remaining holder is something other than a scan predicate.

**What I have not ruled out**, in the order I would look:
- `Engine::emit` holds the world guard while calling `sink.emit`; if any sink path re-enters the
  handle the engine and the bot deadlock against each other. The npc sink is an unbounded channel
  send, which should not block — *should* is doing work in that sentence.
- A guard held across an `.await` somewhere in the engine task.
- `Client::pawn_busy`/`arm_pending`'s early-return paths (`let ... else { return }` inside an
  `if let Ok(mut w)`).

**It predates today's uncommitted work.** After `git stash`, HEAD alone still hangs — so it is in
one of the P2d–P3c commits, all of which were verified live at the time. The most likely candidate
is the commit that made brains call `pawn_busy` per pawn per tick, since that is the first
per-pawn lock npc had ever taken.

**Bisect not run** — that is the next step and it is cheap: check out the `arm_pending` commit
(verified at 13 intents/40 s) and walk forward.

The stash `P3b ETA busy-join — wedges every pawn` also holds a SECOND, separate defect: that join
froze every pawn by reading a never-cleared `dest` as permanently committed. Do not restore it.

## I13 — the wolf-chase criterion cannot be met in a 10-minute soak, by construction
**2026-08-10. PLAN ERROR — criterion reworded; the underlying check still owed.**

P3's "a wolf closes on a moving bunny without overshoot across a **10-minute soak**" never fired in
two consecutive soaks (`food=0` in both). Not a bug in the chase — arithmetic:

`hunger` authors `deplete = 43200.0` and a `hungry` band of 10–35 (`content/needs.toml`). A wolf
mints FULL, so it must drain 65 of 100 units before the hunt triggers: `0.65 × 43200 ≈ 28,000
tics ≈ **78 minutes** at 6 Hz. A ten-minute window cannot contain the event it is asserting about.

Nor can I force it from the client: `SET_NEED` is not in the edge's verb allowlist
(movement-hardening), which is correct — a client that could rewrite a pawn's needs is a client
that can cheat. Queuing it from the browser was silently dropped, as designed.

**Reworded** to what is actually checkable: the SELECTION property (the hunt picks its target from
core's subtile point, unit-testable) plus a conditional soak assertion (IF a hunt fires, the
wolf→prey distance decreases). The 78-minute observation is worth having and belongs in a long
soak, not in this stream's inner loop.

**The general lesson for this plan:** an acceptance criterion that names a duration should be
checked against the timescale of the behaviour it waits for. Three of this stream's criteria say
"10-minute soak"; only one of them is about something that happens every few seconds.

## I12 — the I9 fix left a live deadlock in the wolf path, and the compiler said so
**2026-08-10. Fixed.**

I9's fix changed `nearest_*` to hand the view INTO the predicate so a caller could not re-enter
the world lock. The wolf's food scan took the new `view` parameter and **ignored it**, still
calling `self.cell_open(bot, cell)` — which calls `bot.tile_kind_at`, which locks. Inside a
predicate that is already holding that lock. A latent hang, shipped, in the path a hungry wolf
takes.

**It was reported the whole time**: `warning: unused variable: view` at `wolves.rs:428`. I read
past it because the crate had a dozen pre-existing warnings and I had stopped looking at them.

`cell_open` now takes `&WorldView`, so the mistake cannot be made — the type says where the answer
comes from. Its other caller, `pick_pathable_dest`, takes the guard once for its whole retry loop
instead of once per probe.

**The lesson is not "be careful with locks":** it is that a signature change without a body change
compiles, runs, and only hangs when a wolf gets hungry near food. The unused-parameter warning is
the tell, and a crate with warnings I have learned to skim is a crate where that tell is free to
hide.

## I11 — the unstreamed-cell rule, pinned; and the divergence it was written against
**2026-08-10. Resolved for the clients; the worker never had the question.**

**The law: an UNSTREAMED cell is PATHABLE.** One named constant,
`world_view::UNKNOWN_CELL_IS_PATHABLE`, with the behaviour pinned through `pathable()` itself
(`pathable_reads_an_unstreamed_cell_as_open`) rather than by asserting the constant equals itself.

A client only knows the zones it subscribed. Treating an unseen cell as a wall would stop every
pawn pathing toward anything beyond its own streamed window — worse and wronger than occasionally
routing into something the server then clamps, which is a case the worker already handles.

**On the divergence the plan item assumed.** P2b's item said "the client reads unknown as OPEN, the
worker does not". That premise is false: the worker's own probe reads an unknown cell as open too
(`server/worker/src/main.rs`), because its view comes from the DB and "unknown" barely arises.
The real divergence was between the two CLIENTS — npc and webgl each answered it separately —
and that is closed by both now reading `WorldView::pathable`.

## I10 — `next_hop`'s RECENTER branch has no reachable test and may be dead
**2026-08-10. Open — found while closing the tick audit's P1 finding.**

The audit proved the branch is unreached: a `panic!` inserted at `move_eval.rs`'s
`if clear <= f64::EPSILON` fails nothing. Trying to reach it showed why. The branch fires only
when `clear_point_fraction` returns ~0 for the segment to the first chord waypoint, and two rules
work against that:

- **The start cell is never probed** (`path_eval` F6), so a pawn standing INSIDE a wall does not
  drive `clear` to 0 — verified: from `(11.5, 11.5)` in a blocked column it hops east, correctly.
- **The chord's waypoint is lattice-reachable by construction**, so the direct segment to it is
  usually clear too; it takes an off-centre start whose FIRST crossed cell is blocked while the
  lattice corridor around it is not.

The sibling CLAMP branch (`f.min(clear)` for `0 < clear < 1`) **is** now covered —
`a_partly_blocked_segment_lands_on_its_clear_prefix`.

So: either a geometry that reaches the recenter exists and should be pinned, or the branch is dead
and should be deleted with the shoreline comment that justifies it. **Not resolving it by
contriving a test that passes for the wrong reason** — that is the habit the tick audit was run to
break. Worth an hour with `clear_point_fraction`'s own traversal, not a guess.

## I9 — reading core's model from npc DEADLOCKS: brain predicates re-enter the lock
**2026-08-10. RESOLVED — fixed at the root; P2c item 4 landed. See `completed.md`.**

`Bot::nearest_thing` / `nearest_tile` take the world guard and then run a **brain-supplied
predicate while still holding it**. Those predicates call back into `Bot`:

```rust
bot.nearest_thing(at, |cell, kind| {
    ...
    bot.tile_kind_at(cell)            // <- locks the mutex nearest_thing already holds
        .is_none_or(|k| ...) && self.usable_eat(id, kind, now).is_some()
})
```
(`bunnies.rs:378-384`; `bunnies.rs:323` and the wolf scans have the same shape.)

`std::sync::Mutex` is not reentrant, so the second lock blocks forever. Confirmed by reading, and
the npc process did stop emitting mid-tick when item 4 landed.

**The fix, for whoever takes item 4 next:** take the guard ONCE and pass the view into the
predicate — `pred(&WorldView, cell, kind)` instead of `pred(cell, kind)` — so a predicate queries
through the guard it is already inside rather than re-acquiring it. Re-entry then cannot compile,
which is better than not happening. Two call sites in `bunnies.rs`, more in `wolves.rs`.

**Honesty note on the diagnosis.** Reverting item 4 did **not** restore npc's move intents, and the
worker was found ~4,000 tics behind at the same moment ([I6](#i6) recurring). So the re-entrancy is
real and would deadlock regardless, but I cannot claim it was the *sole* cause of the stall I
observed — the world was degraded underneath it. Both need checking on a fresh world before item 4
is retried.

## I8 — `MoverTrack` drops payload/need rows that arrive before the pawn's first anchor
**2026-08-10. Open — a real bug in code THIS STREAM shipped today, found by the leak audit.**

`observe_payload` ([`movers.rs:196`](../../../client/core/src/movers.rs)) and `observe_need`
(`:205`) both `if let Some(m) = self.movers.get_mut(&entity)` and **silently drop the row when no
mover exists yet**. Both other hosts document the pre-anchor arrival as legal and buffer for it
(`MoverLayer.ts:378-380`; `wolves.rs:815-817` keeps a payload for an entity it has not adopted,
because a zone snapshot may fan `Payload` before `StateObject`).

Consequence: core loses the snapshot's payload and the pawn is stranded at `pace: None` — which, by
the deliberate choice recorded in [I2](#i2), means **held still forever**. I made the fallback
honest and then built a way to never leave the fallback.

Second defect in the same shape: `PawnNeed` also fans for `0x40…` **player-pawn** references
(`client/npc/src/lib.rs:192`) — entities with no position at all. A positional track cannot key
them, so hanging the rows off `Mover` is wrong independently of the timing bug.

**Chosen fix** (audit's, and it is right): the rows are not the mover's, they are the entity's. A
separate `GameplayRows` store in core keyed by entity, buffered regardless of anchor, evicted on
`StateGone` and zone close; `MoverTrack::derive_paces` READS it. That is [P2d](todo.md), and it
supersedes the `payload`/`needs` fields on `Mover`.

**Worth naming the mechanism, because it is the stream's own thesis turned on me:** I hung the rows
where my first consumer needed them — the pace derivation — rather than where they belong. That is
precisely "the label follows the first reader", the pattern the audit identifies as the cause of
every other leak here.

## I7 — the surviving drift is NOT any of the five TypeScript defects
**2026-08-10. Open — the target for P4, now with a source-level answer.**

An adversarial source analysis ([`walk-divergence.md`](walk-divergence.md), 42 agents, 33
candidates, 11 survivors) found five genuine defects in `MoverLayer.ts` — `walkGreedy` steps
CHEBYSHEV where every other consumer spends progress as EUCLIDEAN tiles, so the "mirrored EXACTLY"
comment at `:130` is simply false; `walkGreedy` cannot terminate from a fractional start; the
client reseed skips the `clear` clamp; `spec.ticsPerTile` is frozen at arm time and never
refreshed; and a same-tic intent tie resolves first-wins on the client and last-wins on the worker.

**And then showed that none of them explains the measured drift.** Each is bounded by one anchor
interval of progress — 1.33 tiles for a bunny — so they cannot arithmetically produce the observed
p90 of 4.60 or max of 12.53. Seven of the eight reported candidates also cannot move `anchorStride`
at all *by construction*, since that metric is a hypot between two SERVER rows.

The mechanisms it points at instead are structural, and the first is the interesting one: **the
client's speculation arms from the RENDERED point while the worker steps from the AUTHORITATIVE
one** (`MoverLayer.ts:978-979` seeds from `m.rx/m.ry`; the worker's `MOVE_TO` resolves from
`p.position_reference`). That is chord-movement F4 working as designed — it exists so an
interrupted trip re-aims from where the pawn is *drawn* and can never snap backwards — but it means
the two walks start from different points by construction, every time an order interrupts a walk.
With 63% of orders arriving mid-walk ([I4](#i4)), that is not an edge case.

**This is exactly what P4 deletes**, and it is the strongest argument yet that the stream's
structural fix is the right one: five real defects and one by-design divergence all disappear when
there is one walk instead of two. It also means P4's acceptance (reseed p50 under 0.5 tiles) is a
genuine test rather than a formality — if the number does not move, this analysis is wrong.

## I1 — the server walks bunnies at ~2× the pace the client speculates
**2026-08-09. 2026-08-10: NOT REPRODUCIBLE — the premise was mine and it was wrong.**

Re-measured on a freshly reset world with the new per-kind tally, and the paces **agree**:

| kind | client pace | implied by the server's own rows (p50) | anchor stride | predicted |
|---|---|---|---|---|
| bunny | 24 | **24.27** | 1.318 | 1.333 |
| wolf | 12 | **12.14** | 2.354 | 2.667 |

**Where the 2× came from: I pooled two species.** The original figure was a single distribution
over all movers. I checked that 86 of 90 were bunnies and concluded the pool was therefore
bunny-dominated — but the pool counted *moving* samples, not pawns, and I never verified that
bunnies contributed samples in proportion to their number. Per kind, each side's stride matches its
own pace. I cannot now separate "pooling artifact" from "artifact of a sim that was hours old and
about to fall over" ([I6](#i6)), and I am not going to invent a clean story: **the honest statement
is that the number does not reproduce on a healthy world, and I should not have reported it as a
measured divergence without splitting by kind first.**

**What survives, and it is not nothing.** On that same healthy world, spec reseed error still runs
p50 1.31 tiles (bunny) / 2.25 (wolf) with a p90 of 5.8 / 8.0 and a max over 15, and **~15% of
reseeds exceed `CHASE_SNAP_TILES`**. So the client's belief still drifts multiple tiles from truth
between anchors with the pace correct — the divergence is real, it is just **not** a pacing
divergence. Geometry, path consumption or lifecycle; the running `walk-divergence` analysis is
aimed there.

**What this does and does not change.** It does not touch the stream's thesis: the rule being
written twice is a structural defect whether or not today's two copies happen to agree, and the
headless clients hold no position track at all. The user made exactly that point when this
correction landed — *"even if this is for whatever reason somewhat performing correctly, it is very
likely still incorrect and requires changes."* It does demote measurement: P0 is a **regression
check**, not a gate ([F6](forks.md#f6)).

_Original entry, kept because the ruling-out work below still stands:_

Live, 86 bunnies + 3 wolves. The client derives **24 tics/tile** for a bunny, which is correct:
`walks` is a leveled passive (`ground_speed add = [24, 12, 6]`, `content/interactions.toml:92`) and
the bunny authors level 1. Authoritative rows arrive **p50 32 tics** apart — the `REANCHOR_TICS`
cadence, as designed. But the displacement between consecutive rows is **p50 2.67 tiles** (p90
5.41, max 7.85 ≈ `CHORD_CAP_TILES`), where 24 tics/tile predicts **1.33**. Consistently double.

Ruled out so far:
- **Not the trait level.** Decoded a live bunny's payload from `resonantdust-dev-pawn-0`: five
  `TRAIT` entries (header `327682` = op 5 count 2), refs `2148139024/040/072/088/104`, every
  `data = 0` and every **variant nibble 0** → tier 0 → level 1 → 24. The level round-trips.
- **Not the stat formula.** Both sides call the same `stat_eval::stat_value` on the same rows,
  through `pawn_gameplay_rows` (worker) and `decode_payload` (wasm), both of which merge the kind's
  constant binds via `object_trait_rows`.
- **Not a stale worker corpus.** `rd-worker` booted 2026-08-09T21:08:35Z, alongside the `forager`
  commit — it is holding the current corpus, so this is not a
  [content-rollout](../2026-08-09-content-rollout/README.md) symptom.

Remaining candidates, in order of suspicion: the anchor gap is not always one hop (`next_hop` can
fire early when the chord is shorter than the stride, so pairing rows 32 tics apart may span two
hops); the worker's `ground_speed_tics` reads a different row set than `pawnGroundSpeed` for
reasons not yet examined; the client's tic estimate runs slow, inflating apparent server speed.

**Chosen path: none of them, yet.** The fix that makes the question unaskable is a single shared
rule, so P0 pins the numbers and P4 must show them converge. Reopen loudly if it survives.

## I2 — a mover speculating at the pre-fan fallback is 8× wrong
**2026-08-09. Open — closes in P6.**

One of the 90 movers reported **3 tics/tile** — `DEFAULT_TICS_PER_TILE`, the fallback
`speedFor` takes when the payload has not fanned yet (`MoverLayer.ts:669`). For a bunny whose real
pace is 24 that is **8× too fast**: that pawn sprints its whole first trip and then gets yanked
back by the first authoritative row. The comment calls this "the pre-fan window", which is honest
about the mechanism and quiet about the size of the error.

The fallback is a symptom of the same architecture: the browser is asked to move a pawn before it
has been told what the pawn is. Once `client/core` owns the track it can simply **decline to move
an entity whose pace it does not yet know** — a pawn that sits still for one frame is invisible;
a pawn that teleports is not. Folded into P2/P6 rather than patched in TypeScript.

## I3 — the worker's rule is private to a binary, not a library
**2026-08-09. Open — this is P1's risk.**

`REANCHOR_TICS`, `CHORD_CAP_TILES`, `hop_stride_tiles` and `resolve_walk_position_for` all live
inside `server/worker/src/main.rs`, a binary crate with no lib target — so nothing outside the
worker process can call them and, per the tree's own note, `cargo test` there runs inside the
binary. Extraction is therefore not a re-export; it is a genuine move across a crate boundary,
touching the hot path that composes every tic.

Mitigation, already written into P1: land it as a **pure move with behaviour unchanged**, prove it
against the P0 baseline (anchor stride within 0.05 tiles) *before* P2 builds on it, and carry 20
landings recorded from the live worker as the unit-test corpus so the chord clamp and the recenter
cannot quietly change shape in transit.

## I4 — 63% of move orders interrupt a walk, but mid-chord resolve fires on 5%
**2026-08-09. Open — observation, may be benign.**

In 10 minutes the worker logged **731** `intent queue REPLACED by a fresh order` against **1157**
move intents — so most orders arrive while the pawn is still walking — but only **35**
`mid-chord resolve — the new order starts where the pawn is`. The resolve only logs when the
resolved point differs from the stored one, so a pawn genuinely sitting on its anchor is a
legitimate silent case. Whether 96% of interrupted walks are really at their anchor is untested,
and if they are not, every one of those orders restarts the walk from a stale point — which would
present exactly as a backward snap.

`position_at` (P1) is the function that answers this properly for both sides. Re-measure after P1;
if the resolve rate stays this low with a shared implementation, it is real and gets its own item.

## I6 — the worker's view of the master tic froze; it ground a backlog it could never finish
**2026-08-10. MOVED to its own stream — [2026-08-10-sim-liveness](../2026-08-10-sim-liveness/README.md).**

Kept here because it shaped two of this stream's measurements and will invalidate the remaining
ones if it recurs; owned there. The evidence below is duplicated into that stream's
[I1](../2026-08-10-sim-liveness/issues.md#i1).

_Original entry:_

P0's baseline run returned **zero `StateObject` rows in 30 s** with 90 movers resident. Not an
instrumentation fault — the sim had wedged:

- `rd-master` was **healthy**: `achieved_hz 5.9–6.2`, `bump_confirmed=360`, `bump_failed=0` every
  minute, continuously.
- `rd-worker`'s `master=` reading was **frozen at 33461 for 12+ minutes**, over which the master
  bumped ~4,300 tics. Every `composed component` line in that window read `master=33461`.
- The worker sat at **115% CPU** the whole time, composing components at tics *behind* where it
  had already been (33098 at 01:34 → 32036 at 01:52), i.e. grinding a backlog whose end had
  stopped moving toward it.
- Onset was a **cliff, not a ramp**: lag sat at 0 ±2 for the whole preceding period, hit 29, then
  2333 on the next sample, then drained steadily (~160 tics per sample block).

So the worker's *subscription to the index clock* died while its compose loop kept running — it
did not crash, log an error, or self-heal. That last part is the interesting bit: the tree has
rebuild-on-next-use uplinks specifically for this (`sim-self-heal`), and they did not fire, because
nothing here *failed* — a frozen-but-live subscription reads as a quiet clock, and a quiet clock is
indistinguishable from a paused world.

**Chosen action:** restarted the worker to get a measurable world, and recorded this rather than
chasing it — it is not this stream's subject and P0 is blocked without a live sim. **Flagged for
the user** in [`blockers.md`](blockers.md#b1): a silent clock-subscription freeze is a durability
bug that belongs to whoever owns the uplink, and the honest detection story is that *I only
noticed because a probe returned zero* — nothing alarmed.

**Two things the restart taught, both mine to own:**

1. `bin/sim run worker` runs the **already-built** binary. The one on disk predated
   [`live-edit` P4](../2026-08-09-live-edit/README.md)'s loader change, so it rejected the `color`
   field that stream had since authored onto trait defs — the worker crash-looped on
   `unknown field 'color'` until I ran `bin/sim build worker` first. The corpus was fine; the
   parser was old. **Restarting a sim process is `build` then `run`, never `run` alone** — the
   long-lived process is the only thing hiding a stale binary, and killing it is exactly when that
   stops being true.
2. A **second session is committing to this branch concurrently** (live-edit P3/P4/P5 landed while
   this stream's opening commit was being written). Nothing in this stream conflicts with it, but
   it means the tree is not mine alone: re-read before assuming, and never rebase or force.

Watch for a recurrence during the P4 re-measure; if it repeats on a timescale of hours it will
corrupt any long soak this stream depends on.

## I5 — 936 duplicate work-group WARNs in 10 minutes
**2026-08-09. Open — noted, out of this stream's scope.**

`duplicate work-group assignment — interaction already executed, SKIPPED (teleport verdict)` fires
~94/minute — 35% of all interaction dispatches. The dedup is working (they are skipped), so this
is not a correctness fault, but it is the cross-zone double-execution guard from
[spawn-authority](../2026-08-08-spawn-authority/README.md) absorbing a third of the dispatch load
at 90 movers. Recorded here because it was found while measuring; it belongs to whichever stream
owns work-group assignment, not to this one.
