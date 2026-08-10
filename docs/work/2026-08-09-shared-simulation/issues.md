# Issues — shared-simulation

_Problems hit, candidate solutions, which we chose and why. Chronological append._

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
**2026-08-10. Hit during P0. Cleared by restart; root cause NOT found.**

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
