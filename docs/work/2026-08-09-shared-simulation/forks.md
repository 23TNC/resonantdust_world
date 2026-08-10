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

## F8 — a WALK counts as busy, and that reduced the order rate on purpose
**2026-08-10.** `IntentQueues::busy` treats a phase-1 (walking) entry as committed. Wiring the
brains onto it dropped observed move intents to roughly a third of the wall-clock guess's rate,
which looked like a regression and is not.

The old guess only held during a DURATION act; a walking pawn read as idle, so a brain re-ordered
over its own walk. That is [I4](issues.md#i4)'s measurement from the other end: **63% of move
orders arrived while the pawn was already walking**, and each one restarted the trip. The new hold
is the server's own answer — the walk is in the queue fan — so the redundant re-orders stop.

Kept, because the alternative is worse in both directions: reading a walking pawn as idle is what
produced the interrupt storm, and `fire_tic 0` on a phase-1 entry means a naive "has it fired?"
check calls every walking pawn idle. Watch it in the P6 soak — if pawns visibly dither or stop
reaching destinations, the fix is a supersede rule, not going back to a clock.

## F7 — the composed WORLD VIEW is the same defect, one layer up
**2026-08-10.** Asked why `client/core` has no world view and how the npc manages without one, I
checked instead of defending, and the answer refutes what I had said.

**What is actually true.** `client/core` *emits* `ColdTiles` / `ColdThings`
([`api.rs:189,192`](../../../client/core/src/api.rs)) and **retains nothing** — it is transport
plus decode. Every consumer then builds its own composed view:

- **`client/npc`** builds one in Rust — `tiles` (zone → 256 kind slots), `tile_overlays`, and the
  composed THING view, with accessors for composed kind at a world coord, nearest matching tile,
  composed thing kind, nearest thing ([`lib.rs:265-272, 440, 461, 491, 505`](../../../client/npc/src/lib.rs)).
- **`client/webgl`** builds a *separate* one in TypeScript — `tileKindAt`, overlay compositing,
  `thingDefAt` ([`WorldBridge.ts:209, 618-648, 831`](../../../client/webgl/src/game/world/WorldBridge.ts)) —
  and wires the pathability probe off it at [`WorldScene.ts:123`](../../../client/webgl/src/scenes/world/WorldScene.ts).

So the composed world view is written **twice, in two languages, and lives in neither the core nor
`shared/`** — the identical shape to the walk, one layer up. I had described this as "core has no
world view", which is true and misleading: it implies none exists, when in fact two do and they can
disagree. The `walk-divergence` analysis already caught them disagreeing — the client's probe reads
an unstreamed cell as OPEN while the worker's mirror does not.

**What it changes.** My earlier P4 plan was to plumb a `js_sys::Function` pathability callback from
TypeScript into wasm so the shared walk could ask the browser what is walkable. That is backwards:
it would cement the browser as the owner of world state and make the Rust client permanently
dependent on a JS callback that `client/npc` cannot supply. **The world view moves into
`client/core` instead** — one composed view fed by the events core already receives, read by npc
and webgl alike, and available to `MoverTrack` natively with no callback at all.

**This widens the stream, and deliberately.** It is the same instruction — simulation state does
not live in the presentation layer — applied to the thing the walk depends on. P4 cannot honestly
be finished without it: a shared walk reading an unshared map is still two simulations. Added as
**P2b**, before P4, because P4's pathability seam is its output.

## F6 — measurement is a regression check, not a gate
**2026-08-10.** [F5](#f5) parked I1's root cause and pinned a baseline instead. Then the baseline
refuted the premise: per kind, the paces agree ([I1](issues.md#i1)). That left a real question —
if the headline symptom is not what I thought, does the structural work still lead?

**Yes, and the plan's ordering was wrong to imply otherwise.** The user settled it: *"even if this
is for whatever reason somewhat performing correctly, it is very likely still incorrect and
requires changes."* The defect was never "the numbers are bad"; it is that one rule is written
twice in two languages and the headless clients hold no position track at all. Those are true
independent of any measurement, and a session that treats a green measurement as permission to stop
has misunderstood the stream.

So **P0 is demoted to a regression check.** Its probe stays (it is how P4 proves webgl still
renders correctly once its simulation is deleted), but no structural phase waits on it. Concretely:
P1–P4 proceed on their own acceptance, and the 10-minute soak becomes a before/after comparison
rather than a gate. This is a correction to my own plan, not a change of scope.

## F5 — root-cause [I1](issues.md#i1) first, or fix the architecture first
**2026-08-09.** The 2× pace divergence is measured but not isolated. Tempting to chase it: it is
the thing the user can see.

**Architecture first, with the measurement pinned.** Isolating which of two copies is wrong is
work the fix deletes — and whichever answer we got, the correct action would still be "make there
be one copy". But going in blind would let the stream declare success on a symptom that moved
rather than went, so P0 pins the numbers before anything changes and P4 must show the divergence
gone. If a single shared rule does not close it, the diagnosis was wrong and we find out loudly
instead of quietly.
