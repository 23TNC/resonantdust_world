# first-pawns — a wolf pawn in a `pawn` shard, driven by an npc container

_Work stream, opened 2026-07-28 (re-planned same day after the user corrected the movement
model). Components: `server/spacetime` (new `pawn` module), `server/{master,orchestrator,worker,
edge}`, `shared/codec` (`CREATE`, movement), `client/core` (tic estimate, intents),
`client/webgl` (speculation), `client/npc` (harness + brains). User's brief: ONE wolf pawn
generated in a pawn spacetime shard, driven by a rust npc container through the whole pipeline —
npc → client/core → edge (validate + queue) → event shard → orchestrator → worker → pawn data
shard → promote on tic → edge subscriptions → fan-out to webgl AND npc — structured in
anticipation of more advanced commands and multiple "brains" (villager-brain, wolves-brain,
region-steward-brain) built on the same npc base._

## The sync philosophy (user, 2026-07-28 — the design's spine)

We do NOT sync clients with servers — lockstep was tried and could never be kept. Instead the
server fans out its **authoritative state**, and clients issue commands based on whatever they
think state is. The **edge** validates and queues events against the server's authoritative
state; the **workers** validate and execute against it too. The spacetime shards are the root of
truth because they carry guarantees most of our system cannot. Movement is where this becomes
visible: a move's **intent** is broadcast once, clients **speculate** position from it — the
first time clients care about tic at all — and the server corrects them with authoritative state
at the destination, plus whenever the object's position must be resolved as part of working out
some other event. Individual steps are never fanned out.

## What exists, verified 2026-07-28 (build on it, don't re-invent)

- **The pipeline is live end-to-end** (spacetime-again W1–W9 + shard-tables P1–P4 core): edge
  `queue` → orchestrator union-find grouping + per-(shard,tier) `claim` → worker compose →
  `write` with the inline smart `PROMOTE` → `entity_state` → edge per-zone subscription →
  `ServerMsg::State` → client `StateObject`. Wolves have moved over WS through exactly this path.
- **A pawn shard is a one-line stamp**: `data_shard`'s whole module body is
  `resonantdust_codec::entity_tables!(data: u8);` — shard-tables lists "move wolves to a `pawn`
  table" as an enabled follow-on, and `TABLES.md`/`world-storage` already write "`data_shard`
  (→ `pawn`)". Module discovery is glob-based, so `modules/pawn/` is auto-picked-up by
  `rd redeploy`; DB `resonantdust-<env>-pawn-0` (no underscore → no name-mapping entry).
- **`ACTIONS.md` §Movement already designed this** (marked *tabled* — this stream un-tables it):
  `MOVE_TO` as a self-perpetuating chain (each hop writes one tile and queues the next),
  queue-at-a-future-tic (`event_tic ≥ master+3` — the completeness barrier is unaffected),
  don't-promote-every-hop (intent once via `PROMOTE_EVENT`, state sparingly), and the client's
  wall↔tic estimate refined from arrivals. The section predates the `PROMOTE`-prefix change and
  needs its rewrite as part of the work. Today's worker `MOVE_TO` arrives in one step.
