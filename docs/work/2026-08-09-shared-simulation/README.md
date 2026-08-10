# shared-simulation — one rule, in `shared/`, called by everyone

_User (2026-08-09): "We should never have had ANY mirrored code in typescript... or rust. If we
need shared simulation code between server and client then it should be in shared as a shared
module thereby guaranteeing our calculations are done exactly the same. Furthermore... by moving
logic into webgl for some stupid reason... you've broken our headless clients. So all of our NPC's
will fail to function correctly. So... we need to ensure that webgl is just displaying whatever
client core says should exist. We seem to have got lazy and started implementing simulation logic
in webgl which is fundementally incorrect."_

The stream opened on a symptom the user could see — bunnies snapping across tiles — but the
symptom is not the subject. The subject is that **the walk rule is written twice and owned by
neither.**

## The survey — where the movement rule actually lives

| consumer | what it holds | language |
|---|---|---|
| `server/worker` | THE rule, authoritative: `REANCHOR_TICS = 32` ([`main.rs:385`](../../../server/worker/src/main.rs)), `CHORD_CAP_TILES = 8` (:387), `hop_stride_tiles` (:391), `resolve_walk_position_for` (:412), `MOVE_TO` (:2953), `MOVE_STEP` (:2986) | Rust, **private to the binary** |
| `client/webgl` | a RE-IMPLEMENTATION: `Spec` ([`MoverLayer.ts:110`](../../../client/webgl/src/game/world/MoverLayer.ts)), the walk — commented *"The server's stepping rule, mirrored EXACTLY"* (:130) — `speedFor` (:660), `computePath` (:699), the reseed (:818) | **TypeScript** |
| `client/core` | nothing. [`world.rs`](../../../client/core/src/world.rs) is 158 lines of DECODE — `StateRow` → tiles, `move_intents`, coordinate conversion | Rust |
| `client/npc` | `pawns: HashMap<u32, (i32, i32)>` ([`lib.rs:266`](../../../client/npc/src/lib.rs)) — the last authoritative **whole tile**. No speculation, no subtile, no pace | Rust |

Two implementations of one rule, in two languages, and **the Rust client — the one every headless
consumer is built on — has neither.** A brain asking "where is that pawn" gets an answer that is
tile-granular and up to one full chord stride (8 tiles) stale, because the only code that closes
the gap between anchors lives in a browser.

The comment `"mirrored EXACTLY"` is the whole defect in two words. A mirror is a claim that has to
be re-earned on every edit to either side, by hand, forever — and nothing checks it.

## What is already shared (the seam exists; it was just not used here)

This is not a new pattern to invent. `shared/content/src/` already holds
[`path_eval.rs`](../../../shared/content/src/path_eval.rs),
[`stat_eval.rs`](../../../shared/content/src/stat_eval.rs),
[`needs_eval.rs`](../../../shared/content/src/needs_eval.rs) and `emotion_eval.rs`, and
[`shared/wasm`](../../../shared/wasm/src/lib.rs) already re-exports them to the browser —
`findChords`, `pawnGroundSpeed`, `pawnConditions`, `pawnNextCrossing`. `shared/codec` owns
positions, subtiles, rows and `DEFAULT_TICS_PER_TILE`.

**The eval families that ARE shared agree. The one that isn't, diverges.** That is the argument
for the stream, and it is worth stating as evidence rather than as principle: the pawn's *pace* is
derived through the shared `stat_eval` on both sides and both sides get the same number; the
pawn's *walk* is written twice and the two walks disagree by 2× (below). There is no `move_eval`.

## What we measured (2026-08-09, live: 86 bunnies + 3 wolves)

- Client-derived pace is **24 tics/tile** for 86 of 90 movers — correct: `walks` is a leveled
  passive (`ground_speed add = [24, 12, 6]`) and the bunny authors level 1.
