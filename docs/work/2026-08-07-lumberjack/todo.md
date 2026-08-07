# Plan — lumberjack

_Items never move; `[x]` IS the move. Context in [`README.md`](README.md), decisions in
[`forks.md`](forks.md) (F#), the anticipated-issue inventory in [`issues.md`](issues.md)
(I#)._

## P0 — the paper

- [x] ACTIONS.md: the intent-queue law — EPHEMERAL per-pawn list (F1), cap 5, fresh
      order REPLACES (F3), duration rides queue_at, every completion RE-VALIDATES →
      no-op when stale (I2). Acceptance: docs-check green; law readable alone. → new
      §"The intent queue, and interactions that cost tics" after §Movement: one
      composer, replace-whole, per-kind advancement (in-pass / final-hop-by-serial /
      queue_at +N), the no-op law; docs-check green.
- [x] VARIABLES.md: the `"adjacent"` location rule (F2), `duration`,
      `destroy = "carrier"` + reserved `yields` (F5) in the TOML schema. Acceptance:
      docs-check green. → the three location rules enumerated on drink's block (drink
      shown `adjacent`), duration's completion/re-validate comment, a full `cut_down`
      example block with `destroy = "carrier"` + the RESERVED `yields` note; green.

## P1 — the corpus

- [x] interactions.toml: `logging` stat, `lumberjack` trait, `can_fell_trees`, `cut_down`
      (adjacent, duration 30, destroy carrier — F7); `drink` → adjacent; the consumed I9
      RESERVED note deleted. Acceptance: content-check clean. → all six defs authored;
      the loader learned `destroy` (carrier-only, refusal tested), `adjacent` joined the
      built location rules, negative durations refuse; content-check clean.
- [x] things.toml: tree/shrub/cactus carry `cut_down` (F6); humans author lumberjack 1
      (F4); golden re-blessed (I5). Acceptance: golden diff = the authored rows only;
      loader round-trips `duration`/`destroy`. → golden diff audited: exactly the new
      registry rows (trait 0x80030030 / interaction 0x80040030 / affordance 0x80050030 /
      stat 0x80060030), drink `adjacent`, three fellable things, humans +lumberjack@1;
      39+2 content tests green incl. the new destroy/duration round-trip.

## P2 — adjacency

- [x] `"adjacent"` in BOTH gates — worker location match + wasm menu offer (Chebyshev ≤ 1
      inclusive); the worker resolves THING carriers from thing ⊕ overlay (I1).
      Acceptance: shared-crate rule tests; native+wasm gates green. → ONE
      `location_in_range` in shared/content (tested), the wasm filter takes
      `cheb_distance` (both wrappers + WorldScene), the worker resolves CANDIDATES per
      rule (adjacent+destination = that cell within 1; destless = the pawn's 3×3) and
      probes tile AND thing carriers (things first; overlay kind-0 suppresses); acting
      pawn/destination generalized to the reserved inputs (destroy-only interactions
      have no satisfy/move); thing uplink subscribes the composed tiers. Gates:
      shared+core+worker builds, 40+2 content tests, tsc clean.
- [x] Drill: a human drinks from the SHORE tile; the wolf's stand-on-water drink still
      lands (F2, I6). Acceptance: both `satisfied=Some("thirst")` in one worker log.
      → 02:39:51 wolf 0x30800000 ON the water 34.62→37.62 (cheb 0 through the new
      probe); 02:41:03 human 0x30800005 from the SHORE (102,67)→water (102,68) at cheb 1,
      0.0→3.0. Negative proven first: the same water menu from cheb 5 offered
      `["Move To"]` only — Drink suppressed by the strict pre-queue gate.

## P3 — the intent queue

- [x] The ephemeral queue (F1) + composition: out of place → `[move_to(adjacent), act]`;
      in place → `[act]`; fresh order replaces; cap-5 rejects whole (F3). Acceptance:
      composed queue logged; a mid-walk re-order leaves ONE fresh queue. → version word
      = the discriminator (0 fresh/replaces, 2 advanced, 1 completion); drink gained the
      `destination` input so far orders can compose (npc binds it = its own tile; golden
      re-blessed — satisfy amount Input(2)); menu relaxes distance ONLY for
      destination-bearing signatures. DRILLED: far-water Drink composed
      (`intent composed … pending=1`), and a mid-walk Move To logged
      `intent queue REPLACED by a fresh order (F3)` — she walked back, ZERO drink lines
      after the replacement.
- [x] Advancement (I4): `duration = 0` completes in-pass; move_to advances on the
      chain's FINAL hop, keyed by trip serial (a superseded chain advances nothing).
      Acceptance: walk-lands → next intent queued in the log; queued drink still +3.
      → arrival detected by authoritative-position-equals-dest polling (runs BEFORE the
      empty-pass early-out); the serial check is SUBSUMED by the replace law — a fresh
      order clears the queue before its seed supersedes the chain, so a superseded
      Move-running queue cannot exist (stated in the code). DRILLED: `intent advanced —
      walk landed` then `drink … 0.0→3.0 … version=2` 8 s after composition.
- [x] Duration via queue_at: a `duration = N` head queues its COMPLETION at `+N`, which
      RE-VALIDATES affordance + location and no-ops when stale (I2). Acceptance: fire
      tic = start + N in the log; a walked-away pawn's completion logs the no-op.
      → CLEAN-WORLD drills: schedule 4261 fire_tic 4290 → `executed tic=4290
      destroyed=true version=1` (fire = start + 30 exact, twice more at 6954/6979).
      The stale case drilled as a RACE (walking out of inclusive-adjacent range in 30
      tics is impossible at 24 t/t — two lumberjacks chop ONE tree staggered): B's
      completion felled it at 6954, A's at 6979 logged `intent completion NO-OP —
      no carrier in range offers this interaction`. The user's law verbatim, zero
      cancellation machinery. (Dropped completions log as NO-OP INFO now, not WARN.)

## P4 — timber

- [x] A FRESH lumberjack (I8: worker restarted, re-minted) menu-clicks a FAR tree: Cut
      Down offered, queue walks then chops 30 tics, the cell SETs 0, tree AND shadow
      leave the render (I3). Acceptance: walk+chop+destroy log; before/after captures.
      → drilled TWICE (dirty world, then the user-requested CLEAN state after a full
      `redeploy --force`): compose 4158 → walk landed 4257 → scheduled fire 4290 →
      executed AT 4290 destroyed=true → tombstone tic 4293 at (103,56) → she stands in
      a clear cell, no orphan shadow (captures). RELOAD-persistence proven: a fresh page
      re-receives the tombstone (`2:103:56` in coldOverrides, thingDefAt = 0) — after
      FIXING two real bugs this drill exposed: the edge framed thing tombstones as
      removed:true (client resurrection), and `seed_zone`'s process-memory once-guard
      re-seeded zones on every edge restart, re-stamping baselines OVER older tombstones
      (now gated on the applied snapshot being empty).
- [x] The refusals + neighbours: a wolf's tree menu offers no Cut Down; shrub and cactus
      fell; drink + wolf trips beside (F6). Acceptance: refusal + two fells + both
      species' arcs in one session. → the wolf's tree menu is EMPTY (affordance gate;
      F7 empty-set-opens-nothing); tree ×3 + shrub ×1 felled (4 tombstones); CACTUS is
      authored identically but UNDRILLED — no desert biome within the drilled area
      (stated, not assumed); the wolf's thermostat drinks (34.8→37.8 twice) and trips
      ran beside the humans' chops and the 17-tile composed walk-then-drink (her needs
      row +3 at set_tic 9275 — sql-verified; log greps are ANSI-blind, noted).

## P5 — the verdict

- [x] Docs + memory truth pass: memories note timed interactions + the queue; consumed
      RESERVED notes gone; index row records delivery. Acceptance: docs-check green.
      → new `lumberjack-delivered` memory + index line (the queue law, adjacency,
      destroy, the seed-guard + registry-seed gotchas); the consumed RESERVED notes died
      in P0/P1; the work-index row flipped done with the delivery note; docs-check
      green.
- [x] Cold boot: pending intents drop, in-flight completions survive + re-validate —
      state what actually happened (F1); fell + drink + wolf arcs green; **the user's
      eyes close the stream**. Acceptance: captures + logs in completed.md. → THREE
      boots this stream: (1) an unplanned HOST BLUESCREEN mid-drill — durable clock,
      all 9 pawns and the tombstones survived; (2) the user-requested clean
      `redeploy --force` standup (registry re-seed + fresh mints + every drill green
      from zero); (3) an orderly worker bounce MID-CHOP: the parked chop DROPPED with
      the ephemeral queue (F1's stated loss, observed — B finished his durable walk,
      no tombstone appeared, no wrong write), while durable queued events (the chain;
      completions by the same mechanism) survived and completed. The user's eyes close
      the stream on this report.