- **`PROMOTE_EVENT` is half-built**: `queue` latches `EVENT_FLAG_PROMOTE`; `settle` projects
  `event` rows per zone (from `complete`'s zones); the edge already subscribes
  `SELECT * FROM event WHERE macro_position_reference = <zone>` and relays `ServerMsg::Event` —
  but the client engine DROPS it (`engine.rs` ignores promoted events). Client consumption is
  the gap, not the server.
- **The tic clock exists client-side, unused for movement**: `clock.rs` NTP-style offset +
  `syncedNowMs()`/`RENDER_DELAY_MS` in the webgl host — but nothing maps wall→tic and nothing
  interpolates. `valid_at` is retired (2026-07-15); `tic` (wrapping u16, serial arithmetic) is
  the ordering. `client/core/intent/sync.md` still speaks `valid_at` — rewrite at wrap.
- **`CREATE` is specified, not built**: `ACTIONS.md` §`CREATE` — mints the entity, spawn-log
  keyed `(event_reference, index) → minted id` for replay, conflict-free singleton at grouping.
  The worker's arm is a no-op. Today's wolves are CLIENT-minted (`wolf_key(i)` = `0x30000001..`),
  colliding with the browser's client-minted `selfEntity()` (`0x30000000 + player_id`).
- **`PLACE` carries no `definition_reference`** — why live wolves render with the TREE def
  (`MoverLayer` falls back to kind 1). The wolf visual is complete and unused: stem
  `pawn/animal/wolf`, masters `template.{e,s,n}.0.png`, facing from `data >> 6`
  (`0→s 1→e 2→n 3→e+flipX`). `CREATE` carrying the def turns wolves into wolves.
- **npc is an automated player** over `client/core` (gateway login, anchors, the same wire as a
  browser): one 266-line `main.rs` whose `Bot` harness is commented as the piece to extract when
  a second behavior lands. Built via `rd build npc`; today it is run manually — no supervised
  container. **`bin/sim`** runs the server sim crates as detached `rd-<crate>` containers; the
  sim stack is currently DOWN — standing it up is P0's first item, not a blocker.

## Design stance

- **The pawn shard is a NEW shard class, not a rename** (F1): `modules/pawn` stamped from
  `entity_tables!(data: u8)`; `TYPE_PAWN` (3) routed to it at orchestrator claim, worker
  compose/write, edge relay/subscribe. `data_shard` stays the catch-all hot shard the worker's
  `_ =>` arm lands on; retirement is a follow-on once nothing routes there.
- **Server-minted pawns** (P1): build `CREATE` per `ACTIONS.md` — spawn-log for replay
  idempotence, minted target never an operand. Kills the client-minted id collision class and
  plumbs `definition_reference` so a wolf IS a wolf on every client.
- **The npc asks A→B; the WORKER works out how** (F2, user-directed): the wolves-brain issues
  one `MOVE_TO obj dest`; the worker steps one tile per hop (greedy straight line now — the
  pathfinding seam is explicit, a follow-on) and self-queues the continuation at
  `tic + tics_per_tile`. Cadence: **intent once** (`PROMOTE_EVENT` on the initial event), state
  promoted at the **seed and the final tile only** — bare continuations fan nothing. Position
  also resolves (and promotes) whenever another event touches the object — free by
  construction, nothing extra to build.
- **Clients speculate on tic — the first time tic matters** (P3): a wall↔tic estimate anchored
  by every `state`/`event` arrival (`TIC_HZ` extrapolation, refined continuously — implicit
  sync from the data stream, not ping-pong lockstep), a decoded `MoveIntent` from the relayed
  `event`, and a `MoverLayer` that walks the pawn fractionally toward dest — smooth motion with
  zero per-hop bandwidth. Authoritative `state` snaps/reseeds the speculation (F8).
- **Brains are a library seam, not processes** (P4): `client/npc` splits into a lib (`Bot`
  harness + `Brain` trait) and a thin bin dispatching a named brain; one container = one brain
  (`rd-npc-<brain>`). The wolves-brain is the first impl; villager-/region-steward-brains are
  future impls of the same trait, not new harnesses.
- **Docs first**: `ACTIONS.md` §Movement is rewritten (prefix vocabulary, un-tabled) BEFORE the
  worker chain lands; every table change lands in `TABLES.md` before the module; subscription
  SQL is string-typed — every table-touching item re-checks the subscriptions naming it.

## Known hazards (accepted, recorded)

- **No ownership model** — any session may move any pawn (documented in `Command::Move`).
  Edge-side ownership validation is a follow-on with players; out of scope here.
- The browser's own `selfEntity()` stays client-minted this stream — only npc pawns use `CREATE`.
- Speculation is best-effort BY DESIGN: the server dictates truth, the client makes it smooth.
  The estimate drifts between promotes; v1 corrects by snapping (F8) and records the observed
  error so the re-anchor-every-N knob has data when it's wanted.

## Follow-ons (each a future stream, deliberately not here)

- **Pathfinding** in the worker's `MOVE_TO` step fn (obstacles, A*) — the greedy-line seam is
  built to be replaced.
- The **re-anchor cadence knob** (`PROMOTE` every N tiles) — tuned against the recorded
  speculation error; v1 is first+final only.
- **Speed from content** (`tics_per_tile` per kind via the DSL) — v1 is a constant behind a
  per-def seam (F7); the worker has no content corpus today (same gap as torch-thing I2).
- Client render-delay smoothing/`RENDER_DELAY_MS` integration beyond v1 speculation.
- Ownership: edge validates who may drive which entity.
- More brains: villager-brain, region-steward-brain; a brain fleet supervisor.
- `data_shard` retirement (or re-purposing) once no type routes to it.
- Sim process resilience is **2026-07-27-sim-self-heal** (open, separate) — its acceptance
  ("wolves resume moving") runs exactly this stream's pipeline.
