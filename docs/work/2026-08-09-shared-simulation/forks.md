# Forks — shared-simulation

_Decision points, the options, which we took and why. Chronological append._

## F1 — where the shared movement rule lives
**2026-08-09.** Options: **(a)** a new `shared/move` crate; **(b)** a `move_eval.rs` module inside
`shared/content`; **(c)** `shared/codec`, beside `DEFAULT_TICS_PER_TILE`.

**Chose (b).** `shared/content/src/` already holds `path_eval`, `stat_eval`, `needs_eval` and
`emotion_eval`, every consumer already depends on the crate, and `shared/wasm` already re-exports
that family to the browser — a new crate would buy nothing and cost a dependency edge in five
places. Rejected (c) because the walk needs the corpus (pace comes from `stat_eval`, pathability
from the composed view) and `codec` is deliberately corpus-free; putting it there would drag the
bundle into the bit-layout crate. The module also inherits the family's naming, which is the point:
the next person looking for the movement rule looks where the other rules are.

## F2 — what webgl is allowed to keep
**2026-08-09.** The user's line is "webgl is just displaying whatever client core says should
exist". Taken literally, the render-chase goes too — it computes a position core did not state.

**Kept the render-chase in webgl; moved everything else out.** The distinction that survives
scrutiny is not *"does it compute a number"* but *"must the server agree with it"*. The chase
exists because snapping a sprite to a corrected position looks broken; it closes the gap at
≤ `CHASE_CAP`× true speed and is deliberately **wrong on purpose**, trailing truth to look right.
No other consumer may share that answer — an npc that chased a smoothed position instead of the
real one would be reasoning about a lie. So: **core says where the pawn is; webgl decides how
quickly the sprite admits it.** Facing-from-rendered-delta stays for the same reason. Everything
that must match the server — the walk, the pace, the path, the reseed — leaves.

## F3 — how the browser reads positions
**2026-08-09.** Options: **(a)** `shared/wasm` exposes `move_eval` and webgl drives it with its own
state; **(b)** `shared/wasm` exposes **`client/core`'s track** and webgl just asks for a point.

**Chose (b).** (a) is the current defect wearing a shared function: webgl would still own the
mover state, the intent bookkeeping and the replay guards, and would still be a second client
implementation — just one calling a shared helper. Only (b) makes the browser a reader. It also
means the guards `MoverLayer.ts` earned the hard way (the stale-intent rejection, the older-row
rejection — the teleport verdict) get ported ONCE into core and inherited by npc, which never had
them.

## F4 — delete the TypeScript, or keep it behind a flag
**2026-08-09.** A flag is tempting: the TS speculation works today and P4 is the risky phase.

**Delete it.** Two implementations behind a flag is the same defect with a switch on it — and the
flag's OFF path would rot silently, which is exactly how the mirror drifted in the first place.
Git holds the history. This follows the tree-wide delete-don't-deprecate posture. The safety this
buys is bought instead by phase order: core's track lands and is proven headless (P2/P3) before
webgl's copy is removed (P4).

## F5 — root-cause [I1](issues.md#i1) first, or fix the architecture first
**2026-08-09.** The 2× pace divergence is measured but not isolated. Tempting to chase it: it is
the thing the user can see.

**Architecture first, with the measurement pinned.** Isolating which of two copies is wrong is
work the fix deletes — and whichever answer we got, the correct action would still be "make there
be one copy". But going in blind would let the stream declare success on a symptom that moved
rather than went, so P0 pins the numbers before anything changes and P4 must show the divergence
gone. If a single shared rule does not close it, the diagnosis was wrong and we find out loudly
instead of quietly.