- Authoritative rows arrive **p50 32 tics** apart — the re-anchor cadence, exactly as designed.
- Displacement between those rows is **p50 2.67 tiles** (p90 5.41, max 7.85 ≈ `CHORD_CAP_TILES`),
  where 24 tics/tile predicts **1.33**. The server is walking bunnies at roughly **twice** the pace
  the client speculates ([I1](issues.md#i1) — measured, not yet isolated).
- Spec reseed error: **p50 3.4–4.0 tiles, p90 8.3, max 13.1**, and **116 of 214 reseeds in 54 s
  exceeded `CHASE_SNAP_TILES = 3`** — past which the render-chase gives up and teleports. *That is
  the snapping the user saw.* It is not a rendering bug; it is the render honestly reporting that
  the client's simulation and the server's have diverged past rescue.
- One bunny was speculating at **3 tics/tile** — `DEFAULT_TICS_PER_TILE`, the pre-fan fallback: an
  8× error the current design simply tolerates ([I2](issues.md#i2)).

**The trait level is tracked correctly — that is not the bug.** Decoded a live bunny's stored
payload out of `resonantdust-dev-pawn-0`: five `TRAIT` entries (header `327682` = op 5, count 2),
all `data = 0`, all with **variant nibble 0** → tier 0 → level 1 → 24. The level round-trips, and
both sides run the same `stat_eval::stat_value`. The shared parts agree.

## The stance

**A rule that the server and any client must both compute belongs in `shared/`, in Rust, once.**
Everything else follows from that one sentence.

- **The module is `shared/content/src/move_eval.rs`** — beside the other `*_eval` siblings, because
  that family already has the consumers wired ([F1](forks.md#f1)). It owns the cadence and cap
  constants, `hop_stride_tiles`, the chord step, and — the function that does not exist anywhere
  today as a shared thing — **`position_at(...)`: where a walking pawn IS at an arbitrary tic**.
  The worker's `resolve_walk_position_for` and the TypeScript `Spec` walk are both partial,
  divergent copies of exactly that function.
- **`client/core` gains the mover track.** It is the Rust client; it must be able to answer "where
  is this pawn now" without a browser. It holds, per entity, the last authoritative point + tic and
  the active intent, and advances them through the shared module. This is the piece whose absence
  broke the headless clients, and it is worth more than the webgl fix.
- **`client/npc` reads that track** instead of its own tile-only map — so brains see subtile
  positions and pawns that are actually moving, which is what chase, adjacency and forage all
  silently need.
- **`client/webgl` becomes a display.** It reads positions from core through `shared/wasm` and
  keeps exactly one thing: **the render-chase** — the capped catch-up that smooths a corrected
  position into a glide. That is presentation and it is legitimately client-side
  ([F2](forks.md#f2)). Facing-from-rendered-delta stays with it, for the same reason. `Spec`, the
  walk, `speedFor` and `computePath` are **deleted**, not deprecated ([F4](forks.md#f4)).
- **A guard, so this cannot come back.** The reason it happened is that nothing objected. The
  stream does not end until something fails when a simulation rule is written outside `shared/`
  ([P5](todo.md)).

The line the stream is drawing, stated so it can be applied to the next feature without asking:
**if the server and a client must get the SAME answer, it is simulation and it lives in `shared/`.
If only the viewer cares how it looks getting there, it is presentation and it lives in webgl.**
The render-chase is presentation. Where the pawn is, is not.

## Watch

- The worker's rule is **private to a binary** (`server/worker/src/main.rs`), not a library — the
  extraction is the risky half of P1, and it must land as a **pure move with behaviour unchanged**,
  proven by the live pace measurement, before anything else is built on it ([I3](issues.md#i3)).
- [I1](issues.md#i1) (the 2× pace divergence) is deliberately **not** root-caused first. Isolating
  which of two copies is wrong is work that the fix deletes; the shared module makes the question
  unaskable. But P0 pins the numbers so P1 can prove it did no harm, and P4 must show the
  divergence gone — if it survives a single shared rule, the diagnosis was wrong and we learn that
  loudly.
- Deleting the TS speculation without core's track landing first would leave movers frozen between
  anchors. The phase order is load-bearing, not cosmetic.

## Exit

`shared/content::move_eval` is the only implementation of the walk. The worker, `client/core`,
`client/npc` and `client/webgl` all call it. `MoverLayer.ts` holds no simulation state. A headless
npc can report a moving pawn's subtile position. Reseed error sits inside a fraction of a tile,
`CHASE_SNAP_TILES` stops firing in steady state, and the user watches the fluffle without seeing a
bunny jump.
