# Todo — pathfinding (planned, not started)

Executes the design in [`docs/intent/pathfinding/`](../../intent/pathfinding/README.md) — *"store
the intent; derive the position."* Decisions + their reasoning: [forks.md](forks.md). Log any
deviation in [deviations.md](deviations.md) **the moment you make it**.

**Validate every shape against the design, not this file** — per
[CONVENTIONS](../../CONVENTIONS.md), `design/`+`intent/` win; a todo is a derived artifact and can
be stale. `grep` before you build.

---

## What the code supports today (verified 2026-07-15, not assumed)

| Need | Today | Verdict |
|---|---|---|
| Schedule a `place` at `start+duration` | `append()` hardcodes `event_tic = master_tic + TIC_GAP(3)` | ⛔ **the one real blocker** |
| A future row waits, doesn't misfire | worker gates execute on `master_tic >= event_tic` | ✅ already correct |
| In-flight object stays readable | read rule = "no pending row **at or below** the read tic" — a write at `T+50` doesn't block reads at `T` | ✅ **safe by construction** |
| A far-future pending row survives GC | `Holder` refcount keeps it; `tick_gc` skips held rows | ✅ |
| Somewhere to put the intent | `data0`/`data1` are **overloaded** (hp *and* cold provenance) | ⛔ needs new payload fields |
| Motion reaching the client | edge already relays `state` rows | ✅ **free**, if motion is payload |

**Read this before building:** the design's §7 ("does the resolution model support idle-until-tic?")
is **answered — yes, via the read rule**, and that is the load-bearing insight of the whole feature.
It's why no per-tile row and no `motion` table are needed.

---

## P0 · `ACTION_MOVE` → `ACTION_PLACE` (pure rename)

- Today's `ACTION_MOVE (0x100)` sets an absolute position in one tic. That's **`place`** in the
  design's vocabulary. Rename it; the new `ACTION_MOVE` is the pathfinding verb.
- ⚠️ `ACTION_*` ids are an **append-only palette** — stored programs carry the numbers
  (`domain::tests::action_ids_are_stable_and_distinct`). `place` **keeps `0x100`**; the new `MOVE`
  takes a **new id**. Do not renumber.
- Touches: `codec::event_word` (`ACTION_MOVE` → `ACTION_PLACE`), `tick::domain` + `vm`, the edge's
  producer, npc, `pack_move` → `pack_place`.
- Done = rename only, zero behaviour change; tests + browser unchanged (wolves still walk).

## P1 · `tile_at` + the motion payload — the deterministic core

- **Payload fields** on the shard's `decl_tick_pipeline!` (per [D2](forks.md)):
  `move_source: u8`, `move_dest: u8`, `move_start_tic: u32`, `move_duration: u32`
  (`0` duration = not in motion — the "settled" sentinel). Exact packing TBD against the object
  model (design §9.1); tiles are `tile_reference` (`x:4|y:4`), so a **cross-zone** move needs
  `dest_zone` too — decide then, don't discover it in P3.
- **`tick::path::tile_at(source, dest, start_tic, duration, tic) -> tile_reference`** —
  **integer only, no floating point** (design §6: worker is native, client is wasm; FP may differ).
  v1 path = a trivial deterministic line (Bresenham); the real A* is P7 and **must not change this
  signature**.
- **The cross-target determinism test** (design §6) — same `(source, dest, …)` native vs wasm,
  diff the result. Build it **now**, with the trivial path, while it's cheap to satisfy. It is the
  guardrail the whole silent-sync claim rests on.
- Done = `cargo test` green + the determinism test runs in both targets.

## P2 · `append_at(tic, actions, targets)` — the scheduler seam

- Add alongside `append` (keep `append` = `append_at(master_tic + TIC_GAP)`).
- ⚠️ Verify `drop_timed_out` does **not** kill a future row: it evicts `ENQUEUE`/`QUEUEING` past
  `ENQUEUE_WINDOW_TICS(3)` off `tic_state_change`. A future row should be stood up to `IN_QUEUE`
  within a tic and thereafter is immune — **confirm live**, don't assume.
- Schema/reducer change → **live check, not just a green build** (subscription SQL is a string).
- Done = a row appended at `now+20` sits, fires at exactly `now+20`, isn't dropped.

## P3 · The new `ACTION_MOVE` — writes intent, schedules the arrival

- Program: `[LITERAL duration] [LITERAL dest] [OBJECT pawn] [ACTION MOVE]` (arity/word order to
  match the design's RPN discipline — operands push, the verb pops LIFO).
- Resolving it writes the **motion payload** (source = current tile, dest, start_tic = event_tic,
  duration) and `append_at(start+duration)` a `PLACE(dest)` against the same target.
- **Stash the scheduled `event_reference`** on the object — P6 needs it to revoke ([D5](forks.md)).
- Done = live: issue a MOVE, watch the intent row land, watch the PLACE fire N tics later at `dest`.

## P4 · Occupancy — the server knows the in-flight tile

- *"The server will need to work out what tile an object in motion occupies so we can handle some
  events properly."* → wherever the worker reads an object's tile for game logic, read
  `tile_at(state, tic)` rather than `state.location` when `move_duration != 0`.
- One helper in `shared` (`tick::path::occupied_tile(state, tic)`), called by worker + edge + client
  — same reason the codec is shared: two call sites can't drift.
- Done = an event resolved mid-flight sees the interpolated tile, proven by a test.

## P5 · Client — render from intent, tween to `place`

- pixijs drives the object with the **same `tile_at`** via wasm; it does **not** wait for the PLACE.
- On PLACE, tween to the confirmed tile ([sync.md](../../intent/sync.md): synced clock + render
  delay `D` + interpolate-by-`valid_at`). If determinism holds the tween is sub-tile — a smooth
  landing, not a correction. **If it's a visible jump, determinism is broken — that's the bug
  signal**, don't paper over it with a longer tween.
- Done = browser: a wolf walks smoothly; the PLACE is invisible.

## P6 · Interruption + supersession ([D5](forks.md), design §5)

- A new MOVE mid-flight, or a block: `tile_at(interrupt_tic)` → PLACE there → new MOVE from there.
- The speculative PLACE **must not fire as written** — revoke it (candidate: `abort` by the stashed
  `event_reference`; the lifecycle already has `abort` + `release_holds`).
- Done = interrupt mid-flight; the object stops where it *was*, and the stale arrival never lands.

## P7 · The real pathfinder + speed model (design §6)

- Integer A* over the static grid; terrain/diagonal costs; `duration_tics` **derived** rather than
  supplied ([D1](forks.md)) + validation of caller-supplied durations. Segment policy ([D6](forks.md)).
- The row shape from P1 must not need to change. If it does, P1 got it wrong.

## P8 · Moving obstacles (design §5)

- The pathfinder's input becomes "the grid **plus every active intent projected forward**". The
  expensive, stateful part — and the reason segments are short.
