# Deviations — shared-simulation

_Where the code departs from the plan (`components/<c>/{design,intent}`). Rows: date · what the
plan says · what the code does · why · fix/status. Log at the moment of deviating; also log one
you find._

## D1 — the client's simulation lives in the presentation layer
**2026-08-09 · found, not introduced this stream · OPEN → the whole stream is the fix.**

**What the plan says.** `client/webgl` renders; `client/core` is the client. The Rust core holds
the world model and every host — browser, npc, any future headless consumer — reads it. Simulation
rules the server and a client must both compute live in `shared/` and are called, not copied.

**What the code does.** The movement rule is implemented **twice** and shared **zero** times:

- `server/worker/src/main.rs` owns it privately — `REANCHOR_TICS` (:385), `CHORD_CAP_TILES` (:387),
  `hop_stride_tiles` (:391), `resolve_walk_position_for` (:412), `MOVE_TO` (:2953),
  `MOVE_STEP` (:2986).
- `client/webgl/src/game/world/MoverLayer.ts` re-implements it in **TypeScript** — `Spec` (:110),
  the walk annotated *"The server's stepping rule, mirrored EXACTLY (worker `apply` MOVE_TO)"*
  (:130), `speedFor` (:660), `computePath` (:699), the reseed (:818).
- `client/core/src/world.rs` is 158 lines of decode and holds no positions at all.
- `client/npc/src/lib.rs:266` keeps `pawns: HashMap<u32, (i32, i32)>` — the last authoritative
  **whole tile**, no subtile, no pace, no advancement between anchors.

**Why it happened.** Speculation was built where its first consumer was — the browser, which was
the only client that had to *look* right. Nothing in the tree objected: `shared/content` already
had `path_eval`/`stat_eval`/`needs_eval`, so the seam existed and was simply not used, and the
comment "mirrored EXACTLY" was accepted as a contract when it is only a hope. *"The browser needed
it first" is not a reason — it is the absence of one.*

**Consequences, both measured.** The two walks disagree by 2× ([I1](issues.md#i1)), which the
render-chase reports honestly as a teleport — 116 of 214 reseeds past `CHASE_SNAP_TILES` in 54 s.
And every headless consumer reasons about pawn positions that are tile-granular and up to one
chord stride (8 tiles) stale, because the code that closes the gap runs in a browser tab.

**Fix/status.** OPEN. [`todo.md`](todo.md) P1–P4: the rule moves to `shared/content::move_eval`,
`client/core` gains the mover track, npc reads it, webgl keeps only the render-chase
([F2](forks.md#f2)). P5 adds the guard that would have caught this, because nothing did.
