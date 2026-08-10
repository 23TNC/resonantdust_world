# Issues — shared-simulation

_Problems hit, candidate solutions, which we chose and why. Chronological append._

## I1 — the server walks bunnies at ~2× the pace the client speculates
**2026-08-09. Open — measured, deliberately not isolated ([F5](forks.md#f5)).**

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

## I5 — 936 duplicate work-group WARNs in 10 minutes
**2026-08-09. Open — noted, out of this stream's scope.**

`duplicate work-group assignment — interaction already executed, SKIPPED (teleport verdict)` fires
~94/minute — 35% of all interaction dispatches. The dedup is working (they are skipped), so this
is not a correctness fault, but it is the cross-zone double-execution guard from
[spawn-authority](../2026-08-08-spawn-authority/README.md) absorbing a third of the dispatch load
at 90 movers. Recorded here because it was found while measuring; it belongs to whichever stream
owns work-group assignment, not to this one.
